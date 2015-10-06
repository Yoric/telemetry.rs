use std::marker::PhantomData;
use std::collections::HashMap;
use std::rc::Rc;

extern crate rustc_serialize;
use rustc_serialize::json::Json;

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

#[allow(dead_code)]
#[allow(unused_variables)]
impl Telemetry {
    // Instantiate telemetry for a given product version.
    pub fn new(version: String, _: Option<StorageSettings>, _: Option<ServerSettings>) -> Telemetry {
        unreachable!() // TODO
    }

    //
    // If an histogram is active, record a new value to the histogram.
    // Otherwise, do nothing. For a histogram to be active, the
    // following conditions must be met:
    // - Telemetry must be active;
    // - the histogram must not have been deactivated with `set_active_histogram`
    // - the histogram must not have expired.
    //
    pub fn record<T, F>(&mut self, histogram: &mut Histogram<T>, closure: F) where F: FnOnce() -> Option<T>{
        if !self.is_active {
            return;
        }
        // FIXME: Handle other cases in which the histogram may be deactivated.
        match closure() {
            Some(x) => histogram.record(x),
            None => {}
        }
    }
    pub fn record_keyed<K, H, T>(&mut self, _: &KeyedHistogram<K, H, T>, _: &FnOnce() -> Option<(K, T)>) {
        unreachable!() // TODO
    }

    // Activate or deactivate Telemetry. If Telemetry is deactivated,
    // calling `record`/`record_keyed` will have no effect.
    pub fn set_active(&mut self, value: bool) {
        self.is_active = value
    }
    pub fn is_active(&self) -> bool {
        self.is_active
    }

    // Activate or dactivate individual histograms. Ignored if Telemetry
    // is deactivated or if the histogram has expired.
    pub fn set_histogram_active<T>(&mut self, _: Histogram<T>, _: bool) {
        unreachable!() // TODO
    }
    pub fn is_histogram_active<T>(&self, _: Histogram<T>) -> bool {
        unreachable!() // TODO
    }

    // Export the list of histograms to a json document that may be
    // uploaded to a Telemetry server. This list does not contain the
    // values, only the metadata needed by the server to make sense of
    // data that has been uploaded.
    pub fn export_histograms(&self) -> Json {
        unreachable!() // TODO
    }

    // Called automatically by histogram constructors.
    fn register_flat(&mut self, meta: Metadata, histogram: Rc<HistogramBase>) {
        let previous = self.flat_histograms.insert(meta.name, histogram);
        assert!(previous.is_none());
    }
    fn register_keyed(&mut self, meta: Metadata, histogram: Rc<KeyedHistogramBase>) {
        let previous = self.keyed_histograms.insert(meta.name, histogram);
        assert!(previous.is_none());
    }
}



// Metadata for an histogram.
#[allow(dead_code)]
struct Metadata {
    // The name of the histogram. Must be unique in the application.
    name: String,

    // A list of e-mails to alert of evolutions in the histogram. Alerts
    // are handled by the server component.
    alerts: Vec<String>,

    // A human-readable description for the histogram. Don't forget to
    // include labels and units, if your measurement has any.
    description: String,

    // If provided, stop recording after a given version of the
    // application.
    expires_in_version: Option<String>,
}

trait HistogramBase {
    // FIXME: In the future, this trait will know about
    // (de)serializing both the content of an histogram
    // and its metadata, as well as how to (de)activate
    // the histogram.
}
trait Histogram<T> : HistogramBase {
    fn record(&mut self, T);
}

trait KeyedHistogramBase {
    // FIXME: In the future, this trait will know about
    // (de)serializing both the content of an histogram
    // and its metadata.
}

struct KeyedHistogram<K, H, T> where H : Histogram<T> {
    placeholder: PhantomData<T>,
    family: HashMap<K, Rc<H>>
}

impl<K, H, T> KeyedHistogramBase for KeyedHistogram<K, H, T> where H: Histogram<T>{
}

// This histogram type allows you to record a single value. This type
// is useful if you need to track whether a feature was ever used
// during a session. You only need to add a single line of code which
// sets the flag when the feature is used because the histogram is
// initialized with a default value of false (flag not set).
pub struct Flag {
    encountered: bool
}

#[allow(dead_code)]
impl Flag {
    fn new(mut telemetry: Telemetry, meta: Metadata) -> Rc<Flag> {
        let histogram = Rc::new(Flag { encountered: false });
        telemetry.register_flat(meta, histogram.clone());
        histogram
    }

    fn new_keyed<K>(mut telemetry: Telemetry, meta: Metadata) -> KeyedHistogram<K, Flag, ()> {
        // FIXME: Construct data structure.
        // FIXME: Register.
        unreachable!()
    }
}

impl Histogram<()> for Flag {
    fn record(&mut self, _: ()) {
        self.encountered = true
    }
}

impl HistogramBase for Flag {
}

// These histograms only record boolean values. Multiple boolean
// entries can be recorded in the same histogram during a single
// browsing session, e.g. if a histogram is measuring user choices in
// a dialog box with options "Yes" or "No", a new boolean value is
// added every time the dialog is displayed.
pub struct Boolean {
    value: bool
}

#[allow(dead_code)]
impl Boolean {
    fn new(_: Metadata) -> Rc<Boolean> {
        // FIXME: Construct data structure.
        // FIXME: Register.
        unreachable!()
    }
}

impl Histogram<bool> for Boolean {
    fn record(&mut self, value: bool) {
        self.value = value
    }
}

impl HistogramBase for Boolean {
}

// This histogram type is used when you want to record a count of
// something. It only stores a single value and it can only be
// incremented by one with each add/accumulate call.
pub struct Count {
    value: u32
}

#[allow(dead_code)]
impl Count {
    fn new(_: Metadata) -> Rc<Count> {
        // FIXME: Construct data structure.
        // FIXME: Register.
        unreachable!()
    }
}

impl Histogram<()> for Count {
    fn record(&mut self, _: ()) {
        self.value += 1
    }
}

impl HistogramBase for Count {
}


// This histogram type is intended for storing "enum" values. An
// enumerated histogram consists of a fixed number of "buckets", each
// of which is associated with a consecutive integer value (the
// bucket's "label"). Each bucket corresponds to an enum value and
// counts the number of times its particular enum value was
// recorded. You might use this type of histogram if, for example, you
// wanted to track the relative popularity of SSL handshake
// types. Whenever the browser started an SSL handshake, it would
// record one of a limited number of enum values which uniquely
// identifies the handshake type.
pub struct Enumerated<T> where T: AsSize<T> { // TODO
    phantom: PhantomData<T>,
    values: Vec<u32>
}

#[allow(dead_code)]
impl<T> Enumerated<T> where T: AsSize<T> {
    fn new(_: Metadata, max_value: usize) -> Enumerated<T> {
        unreachable!() // TODO
    }
}

// A type (typically, an enumeration) that can be represented as a
// number.
pub trait AsSize<T> {
    fn as_usize(T) -> usize;
    fn from_usize(usize) -> Option<T>;
}

impl<T: AsSize<T>> Histogram<T> for Enumerated<T> {
    fn record(&mut self, value: T) {
        let index = T::as_usize(value);
        self.values[index] += 1
    }
}

impl<T> HistogramBase for Enumerated<T> {
}

#[allow(dead_code)]
impl AsSize<u32> {
    fn as_usize(value: u32) -> usize {
        value as usize
    }
    fn from_usize(value: usize) -> Option<u32> {
        Some(value as u32)
    }
}

// Linear histograms are similar to enumerated histograms, except each
// bucket is associated with a range of values instead of a single
// enum value. The range of values covered by each bucket increases
// linearly from the previous bucket, e.g. one bucket might count the
// number of occurrences of values between 0 to 9, the next bucket
// would cover values 10-19, the next 20-29, etc. This bucket type is
// useful if there aren't orders of magnitude differences between the
// minimum and maximum values stored in the histogram, e.g. if the
// values you are storing are percentages 0-100%.
//
// For the sake of type-safety (and in particular to help avoid errors
// with inconsistent units), by default, linear histograms do not
// accept raw integers, but rather `AsSize`.
pub struct Linear<T> where T: AsSize<T> { // TODO
    placeholder: PhantomData<T>
}

#[allow(unused_variables)]
impl<T> Linear<T> where T: AsSize<T> {
    fn new(mut telemetry: Telemetry, meta: Metadata, min: u32, high: u32, buckets: u16) -> Rc<Linear<T>> {
        unreachable!() // TODO
    }
}

impl<T> Histogram<u32> for Linear<T> where T:AsSize<T> {
    fn record(&mut self, _: u32) {
        unreachable!() // TODO
    }
}

impl<T> HistogramBase for Linear<T> {
}


// Exponential histograms are similar to linear histograms but the
// range of values covered by each bucket increases exponentially. As
// an example of its use, consider the timings of an I/O operation
// whose duration might normally fall in the range of 0ms-50ms but
// extreme cases might have durations in seconds or minutes. For such
// measurements, you would want finer-grained bucketing in the normal
// range but coarser-grained bucketing for the extremely large
// values. An exponential histogram fits this requirement since it has
// "narrow" buckets near the minimum value and significantly "wider"
// buckets near the maximum value.
//
// For the sake of type-safety (and in particular to help avoid errors
// with inconsistent units), by default, exponential histograms do not
// accept raw integers, but rather `AsSize`.
pub struct Exponential<T> where T: AsSize<T> { // TODO
    placeholder: PhantomData<T>
}

#[allow(unused_variables)]
impl<T> Exponential<T> where T: AsSize<T> {
    fn new(mut telemetry: Telemetry, meta: Metadata, min: u32, high: u32, buckets: u16) -> Rc<Exponential<T>> {
        unreachable!() // TODO
    }
}

impl<T> Histogram<u32> for Exponential<T> where T:AsSize<T> {
    fn record(&mut self, _: u32) {
        unreachable!() // TODO
    }
}

impl<T> HistogramBase for Exponential<T> {
}

#[allow(dead_code)]
struct Telemetry {
    flat_histograms: HashMap<String, Rc<HistogramBase>>,
    keyed_histograms: HashMap<String, Rc<KeyedHistogramBase>>,
    is_active: bool,
}

#[allow(dead_code)]
pub struct StorageSettings; // TODO
#[allow(dead_code)]
pub struct ServerSettings; // TODO
