extern crate rustc_serialize;
use self::rustc_serialize::json::Json;

use std::collections::HashMap;
use std::collections::hash_map::{Entry, VacantEntry, OccupiedEntry};
use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::marker::PhantomData;

//
// Telemetry is a mechanism used to capture metrics in an application,
// and either store the data locally or upload to a server for
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

    // A human-redable description of the histogram. Do not forget units
    // or explanations of enumeration labels.
    pub description: String,
}

trait Histogram<T> {
    //
    // Record a value in this histogram.
    //
    // The value is recorded only if all of the following conditions are met:
    // - `telemetry` is activated; and
    // - this histogram has not expired; and
    // - the histogram is active.
    //
    fn record(&mut self, telemetry: &mut Telemetry, value: T) {
        self.record_cb(telemetry, || Some(value))
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
    fn record_cb<F>(&mut self, telemetry: &mut Telemetry, _: F) where F: FnOnce() -> Option<T>;
}

pub trait Flatten<T> {
    fn as_u32(&self) -> u32;
}

impl Flatten<u32> for u32 {
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

struct FlagStorage {
    // `true` once we have called `record`, `false` until then.
    encountered: bool
}


impl FlagStorage {
    fn new() -> FlagStorage {
        FlagStorage {
            encountered: false
        }
    }
}

impl RawStorage for FlagStorage {
    fn store(&mut self, _: u32) {
        self.encountered = true;
    }
    fn serialize(&self) -> Json {
        unreachable!() // FIXME: Implement
    }
}

pub struct Flag {
    shared: Shared,
    // FIXME: We could cache the value to avoid sending messages across threads.
}

impl Histogram<()> for Flag {
    fn record_cb<F>(&mut self, telemetry: &mut Telemetry, cb: F) where F: FnOnce() -> Option<()>  {
        self.shared.with_key(telemetry, |k| {
            match cb() {
                None => {}
                Some(()) => telemetry.raw_record(&k, 0)
            }
        });
    }
}


impl Flag {
    pub fn new(telemetry: &mut Telemetry, meta: Metadata) -> Flag {
        let storage = Box::new(FlagStorage::new());
        let key = telemetry.register(meta, storage);
        Flag {
            shared: Shared::new(key),
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


struct LinearStorage {
    values: Vec<u32>// We cannot use an array here, as this would make the struct unsized
}

impl LinearStorage {
    fn new(buckets: usize) -> LinearStorage {
        LinearStorage {
            values: Vec::with_capacity(buckets)
        }
    }
}

impl RawStorage for LinearStorage {
    fn store(&mut self, index: u32) {
        self.values[index as usize] += 1;
    }
    fn serialize(&self) -> Json {
        unreachable!() // FIXME: Implement
    }
}

pub struct Linear<T> where T: Flatten<T> {
    witness: PhantomData<T>,
    shared: Shared,
    min: u32,
    max: u32, // Invariant: max > min
    buckets: u32 // FIXME: We assume that u32 <= usize. Assert?
}

impl<T> Histogram<T> for Linear<T> where T: Flatten<T> {
    fn record_cb<F>(&mut self, telemetry: &mut Telemetry, cb: F) where F: FnOnce() -> Option<T>  {
        self.shared.with_key(telemetry, |k| {
            match cb() {
                None => {}
                Some(v) => telemetry.raw_record(&k, self.get_bucket(v))
            }
        });
    }
}

impl<T> Linear<T> where T: Flatten<T> {
    fn new(telemetry: &mut Telemetry, meta: Metadata, min: u32, max: u32, buckets: usize) -> Linear<T> {
        let storage = Box::new(LinearStorage::new(buckets));
        let key = telemetry.register(meta, storage);
        Linear {
            witness: PhantomData,
            shared: Shared::new(key),
            min: min,
            max: max,
            buckets: buckets as u32
        }
    }

    fn get_bucket(&self, value: T) -> u32 {
        let value = value.as_u32();
        if value >= self.max {
            0
        } else if value <= self.min {
            self.buckets - 1 as u32
        } else {
            let num = value as f32 - self.min as f32;
            let den = self.max as f32 - self.min as f32;
            let res = (num / den) * self.buckets as f32;
            res as u32
        }
    }
}


impl Telemetry {
    fn new(product: String, version: Version) -> Telemetry {
        let (sender, receiver) = channel();
        thread::spawn(|| {
            let mut data = TelemetryTask::new();
            for msg in receiver {
                match msg {
                    Op::Register(index, storage) => {
                        data.data.insert(index, storage);
                    }
                    Op::Record(index, value) => {
                        let ref mut storage = data.data.get_mut(&index).unwrap();
                        storage.store(value);
                    }
                    Op::Serialize(chan) => {
                        unreachable!() // Implement
                    }
                }
            }
        });
        Telemetry {
            generator: KeyGenerator::new(),
            product: product,
            version: version,
            sender: sender,
            is_active: false,
        }
    }

    fn register(&self, meta: Metadata, storage: Box<RawStorage>) -> Option<Key<Telemetry>> {
        // Don't bother adding the histogram if it is expired.
        match meta.expires {
            Some(v) if v <= self.version => return None,
            _ => {}
        }

        let key = self.generator.next(self);
        self.sender.send(Op::Register(key.index, storage)).unwrap();
        Some(key)
    }

    fn raw_record(&self, key: &Key<Telemetry>, value: u32) {
        assert!(key.owner == self);
        self.sender.send(Op::Record(key.index, value)).unwrap();
    }
}

pub struct Telemetry {
    // The name of the product.
    product: String,

    // The version of the product. Some histograms may be limited to
    // specific versions of the product.
    version: Version,

    // Has telemetry been activated? `false` by default.
    is_active: bool,

    // A key generator for registration of new histograms. Uses atomic
    // to avoid the use of &mut.
    generator: KeyGenerator,

    // Connection to the thread holding all the storage of this
    // instance of telemetry.
    sender: Sender<Op>,
}

static tele : Telemetry = Telemetry::new("test", [0, 0, 0, 0]);
/*
pub struct StorageSettings; // FIXME: Define
pub struct ServerSettings; // FIXME: Define

struct StorageKey {
    index: usize,
    telemetry: *const Telemetry
}

impl Telemetry {
    //
    // Construct a new instance of telemetry.
    //
    // This instance is deactivated by default.
    //
    pub fn new(product: String, version: Version) -> Telemetry {
        Telemetry {
            product: product,
            version: version,
            stores: Vec::new(),
            is_active: false,
            keyed_stores: Vec::new(),
        }
    }

    //
    // Activate/deactivate Telemetry. If Telemetry is deactivated,
    // it will not record new data.
    //
    pub fn set_active(&mut self, value: bool) {
        self.is_active = value;
    }

    pub fn is_active(&self) -> bool {
        self.is_active
    }

    //
    // Register the `RawStorage` used by an histogram.
    //
    fn register_storage(&mut self, meta: Metadata, storage: Box<RawStorage>) -> Option<StorageKey> {
        {
            // Don't add the histogram if it is expired.
            match meta.expires {
                Some(v) if v <= self.version => return None,
                _ => {}
            }
        }
        self.stores.push(storage);
        Some(StorageKey {
            index: self.stores.len(), // Note: this won't be 0.
            telemetry: self
        })
    }

    //
    // Record a value in an histogram.
    //
    // This call causes a panic if `histogram` has been created for
    // another instance of telemetry or if the histogram has expired.
    //
    fn raw_record(&mut self, histogram: &Shared, value: u32) {
        assert!(histogram.key.index != 0);
        assert!(histogram.key.telemetry == self);
        // FIXME: Assert that this is a non-keyed histogram
        let ref mut storage = self.stores[histogram.key.index];
        storage.store(value);
    }

    fn raw_record_keyed<F>(&mut self, histogram: &Shared, key: String, value: u32, cb: &Box<F>) where F: Fn() -> Box<RawStorage> {
        assert!(histogram.key.index != 0);
        assert!(histogram.key.telemetry == self);
        // FIXME: Assert that this is a keyed histogram
        let ref mut storage = self.keyed_stores[histogram.key.index];
        match storage.entry(key) {
            Occupied(mut e) => {
                e.get_mut().store(value)
            }
            Vacant(mut e) => {
                let mut instance = cb();
                instance.store(value);
                e.insert(instance);
            }
        }
    }
}

*/
//
// Low-level, untyped, implementation of histogram storage.
//
trait RawStorage: Send {
    fn store(&mut self, value: u32);
    fn serialize(&self) -> Json;
}

//
// Features shared by all histograms
//
struct Shared {
    // A key used to map a histogram to its storage owned by telemetry,
    // or None if the histogram has been rejected by telemetry because
    // it has expired.
    key: Option<Key<Telemetry>>,

    // `true` unless the histogram has been deactivated by user request.
    // If `false`, no data will be recorded for this histogram.
    is_active: bool,
}

impl Shared {
    fn new(key: Option<Key<Telemetry>>) -> Shared {
        Shared {
            key: key,
            is_active: true,
        }
    }

    fn with_key<F>(&self, telemetry: &Telemetry, cb: F)
        where F: FnOnce(&Key<Telemetry>) -> ()
    {
        if !self.is_active {
            return;
        }
        if !telemetry.is_active {
            return;
        }
        match self.key {
            None => return,
            Some(ref k) => cb(k)
        }
    }
}


///////////////// Experiments

/*
Perhaps we should unpair the stuff that stores histograms from the
stuff that handles saving/upload.

Have a Collector that can request (and merge) data from all instances
of Telemetry.
*/


/*
A histogram is:
- a front-end, owned by the creator (implements Histogram<T>);
- a back-end, owned by the global telemetry (implements Storage2);
- the front-end contains an Arc<Sender<Key, u32>> used to send the data to the back-end through `telemetry`;
- keys are generated through an atomic KeyGenerator (avoiding the need to &mut)
*/

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::atomic::Ordering::Relaxed;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::rc::Rc;
use std::thread;

struct Key<T> {
    index: usize,
    owner: *const T,
}
struct KeyGenerator {
    counter: AtomicUsize,
}
impl KeyGenerator {
    fn new() -> KeyGenerator {
        KeyGenerator {
            counter: AtomicUsize::new(0),
        }
    }
    fn next<T>(&self, owner: &T) -> Key<T> {
        Key {
            index: self.counter.fetch_add(1, Ordering::Relaxed),
            owner: owner
        }
    }
}

enum Op {
    Register(usize, Box<RawStorage>),
    Record(usize, u32),
    Serialize(Sender<Json>),
}

struct TelemetryTask {
    // Note: if we decide to change this to a Vec, don't forget that
    // we have no guarantee that items will be added in the order they
    // were created, so don't forget to make it a Vec<Option<>> and to
    // resize as needed.
    data: HashMap<usize, Box<RawStorage>>
}

impl TelemetryTask {
    fn new() -> TelemetryTask {
        TelemetryTask {
            data: HashMap::new()
        }
    }
}


struct FlatStorage {
    data: Vec<u32>
}
impl FlatStorage {
    fn new() -> FlatStorage {
        FlatStorage {
            data: Vec::new()
        }
    }
}

impl RawStorage for FlatStorage {
    fn store(&mut self, index: u32) {
        self.data[index as usize] += 1
    }
    fn serialize(&self) -> Json {
        unreachable!() // FIXME: Implement
    }
}

pub struct Flat {
    shared: Shared,
}

impl Flat {
    fn new(meta: Metadata, telemetry: &Telemetry) -> Flat {
        let storage = Box::new(FlatStorage::new());
        let key = telemetry.register(meta, storage);
        Flat {
            shared: Shared::new(key),
        }
    }
}

impl Flat {
    // FIXME: Deactivate if necessary.
    fn record_cb<F>(&mut self, telemetry: &Telemetry, cb: F) where F: FnOnce() -> Option<u32> {
        match self.shared.key {
            None => {}
            Some(ref k) => {
                match cb() {
                    None => {}
                    Some(v) => telemetry.raw_record(&k, v)
                }
            }
        }
    }
}

/*

pub struct Linear2 {
    updater: Sender<u32>,
}

struct Linear2Store {
    values: Vec<u32>
}


pub struct Flag2 {
    updater: Sender<(StorageKey, u32>>
    encountered: bool
}

struct Flag2Store {
    encountered: bool
}

pub struct Telemetry;
impl Telemetry {
    fn register<F>(&self, meta: Metadata, chan: Receiver<(StorageKey, u32)>, cb: F) -> StorageKey {
        where F: FnMut(u32) -> () {
        // 
        unreachable!()
    }
}

impl Flag2 {
    pub fn new(meta: Metadata, telemetry: &Telemetry) -> Flag2 {
        let (sender, receiver) = channel();

        // This store is updated whenever we receive a message on `receiver`.
        // How do I do that? With an index, as usual.
        let mut store = Flag2Store {
            encountered: false
        };
        let mut closure = |_: u32| store.encountered = true;
        telemetry.register(meta, receiver, closure);
        Flag2 {
            updater: sender,
            encountered: false,
        }
    }
}

impl Flag2 {
    fn record_cb<F>(&mut self, telemetry: &mut Telemetry, _: F) where F: FnOnce() -> Option<()>{
        if self.encountered {
            return;
        }
        // FIXME: use callback
        self.encountered = true;
        // FIXME: Send message
//        self.updater.send(0)
    }
}
*/
/*
struct TelemetryCollector;
impl TelemetryCollector {
    fn register_set(&self, _: &Telemetry) {
        unreachable!() // TODO
    }

    // Spawn a thread to asynchronously collect data from all
    // instances of `Telemetry` (how do we deal with dead tasks? sync
    // channels?), merge it (how?), then export it as Json.
    fn export<F>(&self, cb: F) where F: FnOnce(Json) {
        unreachable!()
    }
}
*/
/*
pub struct KeyedFlag {
    map: HashMap<String, Flag>,
    shared: Shared,
    make: Box<Fn() -> Box<Flag>>
}

impl Flag {
    pub fn new_keyed<K>(telemetry: &mut Telemetry, meta: Metadata) -> KeyedFlag {
        unreachable!()
    }
}

impl Histogram<(String, ())> for KeyedFlag {
    fn record_cb<F>(&self, telemetry: &mut Telemetry, cb: F) where F: FnOnce() -> Option<(String, ())> {
        if !self.shared.should_record(telemetry) {
            return;
        }
        match cb() {
            None => {}
            Some((k, ())) => {
                telemetry.raw_record_keyed(&self.shared, k, 0, &self.make)
            }
        }
    }
}
 

use std::cell::RefCell;

static mut tele : Option<Telemetry> = None;
*/
