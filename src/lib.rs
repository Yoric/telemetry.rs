use std::marker::PhantomData;
use std::collections::HashMap;
use std::rc::Rc;

// Metadata for an histogram.
#[allow(dead_code)]
struct Metadata {

    // The name of the histogram. Must be unique in the application.
    name: String,

    // A list of e-mails to alert of evolutions in the histogram. Alerts
    // are handled by the server component.
    alerts: Vec<String>,

    // A human-readable description for the histogram.
    description: String,

    // If provided, stop recording after a given version of the
    // application.
    expires_in_version: Option<String>,
}

trait HistogramBase {
    // FIXME: In the future, this trait will know about
    // (de)serializing both the content of an histogram
    // and its metadata.
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
        self.encountered = true;
        // FIXME: Store data.
        unreachable!()
    }
}

impl HistogramBase for Flag {
}

// These histograms only record boolean values. Multiple boolean
// entries can be recorded in the same histogram during a single
// browsing session, e.g. if a histogram is measuring user choices in
// a dialog box with options "Yes" or "No", a new boolean value is
// added every time the dialog is displayed.
pub struct Boolean; // FIXME: Define.

#[allow(dead_code)]
impl Boolean {
    fn new(_: Metadata) -> Rc<Boolean> {
        // FIXME: Construct data structure.
        // FIXME: Register.
        unreachable!()
    }
}

impl Histogram<bool> for Boolean {
    fn record(&mut self, _: bool) {
        unreachable!() // TODO
    }
}

impl HistogramBase for Boolean {
}

// This histogram type is used when you want to record a count of
// something. It only stores a single value and it can only be
// incremented by one with each add/accumulate call.
pub struct Count; // TODO

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
        unreachable!() // TODO
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
pub struct Enumerated<T> where T: AsU32<T> { // TODO
    placeholder: PhantomData<T>
}

#[allow(dead_code)]
impl<T> Enumerated<T> where T: AsU32<T> {
    fn new(_: Metadata, max_value: u32) -> Enumerated<T> {
        unreachable!() // TODO
    }
}

// A type (typically, an enumeration) that can be represented as a
// number.
pub trait AsU32<T> {
    fn as_u32(T) -> u32 {
        unreachable!() // TODO
    }
    fn from_u32(u32) -> Option<T> {
        unreachable!() // TODO
    }
}

impl<T: AsU32<T>> Histogram<T> for Enumerated<T> {
    fn record(&mut self, _: T) {
        unreachable!() // TODO
    }
}

impl<T> HistogramBase for Enumerated<T> {
}

#[allow(dead_code)]
impl AsU32<u32> {
    fn as_u32(value: u32) -> u32 {
        value
    }
    fn from_u32(value: u32) -> Option<u32> {
        Some(value)
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
// accept raw integers, but rather `AsU32`.
pub struct Linear<T> where T: AsU32<T> { // TODO
    placeholder: PhantomData<T>
}

#[allow(unused_variables)]
impl<T> Linear<T> where T: AsU32<T> {
    fn new(mut telemetry: Telemetry, meta: Metadata, min: u32, high: u32, buckets: u16) -> Rc<Linear<T>> {
        unreachable!() // TODO
    }
}

impl<T> Histogram<u32> for Linear<T> where T:AsU32<T> {
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
// accept raw integers, but rather `AsU32`.
pub struct Exponential<T> where T: AsU32<T> { // TODO
    placeholder: PhantomData<T>
}

#[allow(unused_variables)]
impl<T> Exponential<T> where T: AsU32<T> {
    fn new(mut telemetry: Telemetry, meta: Metadata, min: u32, high: u32, buckets: u16) -> Rc<Exponential<T>> {
        unreachable!() // TODO
    }
}

impl<T> Histogram<u32> for Exponential<T> where T:AsU32<T> {
    fn record(&mut self, _: u32) {
        unreachable!() // TODO
    }
}

impl<T> HistogramBase for Exponential<T> {
}

#[allow(dead_code)]
struct Telemetry {
    flat_histograms: HashMap<String, Rc<HistogramBase>>,
    keyed_histograms: HashMap<String, Rc<KeyedHistogramBase>>
}

#[allow(dead_code)]
pub struct StorageSettings; // TODO
#[allow(dead_code)]
pub struct ServerSettings; // TODO

#[allow(dead_code)]
#[allow(unused_variables)]
impl Telemetry {
    pub fn new(version: String, _: Option<StorageSettings>, _: Option<ServerSettings>) -> Telemetry {
        unreachable!() // TODO
    }

    // If an histogram is active, record a new value to the histogram.
    // Otherwise, do nothing.
    pub fn record<T>(&mut self, _: &Histogram<T>, _: &FnOnce() -> Option<T>) {
        unreachable!() // TODO
    }
    pub fn record_keyed<K, H, T>(&mut self, _: &KeyedHistogram<K, H, T>, _: &FnOnce() -> Option<(K, T)>) {
        unreachable!() // TODO
    }

    // Activate or deactivate Telemetry.
    pub fn set_active(&mut self, _: bool) {
        unreachable!() // TODO
    }
    pub fn is_active(&self) -> bool {
        unreachable!() // TODO
    }

    // Activate or dactivate individual histograms. Ignored if Telemetry
    // is deactivated or if the histogram has expired.
    pub fn set_active_histogram<T>(&mut self, _: Histogram<T>, _: bool) {
        unreachable!() // TODO
    }
    pub fn is_active_histogram<T>(&self, _: Histogram<T>) -> bool {
        unreachable!() // TODO
    }

    // Called automatically by histogram constructors.
    fn register_flat(&mut self, meta: Metadata, histogram: Rc<HistogramBase>) {
        self.flat_histograms.insert(meta.name, histogram); // FIXME: Make sure that we register each name only once.
    }
    fn register_keyed(&mut self, meta: Metadata, histogram: Rc<KeyedHistogramBase>) {
        self.keyed_histograms.insert(meta.name, histogram); // FIXME: Make sure that we register each name only once.
    }

    // Export the list of histograms to a json document that may be
    // uploaded to a Telemetry server.
    pub fn export_histograms(&self) -> String {
        unreachable!() // TODO
    }
}


