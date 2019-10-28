//!
//! Definition of keyed (aka "dynamic family of") histograms.
//!
//! Keyed histograms represent measures on a set of entities known
//! only at runtime, e.g. plug-ins, user scripts, etc. They are also
//! slower, more memory-consuming and less type-safe than Plain
//! histograms, so you should prefer the latter whenever possible.
//!

use rustc_serialize::json::Json;

use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::marker::PhantomData;
use std::mem::size_of;

use indexing::*;
use misc::{vec_resize, vec_with_size, Flatten, LinearBuckets, SerializationFormat};
use service::{PrivateAccess, Service};
use task::{BackEnd, KeyedRawStorage, Op};

///
/// A family of histograms, indexed by some dynamic value. Use these
/// to monitor families of values that cannot be determined at
/// compile-time, e.g. add-ons, programs, etc.
///
/// Histograms do not implement `Sync`, so an instance of
/// `KeyedHistogram` cannot be shared by several threads. However, any
/// histogram can be cloned as needed for concurrent use.
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
pub trait KeyedHistogram<K, T>: Clone {
    ///
    /// Record a value in this histogram.
    ///
    /// If the service is currently inactive, this is a noop.
    ///
    fn record(&self, key: K, value: T) {
        self.record_cb(|| Some((key, value)))
    }

    ///
    /// Record a value in this histogram, as provided by a callback.
    ///
    /// If the service is currently inactive, this is a noop.
    ///
    /// If the callback returns `None`, no value is recorded.
    ///
    fn record_cb<F>(&self, _: F)
    where
        F: FnOnce() -> Option<(K, T)>;
}

/// Back-end features specific to keyed histograms.
impl<K> BackEnd<Keyed<K>>
where
    K: ToString,
{
    /// Instruct the Telemetry Task to record a value in an
    /// already registered histogram.
    fn raw_record(&self, k: &Key<Keyed<K>>, key: String, value: u32) {
        self.sender
            .send(Op::RecordKeyed(k.index, key, value))
            .unwrap();
    }

    /// Instruct the Telemetry Task to record the result of a callback
    /// in an already registered histogram.
    fn raw_record_cb<F, T>(&self, cb: F) -> bool
    where
        F: FnOnce() -> Option<(K, T)>,
        T: Flatten,
    {
        if let Some(k) = self.get_key() {
            if let Some((key, v)) = cb() {
                self.raw_record(&k, key.to_string(), v.as_u32());
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
pub struct KeyedIgnoring<T, U> {
    witness: PhantomData<(T, U)>,
}

impl<T, U> KeyedIgnoring<T, U> {
    //
    // Create an histogram that ignores any input.
    //
    // `KeyedIgnoring` histograms are effectively implemented as empty
    // structs, without a back-end, so they take no memory.
    //
    pub fn new() -> KeyedIgnoring<T, U> {
        KeyedIgnoring {
            witness: PhantomData,
        }
    }
}

impl<K, T> KeyedHistogram<K, T> for KeyedIgnoring<K, T> {
    fn record_cb<F>(&self, _: F)
    where
        F: FnOnce() -> Option<(K, T)>,
    {
        return;
    }
}

impl<T, U> Clone for KeyedIgnoring<T, U> {
    fn clone(&self) -> Self {
        KeyedIgnoring {
            witness: PhantomData,
        }
    }
}

///
///
/// Flag histograms.
///
/// Each entry has only two states. Until the first call to
/// `record()`, it is _unset_. Once `record()` has been called once,
/// it is _set_ and won't change anymore. This type is useful if you
/// need to track whether a feature was ever used during a session.
///
///
/// With `SerializationFormat::SimpleJson`, these histograms are
/// serialized as an array of the keys with which it was called.
/// Keys are sorted by alphabetical order, and appear only once.
///
pub struct KeyedFlag<T> {
    back_end: BackEnd<Keyed<T>>,
}

impl<K> KeyedFlag<K>
where
    K: ToString,
{
    pub fn new(service: &Service, name: String) -> KeyedFlag<K> {
        let storage = Box::new(KeyedFlagStorage {
            encountered: HashSet::new(),
        });
        let key = PrivateAccess::register_keyed(service, name, storage);
        KeyedFlag {
            back_end: BackEnd::new(service, key),
        }
    }
}

struct KeyedFlagStorage {
    encountered: HashSet<String>,
}

impl KeyedRawStorage for KeyedFlagStorage {
    fn store(&mut self, k: String, _: u32) {
        self.encountered.insert(k);
    }
    fn to_json(&self, format: &SerializationFormat) -> Json {
        match format {
            &SerializationFormat::SimpleJson => {
                // Collect and sort the keys.
                let mut keys: Vec<&String> = self.encountered.iter().collect();
                keys.sort();
                let array = keys.iter().map(|&x| Json::String(x.clone())).collect();
                Json::Array(array)
            }
        }
    }
}

impl<K> KeyedHistogram<K, ()> for KeyedFlag<K>
where
    K: ToString,
{
    fn record_cb<F>(&self, cb: F)
    where
        F: FnOnce() -> Option<(K, ())>,
    {
        self.back_end.raw_record_cb(cb);
    }
}

impl<T> Clone for KeyedFlag<T> {
    fn clone(&self) -> Self {
        KeyedFlag {
            back_end: self.back_end.clone(),
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
/// serialized as an object
/// ````js
/// {
///   key_1: array_1,
///   key_2: array_2,
///   ...
/// }
/// ````
///
/// where each `array_i` is an array of numbers, one per bucket, in
/// the numeric order of buckets
///
pub struct KeyedLinear<K, T>
where
    T: Flatten,
{
    witness: PhantomData<T>,
    back_end: BackEnd<Keyed<K>>,
}

type KeyedLinearBuckets = LinearBuckets;

struct KeyedLinearStorage {
    values: HashMap<String, Vec<u32>>,
    shape: KeyedLinearBuckets,
}

impl KeyedLinearStorage {
    fn new(shape: KeyedLinearBuckets) -> KeyedLinearStorage {
        KeyedLinearStorage {
            values: HashMap::new(),
            shape: shape,
        }
    }
}

impl KeyedRawStorage for KeyedLinearStorage {
    fn store(&mut self, key: String, value: u32) {
        let index = self.shape.get_bucket(value);
        match self.values.entry(key) {
            Occupied(mut e) => {
                e.get_mut()[index] += 1;
            }
            Vacant(e) => {
                let mut vec = vec_with_size(self.shape.buckets, 0);
                vec[index] += 1;
                e.insert(vec);
            }
        }
    }
    fn to_json(&self, _: &SerializationFormat) -> Json {
        // Sort keys, for easier testing/comparison.
        let mut values: Vec<_> = self.values.iter().collect();
        values.sort();
        // Turn everything into an object.
        let mut tree = BTreeMap::new();
        for value in values {
            let (name, vec) = value;
            let array = Json::Array(vec.iter().map(|&x| Json::I64(x as i64)).collect());
            tree.insert(name.clone(), array);
        }
        Json::Object(tree)
    }
}

impl<K, T> KeyedLinear<K, T>
where
    K: ToString,
    T: Flatten,
{
    ///
    /// Create a new Linear histogram with a given name.
    ///
    /// Argument `name` is used as key when processing and exporting
    /// the data. Each `name` must be unique to the `Service`.
    ///
    /// `min` is the minimal value expected to be entered in this
    /// histogram. Any value lower than `min` is rounded up to `min`.
    ///
    /// `max` is the maximal value expected to be entered in this
    /// histogram. Any value higher than `max` is rounded up to `max`.
    ///
    /// `buckets` is the number of buckets in this histogram. For
    /// highest possible precision, use `buckets = max - min + 1`.
    /// In most cases, however, such precision is not needed, so you
    /// should use a lower number of buckets.
    ///
    ///
    /// # Performance
    ///
    /// Increasing the number of buckets increases the memory usage on
    /// the client by a few bytes per bucket per key. More
    /// importantly, it also increases the size of the payload, hence
    /// the total amount of data that the application will eventually
    /// upload to a central server. If your application has many
    /// clients and you wish to keep your server happy and your
    /// bandwidth costs manageable, don't use too many buckets.
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
    pub fn new(
        service: &Service,
        name: String,
        min: u32,
        max: u32,
        buckets: usize,
    ) -> KeyedLinear<K, T> {
        assert!(size_of::<u32>() <= size_of::<usize>());
        assert!(min < max);
        assert!(max - min >= buckets as u32);
        let shape = KeyedLinearBuckets::new(min, max, buckets);
        let storage = Box::new(KeyedLinearStorage::new(shape));
        let key = PrivateAccess::register_keyed(service, name, storage);
        KeyedLinear {
            witness: PhantomData,
            back_end: BackEnd::new(service, key),
        }
    }
}

impl<K, T> KeyedHistogram<K, T> for KeyedLinear<K, T>
where
    K: ToString,
    T: Flatten,
{
    fn record_cb<F>(&self, cb: F)
    where
        F: FnOnce() -> Option<(K, T)>,
    {
        self.back_end.raw_record_cb(cb);
    }
}

impl<K, T> Clone for KeyedLinear<K, T>
where
    T: Flatten,
{
    fn clone(&self) -> Self {
        KeyedLinear {
            back_end: self.back_end.clone(),
            witness: PhantomData,
        }
    }
}

///
///
/// Count histograms.
///
/// A Count histogram simply accumulates the numbers passed with `record()`.
///
///
/// With `SerializationFormat::SimpleJson`, these histograms are
/// serialized as an object, with keys sorted, in which each field is
/// a number.
///
pub struct KeyedCount<K> {
    back_end: BackEnd<Keyed<K>>,
}

// The storage, owned by the Telemetry Task.
struct KeyedCountStorage {
    values: HashMap<String, u32>,
}

impl KeyedRawStorage for KeyedCountStorage {
    fn store(&mut self, key: String, value: u32) {
        match self.values.entry(key) {
            Occupied(mut e) => {
                let v = e.get().clone();
                e.insert(v + value);
            }
            Vacant(e) => {
                e.insert(value);
            }
        }
    }
    fn to_json(&self, format: &SerializationFormat) -> Json {
        match format {
            &SerializationFormat::SimpleJson => {
                // Sort keys, for easier testing/comparison.
                let mut values: Vec<_> = self.values.iter().collect();
                values.sort();
                // Turn everything into an object.
                let mut tree = BTreeMap::new();
                for value in values {
                    let (name, val) = value;
                    tree.insert(name.clone(), Json::I64(val.clone() as i64));
                }
                Json::Object(tree)
            }
        }
    }
}

impl<K> KeyedHistogram<K, u32> for KeyedCount<K>
where
    K: ToString,
{
    fn record_cb<F>(&self, cb: F)
    where
        F: FnOnce() -> Option<(K, u32)>,
    {
        self.back_end.raw_record_cb(cb);
    }
}

impl<K> KeyedCount<K> {
    ///
    /// Create a new KeyedCount histogram with a given name.
    ///
    /// Argument `name` is used as key when processing and exporting
    /// the data. Each `name` must be unique to the `Service`.
    ///
    /// # Panics
    ///
    /// If `name` is already used by another histogram in `service`.
    ///
    pub fn new(service: &Service, name: String) -> KeyedCount<K> {
        let storage = Box::new(KeyedCountStorage {
            values: HashMap::new(),
        });
        let key = PrivateAccess::register_keyed(service, name, storage);
        KeyedCount {
            back_end: BackEnd::new(service, key),
        }
    }
}

impl<K> Clone for KeyedCount<K> {
    fn clone(&self) -> Self {
        KeyedCount {
            back_end: self.back_end.clone(),
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
/// serialized as an object, one field per key (sorted), with value an
/// array of numbers, in the order of enum values.
///
pub struct KeyedEnum<K, T>
where
    K: ToString,
    T: Flatten,
{
    witness: PhantomData<T>,
    back_end: BackEnd<Keyed<K>>,
}

// The storage, owned by the Telemetry Task.
struct KeyedEnumStorage {
    values: HashMap<String, Vec<u32>>,
}

impl KeyedRawStorage for KeyedEnumStorage {
    fn store(&mut self, key: String, value: u32) {
        match self.values.entry(key) {
            Occupied(mut e) => {
                let mut vec = e.get_mut();
                vec_resize(&mut vec, value as usize + 1, 0);
                vec[value as usize] += 1;
            }
            Vacant(e) => {
                let mut vec = Vec::new();
                vec_resize(&mut vec, value as usize + 1, 0);
                vec[value as usize] = 1;
                e.insert(vec);
            }
        }
    }
    fn to_json(&self, format: &SerializationFormat) -> Json {
        match format {
            &SerializationFormat::SimpleJson => {
                // Sort keys, for easier testing/comparison.
                let mut values: Vec<_> = self.values.iter().collect();
                values.sort();
                // Turn everything into an object.
                let mut tree = BTreeMap::new();
                for value in values {
                    let (name, array) = value;
                    let vec = array.iter().map(|&x| Json::I64(x.clone() as i64)).collect();
                    tree.insert(name.clone(), Json::Array(vec));
                }
                Json::Object(tree)
            }
        }
    }
}

impl<K, T> KeyedHistogram<K, T> for KeyedEnum<K, T>
where
    K: ToString,
    T: Flatten,
{
    ///
    /// Record a value.
    ///
    /// Actual recording takes place on the background thread.
    ///
    fn record_cb<F>(&self, cb: F)
    where
        F: FnOnce() -> Option<(K, T)>,
    {
        self.back_end.raw_record_cb(cb);
    }
}

impl<K, T> KeyedEnum<K, T>
where
    K: ToString,
    T: Flatten,
{
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
    pub fn new(service: &Service, name: String) -> KeyedEnum<K, T> {
        let storage = Box::new(KeyedEnumStorage {
            values: HashMap::new(),
        });
        let key = PrivateAccess::register_keyed(service, name, storage);
        KeyedEnum {
            witness: PhantomData,
            back_end: BackEnd::new(service, key),
        }
    }
}

impl<K, T> Clone for KeyedEnum<K, T>
where
    K: ToString,
    T: Flatten,
{
    fn clone(&self) -> Self {
        KeyedEnum {
            back_end: self.back_end.clone(),
            witness: PhantomData,
        }
    }
}
