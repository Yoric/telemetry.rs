extern crate rustc_serialize;
use self::rustc_serialize::json::Json;

extern crate vec_map;
use self::vec_map::VecMap;

use std::marker::PhantomData;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::atomic::Ordering::Relaxed;
use std::sync::mpsc::{channel, Sender, Receiver};
use std::thread;
use std::mem::size_of;
use std::sync::Arc;
use std::cell::Cell;

mod misc;
pub use misc::SerializationFormat;
use misc::{NamedStorage};

mod indexing;
use indexing::*;

// Telemetry is a mechanism used to capture metrics in an application,
// to later store the data locally or upload it to a server for
// statistical analysis.
//
// Examples of usage:
// - capturing the speed of an operation;
// - finding out if users are actually using a feature;
// - finding out how the duration of a session;
// - determine the operating system on which the application is executed;
// - determining the configuration of the application;
// - capturing the operations that slow down the application;
// - determining the amount of I/O performed by the application;
// - ...
//
// The abstraction used by this library is the Histogram. Each
// Histogram serves to capture a specific measurement, store it
// locally and/or upload it to the server. Several types of Histograms
// are provided, suited to distinct kinds of measures.
//
//
// Memory note: the memory used by a histogram is recollected only
// when its instance of `telemetry` is garbage-collected. In other words,
// if a histogram goes out of scope for some reason, its data remains
// in telemetry and will be stored and/or uploaded in accordance with the
// configuration of this telemetry instance.
//

//
// A software version, e.g. [2015, 10, 10, 0]
//
pub type Version = [u32;4];

//
// Metadata on a histogram.
//
pub struct Metadata {
    // A key used to identify the histogram. Must be unique to the instance
    // of `telemetry`.
    pub key: String,

    // Optionally, a version of the product at which this histogram expires.
    pub expires: Option<Version>,
}

//
// A single histogram.
//
pub trait SingleHistogram<T> {
    //
    // Record a value in this histogram.
    //
    // The value is recorded only if all of the following conditions are met:
    // - telemetry is activated; and
    // - this histogram has not expired; and
    // - the histogram is active.
    //
    fn record(&self, value: T) {
        self.record_cb(|| Some(value))
    }

    //
    // Record a value in this histogram, as provided by a callback.
    //
    // The callback is triggered only if all of the following conditions are met:
    // - telemetry is activated; and
    // - this histogram has not expired; and
    // - the histogram is active.
    //
    // If the callback returns `None`, no value is recorded.
    //
    fn record_cb<F>(&self, _: F) where F: FnOnce() -> Option<T>;
}

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

pub trait Flatten {
    fn as_u32(&self) -> u32;
}

impl Flatten for u32 {
    fn as_u32(&self) -> u32 {
        *self
    }
}

//
//
// Flag histograms.
//
// This histogram type allows you to record a single value. This type
// is useful if you need to track whether a feature was ever used
// during a session. You only need to add a single line of code which
// sets the flag when the feature is used because the histogram is
// initialized with a default value of false (flag not set).
//
//

// Single histogram, good for recording a single value.
pub struct SingleFlag {
    back_end: BackEnd<Single>,
    cache: AtomicBool,
}

// Map histogram, good for recording the presence of a set of values,
// when the set cannot be known at compile-time. If the set is known
// at compile-time, you should prefer several instances of
// `SingleFlag`.
pub struct KeyedFlag<T> {
    back_end: BackEnd<Keyed<T>>
}

struct SingleFlagStorage {
    // `true` once we have called `record`, `false` until then.
    encountered: bool
}


impl RawStorage for SingleFlagStorage {
    fn store(&mut self, _: u32) {
        self.encountered = true;
    }
    fn serialize(&self, format: &SerializationFormat) -> Json {
        match format {
            Simple => {
                Json::Boolean(self.encountered)
            }
        }
    }
}

impl SingleHistogram<()> for SingleFlag {
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


impl SingleFlag {
    pub fn new(feature: &Feature, meta: Metadata) -> SingleFlag {
        let storage = Box::new(SingleFlagStorage { encountered: false });
        let key = feature.telemetry.register_single(meta, storage);
        SingleFlag {
            back_end: BackEnd::new(feature, key),
            cache: AtomicBool::new(false),
        }
    }
}

// Map histogram, good for a family of values. Note that if the family
// of values is known at compile-time, using a set of `Flag` instead of
// a single `KeyedFlag` is both more efficient and more type-safe.

impl<K> KeyedFlag<K> where K: ToString {
    pub fn new(feature: &Feature, meta: Metadata) -> KeyedFlag<K> {
        let storage = Box::new(KeyedFlagStorage { encountered: HashSet::new() });
        let key = feature.telemetry.register_keyed(meta, storage);
        KeyedFlag {
            back_end: BackEnd::new(feature, key),
        }
    }
}

struct KeyedFlagStorage {
    encountered: HashSet<String>
}

impl RawStorageMap for KeyedFlagStorage {
    fn store(&mut self, k: String, _: u32) {
        self.encountered.insert(k);
    }
    fn serialize(&self, format: &SerializationFormat) -> Json {
        match format {
            Simple => {
                let mut keys = Vec::with_capacity(self.encountered.len());
                for key in &self.encountered {
                    keys.push(Json::String(key.clone())) // FIXME: Why do I need to copy a String for this?
                }
                Json::Array(keys)
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



//
// Linear histograms.
//
//
// Linear histograms classify numeric integer values into same-sized
// buckets. This type is typically used for percentages.
//


struct LinearBuckets {
    min: u32,
    max: u32, // Invariant: max > min
    buckets: usize
}

impl LinearBuckets {
    fn get_bucket(&self, value: u32) -> usize {
        if value >= self.max {
            0
        } else if value <= self.min {
            self.buckets - 1 as usize
        } else {
            let num = value as f32 - self.min as f32;
            let den = self.max as f32 - self.min as f32;
            let res = (num / den) * self.buckets as f32;
            res as usize
        }
    }
}

struct LinearStorage {
    values: Vec<u32>,// We cannot use an array here, as this would make the struct unsized
    shape: LinearBuckets,
}

impl LinearStorage {
    fn new(shape: LinearBuckets) -> LinearStorage {
        let mut vec = Vec::with_capacity(shape.buckets);
        unsafe {
            // Resize. In future versions of Rust, we should
            // be able to use `vec.resize`.
            vec.set_len(shape.buckets);
            for i in 0 .. shape.buckets - 1 {
                vec[i] = 0;
            }
        }
        LinearStorage {
            values: vec,
            shape: shape,
        }
    }
}

impl RawStorage for LinearStorage {
    fn store(&mut self, value: u32) {
        let index = self.shape.get_bucket(value);
        self.values[index] += 1;
    }
    fn serialize(&self, format: &SerializationFormat) -> Json {
        unreachable!() // FIXME: Implement
    }
}

pub struct KeyedLinear<K, T> where T: Flatten {
    witness: PhantomData<T>,
    back_end: BackEnd<indexing::Keyed<K>>,
}

pub struct SingleLinear<T> where T: Flatten {
    witness: PhantomData<T>,
    back_end: BackEnd<indexing::Single>,
}

impl<T> SingleHistogram<T> for SingleLinear<T> where T: Flatten {
    fn record_cb<F>(&self, cb: F) where F: FnOnce() -> Option<T>  {
        if let Some(k) = self.back_end.get_key() {
            match cb() {
                None => {}
                Some(v) => self.back_end.raw_record(&k, v.as_u32())
            }
        }
    }
}

impl<T> SingleLinear<T> where T: Flatten {
    pub fn new(feature: &Feature, meta: Metadata, min: u32, max: u32, buckets: usize) -> SingleLinear<T> {
        assert!(size_of::<u32>() <= size_of::<usize>());
        assert!(min < max);
        assert!(max - min >= buckets as u32);
        let shape = LinearBuckets { min: min, max: max, buckets: buckets };
        let storage = Box::new(LinearStorage::new(shape));
        let key = feature.telemetry.register_single(meta, storage);
        SingleLinear {
            witness: PhantomData,
            back_end: BackEnd::new(feature, key),
        }
    }
}


struct LinearStorageMap {
    values: HashMap<String, Vec<u32>>,
    shape: LinearBuckets,
}

impl LinearStorageMap {
    fn new(shape: LinearBuckets) -> LinearStorageMap {
        LinearStorageMap {
            values: HashMap::new(),
            shape: shape
        }
    }
}

impl RawStorageMap for LinearStorageMap {
    fn store(&mut self, key: String, value: u32) {
        let index = self.shape.get_bucket(value);
        match self.values.entry(key) {
            Occupied(mut e) => {
                e.get_mut()[index] += 1;
            }
            Vacant(e) => {
                let mut vec = Vec::with_capacity(self.shape.buckets);
                unsafe {
                    // Resize. In future versions of Rust, we should
                    // be able to use `vec.resize`.
                    vec.set_len(self.shape.buckets);
                    for i in 0 .. self.shape.buckets - 1 {
                        vec[i] = 0;
                    }
                }
                vec[index] += 1;
                e.insert(vec);
            }
        }
    }
    fn serialize(&self, format: &SerializationFormat) -> Json {
        unreachable!() // FIXME: Implement
    }
}


impl<K, T> KeyedLinear<K, T> where K: ToString, T: Flatten {
    pub fn new(feature: &Feature, meta: Metadata, min: u32, max: u32, buckets: usize) -> KeyedLinear<K, T> {
        assert!(size_of::<u32>() <= size_of::<usize>());
        assert!(min < max);
        assert!(max - min >= buckets as u32);
        let shape = LinearBuckets { min: min, max: max, buckets: buckets };
        let storage = Box::new(LinearStorageMap::new(shape));
        let key = feature.telemetry.register_keyed(meta, storage);
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

//
// A group of histograms observed by Telemetry.
//
impl Feature {
    //
    // Create a new feature.
    //
    // New features are deactivated by default.
    //
    pub fn new(telemetry: &Arc<Service>) -> Feature {
        Feature {
            is_active: Arc::new(Cell::new(false)),
            sender: telemetry.sender.clone(),
            telemetry: telemetry.clone(),
        }
    }
}

//
// The Telemetry service.
//
// Generally, an application will have only a single instance of this
// service but may have any number of instances of `Feature` which may
// be activated and deactivated individually.
//
impl Service {
    pub fn new(version: Version) -> Service {
        let (sender, receiver) = channel();
        thread::spawn(|| {
            let mut task = TelemetryTask::new(receiver);
            task.run()
        });
        Service {
            keys_single: indexing::KeyGenerator::new(),
            keys_keyed: indexing::KeyGenerator::new(),
            version: version,
            sender: sender,
        }
    }

    pub fn serialize(&self, format: SerializationFormat, sender: Sender<(Json, Json)>) {
        self.sender.send(Op::Serialize(format, sender)).unwrap();
    }

    fn register_single(&self, meta: Metadata, storage: Box<RawStorage>) -> Option<indexing::Key<Single>> {
        // Don't bother adding the histogram if it is expired.
        match meta.expires {
            Some(v) if v <= self.version => return None,
            _ => {}
        }

        let key = self.keys_single.next();
        let named = NamedStorage { name: meta.key, contents: storage };
        self.sender.send(Op::RegisterSingle(key.index, named)).unwrap();
        Some(key)
    }

    fn register_keyed<T>(&self, meta: Metadata, storage: Box<RawStorageMap>) -> Option<indexing::Key<Keyed<T>>> {
        // Don't bother adding the histogram if it is expired.
        match meta.expires {
            Some(v) if v <= self.version => return None,
            _ => {}
        }

        let key = self.keys_keyed.next();
        let named = NamedStorage { name: meta.key, contents: storage };
        self.sender.send(Op::RegisterKeyed(key.index, named)).unwrap();
        Some(key)
    }
}

pub struct Service {
    // The version of the product. Some histograms may be limited to
    // specific versions of the product.
    version: Version,

    // A key generator for registration of new histograms. Uses atomic
    // to avoid the use of &mut.
    keys_single: indexing::KeyGenerator<Single>,
    keys_keyed: indexing::KeyGenerator<Map>,

    // Connection to the thread holding all the storage of this
    // instance of telemetry.
    sender: Sender<Op>,
}

pub struct Feature {
    // Are measurements active for this feature?
    is_active: Arc<Cell<bool>>,
    sender: Sender<Op>,
    telemetry: Arc<Service>,
}

//
// Low-level, untyped, implementation of histogram storage.
//
trait RawStorage: Send {
    fn store(&mut self, value: u32);
    fn serialize(&self, &SerializationFormat) -> Json;
}
trait RawStorageMap: Send {
    fn store(&mut self, key: String, value: u32);
    fn serialize(&self, format: &SerializationFormat) -> Json;
}

//
// Features shared by all histograms
//
struct BackEnd<K> {
    // A key used to map a histogram to its storage owned by telemetry,
    // or None if the histogram has been rejected by telemetry because
    // it has expired.
    key: Option<indexing::Key<K>>,

    // `true` unless the histogram has been deactivated by user request.
    // If `false`, no data will be recorded for this histogram.
    is_active: bool,

    sender: Sender<Op>,
    is_feature_active: Arc<Cell<bool>>,
}

impl<K> BackEnd<K> {
    fn new(feature: &Feature, key: Option<indexing::Key<K>>) -> BackEnd<K> {
        BackEnd {
            key: key,
            is_active: true,
            sender: feature.sender.clone(),
            is_feature_active: feature.is_active.clone(),
        }
    }

    fn get_key(&self) -> Option<&indexing::Key<K>>
    {
        if !self.is_active {
            return None;
        }
        if !self.is_feature_active.get() {
            return None;
        }
        match self.key {
            None => None,
            Some(ref k) => Some(k)
        }
    }
}

impl BackEnd<Single> {
    fn raw_record(&self, k: &indexing::Key<Single>, value: u32) {
        self.sender.send(Op::RecordSingle(k.index, value)).unwrap();
    }
}

impl<T> BackEnd<Keyed<T>> {
    fn raw_record(&self, k: &indexing::Key<Keyed<T>>, key: String, value: u32) {
        self.sender.send(Op::RecordKeyed(k.index, key, value)).unwrap();
    }
}

type NamedStorageSingle = NamedStorage<RawStorage>;
type NamedStorageMap = NamedStorage<RawStorageMap>;

// Operations used to communicate with the TelemetryTask.
enum Op {
    RegisterSingle(usize, NamedStorageSingle),
    RegisterKeyed(usize, NamedStorageMap),
    RecordSingle(usize, u32),
    RecordKeyed(usize, String, u32),
    Serialize(SerializationFormat, Sender<(Json, Json)>),
}


struct TelemetryTask {
    single: VecMap<NamedStorage<RawStorage>>,
    keyed: VecMap<NamedStorage<RawStorageMap>>,
    receiver: Receiver<Op>,
}

impl TelemetryTask {
    fn new(receiver: Receiver<Op>) -> TelemetryTask {
        TelemetryTask {
            single: VecMap::new(),
            keyed: VecMap::new(),
            receiver: receiver
        }
    }

    fn run(&mut self) {
        for msg in &self.receiver {
            match msg {
                Op::RegisterSingle(index, storage) => {
                    self.single.insert(index, storage);
                }
                Op::RegisterKeyed(index, storage) => {
                    self.keyed.insert(index, storage);
                }
                Op::RecordSingle(index, value) => {
                    let ref mut storage = self.single.get_mut(&index).unwrap();
                    storage.contents.store(value);
                }
                Op::RecordKeyed(index, key, value) => {
                    let ref mut storage = self.keyed.get_mut(&index).unwrap();
                    storage.contents.store(key, value);
                }
                Op::Serialize(format, sender) => {
                    let mut single_object = BTreeMap::new();
                    for ref histogram in self.single.values() {
                        single_object.insert(histogram.name.clone(), histogram.contents.serialize(&format));
                    }

                    let mut keyed_object = BTreeMap::new();
                    for ref histogram in self.keyed.values() {
                        keyed_object.insert(histogram.name.clone(), histogram.contents.serialize(&format));
                    }

                    sender.send((Json::Object(single_object), Json::Object(keyed_object))).unwrap();
                }
            }
        }
    }
}

//////////////////////////////////// Tests

#[cfg(test)]
mod tests {
    extern crate rustc_serialize;
    use self::rustc_serialize::json::Json;

    use std::sync::Arc;
    use std::sync::mpsc::{channel, Sender};
    use std::collections::BTreeMap;

    use super::*;


    #[test]
    fn create_flags() {
        let telemetry = Arc::new(Service::new([0, 0, 0, 0]));
        let feature = Feature::new(&telemetry);
        let flag_single = SingleFlag::new(&feature, Metadata { key: "Test linear single".to_string(), expires: None});
        let flag_map = KeyedFlag::new(&feature, Metadata { key: "Test flag map".to_string(), expires: None});

        flag_single.record(());
        flag_map.record("key".to_string(), ());

        feature.is_active.set(true);
        flag_single.record(());
        flag_map.record("key".to_string(), ());
    }

    #[test]
    fn create_linears() {
        let telemetry = Arc::new(Service::new([0, 0, 0, 0]));
        let feature = Feature::new(&telemetry);
        let linear_single =
            SingleLinear::new(&feature,
                              Metadata {
                                  key: "Test linear single".to_string(),
                                  expires: None
                              }, 0, 100, 10);
        let linear_map =
            KeyedLinear::new(&feature,
                              Metadata {
                                  key: "Test linear map".to_string(),
                                  expires: None
                              }, 0, 100, 10);

        linear_single.record(0);
        linear_map.record("key".to_string(), 0);

        feature.is_active.set(true);
        linear_single.record(0);
        linear_map.record("key".to_string(), 0);
    }

    #[test]
    fn test_serialize_simple() {
        let telemetry = Arc::new(Service::new([0, 0, 0, 0]));
        let feature = Feature::new(&telemetry);

        feature.is_active.set(true);

        // A single flag that will remain untouched.
        let flag_single_1_name = "Test linear single 1".to_string();
        let flag_single_1 = SingleFlag::new(&feature, Metadata { key: flag_single_1_name.clone(), expires: None});

        // A single flag that will be recorded once.
        let flag_single_2_name = "Test linear single 2".to_string();
        let flag_single_2 = SingleFlag::new(&feature, Metadata { key: flag_single_2_name.clone(), expires: None});
        flag_single_2.record(());

        // A map flag.
        let flag_map_name = "Test flag map".to_string();
        let flag_map = KeyedFlag::new(&feature, Metadata { key: flag_map_name.clone(), expires: None});
        let key1 = "key 1".to_string();
        let key2 = "key 2".to_string();
        flag_map.record(key1.clone(), ());
        flag_map.record(key2.clone(), ());

        // Serialize and check the results.
        let (sender, receiver) = channel();
        telemetry.serialize(SerializationFormat::Simple, sender);
        let (single, keyed) = receiver.recv().unwrap();

        // Compare the single stuff.
        let mut all_flag_single = BTreeMap::new();
        all_flag_single.insert(flag_single_1_name.clone(), Json::Boolean(false));
        all_flag_single.insert(flag_single_2_name.clone(), Json::Boolean(true));
        assert_eq!(single, Json::Object(all_flag_single));

        // Compare the map stuff.
        let mut all_flag_map = BTreeMap::new();
        all_flag_map.insert(flag_map_name.clone(),
                            Json::Array(vec![
                                Json::String(key2.clone()),
                                Json::String(key1.clone())
                                    ]));

        assert_eq!(keyed, Json::Object(all_flag_map));
    }
}
