extern crate rustc_serialize;
use self::rustc_serialize::json::Json;

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
type Version = [u32;4];

//
// Metadata on a histogram.
//
struct Metadata {
    // A key used to identify the histogram. Must be unique to the instance
    // of `telemetry`.
    key: String,

    // Optionally, a version of the product at which this histogram expires.
    expires: Option<Version>,

    // A human-redable description of the histogram. Do not forget units
    // or explanations of enumeration labels.
    description: String,
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
    fn record(&self, telemetry: &mut Telemetry, value: T) {
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
    fn record_cb<F>(&self, telemetry: &mut Telemetry, _: F) where F: FnOnce() -> Option<T>;
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
}

impl Histogram<()> for Flag {
    fn record_cb<F>(&self, telemetry: &mut Telemetry, cb: F) where F: FnOnce() -> Option<()>  {
        if !self.shared.should_record(telemetry) {
            return;
        }
        match cb() {
            None => {}
            Some(()) => telemetry.raw_record(&self.shared, 0)
        }
    }
}

impl Flag {
    fn new(telemetry: &mut Telemetry, meta: Metadata) -> Flag {
        let storage = Box::new(FlagStorage::new());
        let key = telemetry.register_storage(meta, storage).unwrap(); // FIXME: This will self-destruct if we have expired. We probably just want to neutralize the histogram.
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
    fn record_cb<F>(&self, telemetry: &mut Telemetry, cb: F) where F: FnOnce() -> Option<T>  {
        if !self.shared.should_record(telemetry) {
            return;
        }
        match cb() {
            None => {}
            Some(v) => telemetry.raw_record(&self.shared, self.get_bucket(v))
        }
    }

}

impl<T> Linear<T> where T: Flatten<T> {
    fn new(telemetry: &mut Telemetry, meta: Metadata, min: u32, max: u32, buckets: usize) -> Linear<T> {
        let storage = Box::new(LinearStorage::new(buckets));
        let key = telemetry.register_storage(meta, storage).unwrap();  // FIXME: This will self-destruct if we have expired. We probably just want to neutralize the histogram.
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

struct Telemetry {
    // The name of the product.
    product: String,

    // The version of the product. Some histograms may be limited to
    // specific versions of the product.
    version: Version,

    // Has telemetry been activated? `false` by default.
    is_active: bool,

    // The storage for all histograms.
    stores: Vec<Box<RawStorage>>,
}

struct StorageSettings; // FIXME: Define
struct ServerSettings; // FIXME: Define

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
    pub fn new(product: String,
               version: Version,
               _: Option<StorageSettings>,
               _: Option<ServerSettings>) -> Telemetry {
        Telemetry {
            product: product,
            version: version,
            stores: Vec::new(),
            is_active: false,
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
    // 
    //
    fn raw_record(&mut self, histogram: &Shared, value: u32) {
        assert!(histogram.key.index != 0);
        assert!(histogram.key.telemetry == self);
        let ref mut storage = self.stores[histogram.key.index];
        storage.store(value);
    }
}


//
// Low-level, untyped, implementation of histogram storage.
//
trait RawStorage {
    fn store(&mut self, value: u32);
    fn serialize(&self) -> Json;
}

//
// Features shared by all histograms
//
struct Shared {
    // A key used to map a histogram to its storage owned by telemetry.
    key: StorageKey,

    // `true` unless the histogram has been deactivated by user request.
    // If `false`, no data will be recorded for this histogram.
    is_active: bool,

    // `false` unless the histogram is designed to expire at some
    // version of the product and the current version is more recent.
    is_expired: bool
}

impl Shared {
    fn new(key: StorageKey) -> Shared {
        Shared {
            key: key,
            is_active: true,
            is_expired: false
        }
    }

    fn should_record(&self, telemetry: &Telemetry) -> bool {
        if !self.is_active {
            return false;
        }
        if self.is_expired {
            return false;
        }
        if !telemetry.is_active {
            return false;
        }
        return true;
    }
}
