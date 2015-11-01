//!
//! Definition of histograms.
//!
//! Histograms represent measures on a set of entities known at
//! compile-time. In some cases, the set of entities is not known
//! at compile-time (e.g. plug-ins, dates), in which case you should
//! rather use [Keyed histograms](../keyed.index.html).
//!

use rustc_serialize::json::Json;

use std::borrow::Cow;
use std::marker::PhantomData;
use std::mem::size_of;
use std::sync::atomic::{AtomicBool, Ordering};

use misc::{Flatten, HistogramType, LinearBuckets, LinearStats, MozillaIntermediateFormat, vec_resize, vec_with_size};
use task::{BackEnd, Op, PlainRawStorage};
use service::{Service, PrivateAccess};
use indexing::*;

///
/// A plain histogram.
///
/// Histograms do not implement `Sync`, so an instance of `Histogram`
/// cannot be shared by several threads. However, any histogram can be
/// cloned as needed for concurrent use.
///
/// # Performance
////
/// Cloning a histogram is relatively cheap, both in terms of memory
/// and in terms of speed (most histograms weigh ~40bytes on a x86-64
/// architecture).
///
/// When the telemetry service is inactive, recording data to a
/// histogram is very fast (essentially a dereference and an atomic
/// fetch). When the telemetry service is active, the duration of
/// recording data is comparable to the duration of sending a simple
/// message to a `Sender`.
///
pub trait Histogram<T> : Clone {
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
    ///
    /// Create an histogram that ignores any input.
    ///
    /// `Ignoring` histograms are effectively implemented as empty
    /// structs, without a back-end, so they take no memory.
    ///
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

impl<T> Clone for Ignoring<T> {
    fn clone(&self) -> Self {
        Ignoring {
            witness: PhantomData
        }
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

    fn to_simple_json(&self) -> Json {
        Json::I64(if self.encountered { 1 } else { 0 })
    }

    fn to_moz_intermediate_format<'a>(&'a self) -> MozillaIntermediateFormat<'a> {
        let mut vec = Vec::with_capacity(1);
        vec.push(if self.encountered { 1 } else { 0 });
        MozillaIntermediateFormat {
            min: 0,
            max: 1,
            bucket_count: 1,
            counts: Cow::Owned(vec),
            histogram_type: HistogramType::Flag,
            linear: None,
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
    ///
    /// Create a new Flag histogram with a given name.
    ///
    /// Argument `name` is used as key when processing and exporting
    /// the data. Each `name` must be unique to the `Service`.
    ///
    /// # Panics
    ///
    /// If `name` is already used by another histogram in `service`.
    ///
    pub fn new(service: &Service, name: String) -> Flag {
        let storage = Box::new(FlagStorage { encountered: false });
        let key = PrivateAccess::register_plain(service, name, storage);
        Flag {
            back_end: BackEnd::new(service, key),
            cache: AtomicBool::new(false),
        }
    }
}

impl Clone for Flag {
    fn clone(&self) -> Self {
        Flag {
            back_end: self.back_end.clone(),
            // The cache is not shared, but that's ok, it's just an
            // optimization.
            cache: AtomicBool::new(self.cache.load(Ordering::Relaxed)),
        }
    }
}

///
/// Linear histograms.
///
///
/// Linear histograms classify numeric integer values into same-sized
/// buckets. This type is typically used for percentages, or to store
/// a relatively precise approximation of the amount of resources
/// (time, memory) used by a section or a data structure.
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
    ///
    /// Create a new Linear histogram with a given name.
    ///
    /// - `name` is used as key when processing and exporting
    /// the data. Each `name` must be unique to the `Service`.
    ///
    /// - `min` is the minimal value expected to be entered in this
    /// histogram. Any value lower than `min` is rounded up to `min`.
    ///
    /// - `max` is the maximal value expected to be entered in this
    /// histogram. Any value higher than `max` is rounded up to `max`.
    ///
    /// - `buckets` is the number of buckets in this histogram. For
    /// highest possible precision, use `buckets = max - min + 1`.
    /// In most cases, however, such precision is not needed, so you
    /// should use a lower number of buckets.
    ///
    ///
    /// # Performance
    ///
    /// Increasing the number of buckets increases the memory usage on
    /// the client by a few bytes per bucket. More importantly, it also
    /// increases the size of the payload, hence the total amount of
    /// data that the application will eventually upload to a central
    /// server. If your application has many clients and you wish to
    /// keep your server happy and your bandwidth costs manageable,
    /// don't use too many buckets.
    ///
    ///
    /// # Panics
    ///
    /// If `name` is already used by another histogram in `service`.
    ///
    /// If `min >= max`.
    ///
    /// If `buckets < max - min + 1`.
    ///
    pub fn new(service: &Service, name: String, min: u32, max: u32, buckets: usize) -> Linear<T> {
        assert!(size_of::<u32>() <= size_of::<usize>());
        assert!(min < max);
        assert!(max - min + 1 >= buckets as u32);
        let shape = LinearBuckets::new(min, max, buckets);
        let storage = Box::new(LinearStorage::new(shape));
        let key = PrivateAccess::register_plain(service, name, storage);
        Linear {
            witness: PhantomData,
            back_end: BackEnd::new(service, key),
        }
    }
}

struct LinearStorage {
    values: Vec<u32>,// We cannot use an array here, as this would make the struct unsized.
    shape: LinearBuckets,
    stats: LinearStats
}


impl LinearStorage {
    fn new(shape: LinearBuckets) -> LinearStorage {
        let vec = vec_with_size(shape.get_bucket_count(), 0);
        LinearStorage {
            values: vec,
            shape: shape,
            stats: LinearStats::new(),
        }
    }
}

impl PlainRawStorage for LinearStorage {
    fn store(&mut self, value: u32) {
        let index = self.shape.get_bucket(value);
        self.values[index] += 1;
        self.stats.record(value);
    }

    fn to_simple_json(&self) -> Json {
        Json::Array(self.values.iter().map(|&x| Json::I64(x as i64)).collect())
    }

    fn to_moz_intermediate_format<'a>(&'a self) -> MozillaIntermediateFormat<'a> {
        MozillaIntermediateFormat {
            histogram_type: HistogramType::Linear,
            min: self.shape.get_min() as i64,
            max: self.shape.get_max() as i64,
            bucket_count: self.shape.get_bucket_count() as i64,
            linear: Some(&self.stats),
            counts: Cow::Borrowed(&self.values)
        }
    }
}

impl<T> Clone for Linear<T> where T: Flatten {
    fn clone(&self) -> Self {
        Linear {
            witness: PhantomData,
            back_end: self.back_end.clone()
        }
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
#[derive(Clone)]
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
    fn to_simple_json(&self) -> Json {
        Json::I64(self.value as i64)
    }
    fn to_moz_intermediate_format<'a>(&'a self) -> MozillaIntermediateFormat<'a> {
        let mut vec = Vec::with_capacity(1);
        vec.push(self.value);
        MozillaIntermediateFormat {
            min: 0, // Following the original implementation.
            max: 2, // Following the original implementation.
            bucket_count: 1,
            counts: Cow::Owned(vec),
            histogram_type: HistogramType::Count,
            linear: None,
        }
    }

}


impl Histogram<u32> for Count {
    fn record_cb<F>(&self, cb: F) where F: FnOnce() -> Option<u32>  {
        self.back_end.raw_record_cb(cb);
    }
}


impl Count {
    ///
    /// Create a new Count histogram with a given name.
    ///
    /// Argument `name` is used as key when processing and exporting
    /// the data. Each `name` must be unique to the `Service`.
    ///
    /// # Panics
    ///
    /// If `name` is already used by another histogram in `service`.
    ///
    pub fn new(service: &Service, name: String) -> Count {
        let storage = Box::new(CountStorage { value: 0 });
        let key = PrivateAccess::register_plain(service, name, storage);
        Count {
            back_end: BackEnd::new(service, key),
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
    values: Vec<u32>,
    stats: LinearStats,
    nbuckets: u32,
}

impl PlainRawStorage for EnumStorage {
    fn store(&mut self, value: u32) {
        vec_resize(&mut self.values, value as usize + 1, 0);
        self.values[value as usize] += 1;
        self.stats.record(value);
    }
    fn to_simple_json(&self) -> Json {
        Json::Array(self.values.iter().map(|&x| Json::I64(x as i64)).collect())
    }
    fn to_moz_intermediate_format<'a>(&'a self) -> MozillaIntermediateFormat<'a> {
        MozillaIntermediateFormat {
            min: 0,
            max: self.nbuckets as i64,
            bucket_count: self.nbuckets as i64,
            counts: Cow::Borrowed(&self.values),
            histogram_type: HistogramType::Linear,
            linear: Some(&self.stats),
        }
    }
}

impl<K> Histogram<K> for Enum<K> where K: Flatten {
    fn record_cb<F>(&self, cb: F) where F: FnOnce() -> Option<K>  {
        self.back_end.raw_record_cb(cb);
    }
}


impl<K> Enum<K> where K: Flatten {
    ///
    /// Create a new Enum histogram with a given name.
    ///
    /// Argument `name` is used as key when processing and exporting
    /// the data. Each `name` must be unique to the `Service`.
    ///
    /// # Panics
    ///
    /// If `name` is already used by another histogram in `service`.
    ///
    pub fn new(service: &Service, name: String, nbuckets: u32) -> Enum<K> {
        let storage = Box::new(EnumStorage {
            values: Vec::new(),
            stats: LinearStats::new(),
            nbuckets: nbuckets,
        });
        let key = PrivateAccess::register_plain(service, name, storage);
        Enum {
            witness: PhantomData,
            back_end: BackEnd::new(service, key),
        }
    }
}

impl<K> Clone for Enum<K> where K: Flatten {
    fn clone(&self) -> Self {
        Enum {
            witness: PhantomData,
            back_end: self.back_end.clone()
        }
    }
}
