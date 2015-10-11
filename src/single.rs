//!
//! Definition of single (aka "regular") histograms.
//!
//! Single histograms represent measures on a set of entities known at
//! compile-time. If the set of entities is dynamically extensible,
//! you should rather used Keyed histograms.
//!

use rustc_serialize::json::Json;

use std::marker::PhantomData;
use std::mem::size_of;
use std::sync::atomic::{AtomicBool, Ordering};

use misc::{Flatten, LinearBuckets, SerializationFormat, vec_with_size};
use task::{BackEnd, Op, SingleRawStorage};
use service::{Feature, PrivateAccess};
use indexing::*;

///
/// A single histogram.
///
pub trait Histogram<T> {
    ///
    /// Record a value in this histogram.
    ///
    /// The value is recorded only if all of the following conditions are met:
    /// - telemetry is activated; and
    /// - this histogram has not expired; and
    /// - the histogram is active.
    ///
    fn record(&self, value: T) {
        self.record_cb(|| Some(value))
    }

    ///
    /// Record a value in this histogram, as provided by a callback.
    ///
    /// The callback is triggered only if all of the following conditions are met:
    /// - telemetry is activated; and
    /// - this histogram has not expired; and
    /// - the histogram is active.
    ///
    /// If the callback returns `None`, no value is recorded.
    ///
    fn record_cb<F>(&self, _: F) where F: FnOnce() -> Option<T>;
}

// Back-end features specific to single histograms.
impl BackEnd<Single> {
    // Instruct the Telemetry Task to record a single value in an
    // already registered histogram.
    fn raw_record(&self, k: &Key<Single>, value: u32) {
        self.sender.send(Op::RecordSingle(k.index, value)).unwrap();
    }
}

///
/// A histogram that ignores any input.
///
/// Useful for histograms that can be activated/deactivated either at
/// compile-time (e.g. because they are attached to specific versions
/// of the application) or during startup (e.g. depending on
/// command-line options).
///
pub struct Ignoring<T> {
    witness: PhantomData<T>,
}

impl<T> Ignoring<T> {
    pub fn new() -> Ignoring<T> {
        Ignoring {
            witness: PhantomData
        }
    }
}

impl<T> Histogram<T> for Ignoring<T> {
    fn record_cb<F>(&self, _: F) where F: FnOnce() -> Option<T>  {
        return;
    }
}

///
///
/// Flag histograms.
///
/// This histogram has only two states. Until the first call to
/// `record()`, it is _unset_. Once `record()` has been called once,
/// it is _set_ and won't change anymore. This type is useful if you
/// need to track whether a feature was ever used during a session.
///
///
/// With `SerializationFormat::SimpleJson`, these histograms are
/// serialized as a single number 0 (unset)/1 (set).
///
pub struct Flag {
    back_end: BackEnd<Single>,

    // A cache used to avoid spamming the Task once the flag has been set.
    cache: AtomicBool,
}

// The storage, owned by the Telemetry Task.
struct FlagStorage {
    // `true` once we have called `record`, `false` until then.
    encountered: bool
}

impl SingleRawStorage for FlagStorage {
    fn store(&mut self, _: u32) {
        self.encountered = true;
    }
    fn serialize(&self, format: &SerializationFormat) -> Json {
        match format {
            &SerializationFormat::SimpleJson => {
                Json::I64(if self.encountered { 1 } else { 0 })
            }
        }
    }
}

impl Histogram<()> for Flag {
    fn record_cb<F>(&self, cb: F) where F: FnOnce() -> Option<()>  {
        if self.cache.load(Ordering::Relaxed) {
            // Don't bother with dereferencing values or sending
            // messages, the histogram is already full.
            return;
        }
        if let Some(k) = self.back_end.get_key() {
            match cb() {
                None => {}
                Some(()) => {
                    self.cache.store(true, Ordering::Relaxed);
                    self.back_end.raw_record(&k, 0)
                }
            }
        }
    }
}


impl Flag {
    pub fn new(feature: &Feature, name: String) -> Flag {
        let storage = Box::new(FlagStorage { encountered: false });
        let key = PrivateAccess::register_single(feature, name, storage);
        Flag {
            back_end: BackEnd::new(feature, key),
            cache: AtomicBool::new(false),
        }
    }
}

///
/// Linear histograms.
///
///
/// Linear histograms classify numeric integer values into same-sized
/// buckets. This type is typically used for percentages, or to store a
/// relatively precise of the amount of resources (time, memory) used
/// by a section.
///
///
/// With `SerializationFormat::SimpleJson`, these histograms are
/// serialized as an array of numbers, one per bucket, in the numeric
/// order of buckets.
pub struct Linear<T> where T: Flatten {
    witness: PhantomData<T>,
    back_end: BackEnd<Single>,
}

impl<T> Histogram<T> for Linear<T> where T: Flatten {
    fn record_cb<F>(&self, cb: F) where F: FnOnce() -> Option<T>  {
        if let Some(k) = self.back_end.get_key() {
            match cb() {
                None => {}
                Some(v) => self.back_end.raw_record(&k, v.as_u32())
            }
        }
    }
}

impl<T> Linear<T> where T: Flatten {
    pub fn new(feature: &Feature, name: String, min: u32, max: u32, buckets: usize) -> Linear<T> {
        assert!(size_of::<u32>() <= size_of::<usize>());
        assert!(min < max);
        assert!(max - min >= buckets as u32);
        let shape = LinearBuckets { min: min, max: max, buckets: buckets };
        let storage = Box::new(LinearStorage::new(shape));
        let key = PrivateAccess::register_single(feature, name, storage);
        Linear {
            witness: PhantomData,
            back_end: BackEnd::new(feature, key),
        }
    }
}

struct LinearStorage {
    values: Vec<u32>,// We cannot use an array here, as this would make the struct unsized.
    shape: LinearBuckets,
}


impl LinearStorage {
    fn new(shape: LinearBuckets) -> LinearStorage {
        let vec = vec_with_size(shape.buckets, 0);
        LinearStorage {
            values: vec,
            shape: shape,
        }
    }
}

impl SingleRawStorage for LinearStorage {
    fn store(&mut self, value: u32) {
        let index = self.shape.get_bucket(value);
        self.values[index] += 1;
    }
    fn serialize(&self, _: &SerializationFormat) -> Json {
        unreachable!() // FIXME: Implement
    }
}

