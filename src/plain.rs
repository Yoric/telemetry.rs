//!
//! Definition of histograms.
//!
//! Histograms represent measures on a set of entities known at
//! compile-time. In some cases, the set of entities is not known
//! at compile-time (e.g. plug-ins, dates), in which case you should
//! rather use [Keyed histograms](../keyed.index.html).
//!

use rustc_serialize::json::Json;

use std::marker::PhantomData;
use std::mem::size_of;
use std::sync::atomic::{AtomicBool, Ordering};

use misc::{Flatten, LinearBuckets, SerializationFormat, vec_with_size};
use task::{BackEnd, Op, PlainRawStorage};
use service::{Service, PrivateAccess};
use indexing::*;

///
/// A plain histogram.
///
pub trait Histogram<T> {
    ///
    /// Record a value in this histogram.
    ///
    /// If the service is currently inactive, this is a noop.
    ///
    fn record(&self, value: T) {
        self.record_cb(|| Some(value))
    }

    ///
    /// Record a value in this histogram, as provided by a callback.
    ///
    /// If the service is currently inactive, this is a noop.
    ///
    /// If the callback returns `None`, no value is recorded.
    ///
    fn record_cb<F>(&self, _: F) where F: FnOnce() -> Option<T>;
}


/// Back-end features specific to plain histograms.
impl BackEnd<Plain> {
    /// Instruct the Telemetry Task to record a value in an
    /// already registered histogram.
    fn raw_record(&self, k: &Key<Plain>, value: u32) {
        self.sender.send(Op::RecordPlain(k.index, value)).unwrap();
    }

    /// Instruct the Telemetry Task to record the result of a callback
    /// in an already registered histogram.
    fn raw_record_cb<F, T>(&self, cb: F) -> bool where F: FnOnce() -> Option<T>, T: Flatten {
        if let Some(k) = self.get_key() {
            if let Some(v) = cb() {
                self.raw_record(&k, v.as_u32());
                true
            } else {
                false
            }
        } else {
            false
        }
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
/// serialized as a plain number 0 (unset)/1 (set).
///
pub struct Flag {
    back_end: BackEnd<Plain>,

    /// A cache used to avoid spamming the Task once the flag has been set.
    cache: AtomicBool,
}

/// The storage, owned by the Telemetry Task.
struct FlagStorage {
    /// `true` once we have called `record`, `false` until then.
    encountered: bool
}

impl PlainRawStorage for FlagStorage {
    fn store(&mut self, _: u32) {
        self.encountered = true;
    }
    fn to_json(&self, format: &SerializationFormat) -> Json {
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
        if self.back_end.raw_record_cb(cb) {
            self.cache.store(true, Ordering::Relaxed);
        }
    }
}


impl Flag {
    pub fn new(feature: &Service, name: String) -> Flag {
        let storage = Box::new(FlagStorage { encountered: false });
        let key = PrivateAccess::register_plain(feature, name, storage);
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
    back_end: BackEnd<Plain>,
}

impl<T> Histogram<T> for Linear<T> where T: Flatten {
    fn record_cb<F>(&self, cb: F) where F: FnOnce() -> Option<T>  {
        self.back_end.raw_record_cb(cb);
    }
}

impl<T> Linear<T> where T: Flatten {
    pub fn new(feature: &Service, name: String, min: u32, max: u32, buckets: usize) -> Linear<T> {
        assert!(size_of::<u32>() <= size_of::<usize>());
        assert!(min < max);
        assert!(max - min >= buckets as u32);
        let shape = LinearBuckets::new(min, max, buckets);
        let storage = Box::new(LinearStorage::new(shape));
        let key = PrivateAccess::register_plain(feature, name, storage);
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

impl PlainRawStorage for LinearStorage {
    fn store(&mut self, value: u32) {
        let index = self.shape.get_bucket(value);
        self.values[index] += 1;
    }
    fn to_json(&self, _: &SerializationFormat) -> Json {
        Json::Array(self.values.iter().map(|&x| Json::I64(x as i64)).collect())
    }
}

///
///
/// Count histograms.
///
/// A Count histogram simply accumulates the numbers passed with
/// `record()`. Count histograms are useful, for instance, to know how
/// many times a feature has been used, or how many times an error has
/// been triggered.
///
///
/// With `SerializationFormat::SimpleJson`, these histograms are
/// serialized as a plain number.
///
pub struct Count {
    back_end: BackEnd<Plain>,
}

// The storage, owned by the Telemetry Task.
struct CountStorage {
    value: u32
}

impl PlainRawStorage for CountStorage {
    fn store(&mut self, value: u32) {
        self.value += value;
    }
    fn to_json(&self, format: &SerializationFormat) -> Json {
        match format {
            &SerializationFormat::SimpleJson => {
                Json::I64(self.value as i64)
            }
        }
    }
}

impl Histogram<u32> for Count {
    fn record_cb<F>(&self, cb: F) where F: FnOnce() -> Option<u32>  {
        self.back_end.raw_record_cb(cb);
    }
}


impl Count {
    pub fn new(feature: &Service, name: String) -> Count {
        let storage = Box::new(CountStorage { value: 0 });
        let key = PrivateAccess::register_plain(feature, name, storage);
        Count {
            back_end: BackEnd::new(feature, key),
        }
    }
}



///
///
/// Enumerated histograms.
///
/// Enumerated histogram generalize Count histograms to families of
/// keys known at compile-time. They are useful, for instance, to know
/// how often users have picked a specific choice from several, or how
/// many times each kind of error has been triggered, etc.
///
///
/// With `SerializationFormat::SimpleJson`, these histograms are
/// serialized as an array of numbers, in the order of enum values.
///
pub struct Enum<K> where K: Flatten {
    witness: PhantomData<K>,
    back_end: BackEnd<Plain>,
}

// The storage, owned by the Telemetry Task.
struct EnumStorage {
    values: Vec<u32>
}

impl PlainRawStorage for EnumStorage {
    fn store(&mut self, value: u32) {
        self.values[value as usize] += 1;
    }
    fn to_json(&self, format: &SerializationFormat) -> Json {
        match format {
            &SerializationFormat::SimpleJson => {
                Json::Array(self.values.iter().map(|&x| Json::I64(x as i64)).collect())
            }
        }
    }
}

impl<K> Histogram<K> for Enum<K> where K: Flatten {
    fn record_cb<F>(&self, cb: F) where F: FnOnce() -> Option<K>  {
        self.back_end.raw_record_cb(cb);
    }
}


impl<K> Enum<K> where K: Flatten {
    pub fn new(feature: &Service, name: String, buckets: usize) -> Enum<K> {
        let storage = Box::new(EnumStorage { values: vec_with_size(buckets, 0) });
        let key = PrivateAccess::register_plain(feature, name, storage);
        Enum {
            witness: PhantomData,
            back_end: BackEnd::new(feature, key),
        }
    }
}
