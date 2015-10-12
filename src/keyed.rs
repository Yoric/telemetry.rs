//!
//! Definition of keyed (aka "dynamic family of") histograms.
//!
//! Keyed histograms represent measures on a set of entities known
//! only at runtime, e.g. plug-ins, user scripts, etc. They are also
//! slower, more memory-consuming and less type-safe than Single
//! histograms, so you should prefer the latter whenever possible.
//!

use rustc_serialize::json::Json;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::marker::PhantomData;
use std::mem::size_of;

use misc::{Flatten, LinearBuckets, SerializationFormat, vec_with_size};
use task::{BackEnd, Op, KeyedRawStorage};
use service::{Feature, PrivateAccess};
use indexing::*;

//
// A family of histograms, indexed by some value. Use these to
// monitor families of values that cannot be determined at
// compile-time, e.g. add-ons, programs, etc.
//
pub trait KeyedHistogram<K, T> {
    //
    // Record a value in this histogram.
    //
    // The value is recorded only if all of the following conditions are met:
    // - telemetry is activated; and
    // - this histogram has not expired; and
    // - the histogram is active.
    //
    fn record(&self, key: K, value: T) {
        self.record_cb(|| Some((key, value)))
    }

    //
    // Record a value in this histogram, as provided by a callback.
    //
    // The callback is triggered only if all of the following conditions are met:
    // - `telemetry` is activated; and
    // - this histogram has not expired; and
    // - the histogram is active.
    //
    // If the callback returns `None`, no value is recorded.
    //
    fn record_cb<F>(&self, _: F) where F: FnOnce() -> Option<(K, T)>;
}

// Back-end features specific to keyed histograms.
impl<T> BackEnd<Keyed<T>> {
    fn raw_record(&self, k: &Key<Keyed<T>>, key: String, value: u32) {
        self.sender.send(Op::RecordKeyed(k.index, key, value)).unwrap();
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
    pub fn new() -> KeyedIgnoring<T, U> {
        KeyedIgnoring {
            witness: PhantomData
        }
    }
}

impl<K, T> KeyedHistogram<K, T> for KeyedIgnoring<K, T> {
    fn record_cb<F>(&self, _: F) where F: FnOnce() -> Option<(K, T)>  {
        return;
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
    back_end: BackEnd<Keyed<T>>
}


impl<K> KeyedFlag<K> where K: ToString {
    pub fn new(feature: &Feature, name: String) -> KeyedFlag<K> {
        let storage = Box::new(KeyedFlagStorage { encountered: HashSet::new() });
        let key = PrivateAccess::register_keyed(feature, name, storage);
        KeyedFlag {
            back_end: BackEnd::new(feature, key),
        }
    }
}

struct KeyedFlagStorage {
    encountered: HashSet<String>
}

impl KeyedRawStorage for KeyedFlagStorage {
    fn store(&mut self, k: String, _: u32) {
        self.encountered.insert(k);
    }
    fn to_json(&self, format: &SerializationFormat) -> Json {
        match format {
            &SerializationFormat::SimpleJson => {
                // Collect and sort the keys.
                let mut keys : Vec<&String> = self.encountered.iter().collect();
                keys.sort();
                let array = keys.iter().map(|&x| Json::String(x.clone())).collect();
                Json::Array(array)
            }
        }
    }
}

impl<K> KeyedHistogram<K, ()> for KeyedFlag<K> where K: ToString {
    fn record_cb<F>(&self, cb: F) where F: FnOnce() -> Option<(K, ())>  {
        if let Some(k) = self.back_end.get_key() {
            match cb() {
                None => {}
                Some((key, ())) => self.back_end.raw_record(&k, key.to_string(), 0)
            }
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
pub struct KeyedLinear<K, T> where T: Flatten {
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
            shape: shape
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
        let mut values : Vec<_> = self.values.iter().collect();
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


impl<K, T> KeyedLinear<K, T> where K: ToString, T: Flatten {
    pub fn new(feature: &Feature, name: String, min: u32, max: u32, buckets: usize) -> KeyedLinear<K, T> {
        assert!(size_of::<u32>() <= size_of::<usize>());
        assert!(min < max);
        assert!(max - min >= buckets as u32);
        let shape = KeyedLinearBuckets { min: min, max: max, buckets: buckets };
        let storage = Box::new(KeyedLinearStorage::new(shape));
        let key = PrivateAccess::register_keyed(feature, name, storage);
        KeyedLinear {
            witness: PhantomData,
            back_end: BackEnd::new(feature, key),
        }
    }
}

impl<K, T> KeyedHistogram<K, T> for KeyedLinear<K, T> where K: ToString, T: Flatten {
    fn record_cb<F>(&self, cb: F) where F: FnOnce() -> Option<(K, T)>  {
        if let Some(k) = self.back_end.get_key() {
            match cb() {
                None => {}
                Some((key, v)) => self.back_end.raw_record(&k, key.to_string(), v.as_u32())
            }
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
                let mut values : Vec<_> = self.values.iter().collect();
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

impl<K> KeyedHistogram<K, u32> for KeyedCount<K> where K: ToString {
    fn record_cb<F>(&self, cb: F) where F: FnOnce() -> Option<(K, u32)>  {
        if let Some(k) = self.back_end.get_key() {
            match cb() {
                None => {}
                Some((key, v)) => {
                    self.back_end.raw_record(&k, key.to_string(), v)
                }
            }
        }
    }
}


impl<K> KeyedCount<K> {
    pub fn new(feature: &Feature, name: String) -> KeyedCount<K> {
        let storage = Box::new(KeyedCountStorage { values: HashMap::new() });
        let key = PrivateAccess::register_keyed(feature, name, storage);
        KeyedCount {
            back_end: BackEnd::new(feature, key),
        }
    }
}
