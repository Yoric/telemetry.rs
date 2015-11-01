///!
///! Misc stuff used throughout the crate.
///!

use rustc_serialize::json::Json;

use std::borrow::Cow;
use std::ptr;
use std::collections::BTreeMap;

///
/// A storage with a name attached.
///
/// Typically, `T` will be either a `PlainRawStorage` or a `KeyedRawStorage`.
///
pub struct NamedStorage<T: ?Sized> {
    /// The name of the storage. Also used as a key, must be unique.
    pub name: String,

    ///
    pub contents: Box<T>,
}

///
/// A subset of data to serialize.
///
pub enum Subset {
    /// Serialize all plain histograms.
    AllPlain,

    /// Serialize all keyed histograms.
    AllKeyed,

    /// Serialize everything.
    Everything,
}

///
/// A subformat of Json to use for serialization.
///
pub enum SerializationFormat {
    /// A simple and concise Json-based format, providing acceptable
    /// human-readability.
    ///
    /// - `Flag` are represented as a single boolean;
    /// - `KeyedFlag` are represented as an array;
    /// - `Linear` are represented as an array of numbers, one cell per bucket;
    /// - `KeyedLinear` are represented as an object, one field per histogram,
    ///    with name = key, value = array of numbers as for `Linear`;
    /// - ...
    ///
    SimpleJson,

    /// A somewhat verbose Json-based format compatible with the Mozilla Telemetry Server.
    ///
    /// {
    ///    range: [min, max],
    ///    bucket_count: <number of buckets>,
    ///    histogram_type: <histogram_type>,
    ///    sum: <sum>,
    ///    sum_squares_lo: <sum_squares_lo>,
    ///    sum_squares_hi: <sum_squares_hi>,
    ///    log_sum: <log_sum>,
    ///    log_sum_squares: <log_sum_squares>,
    ///    values: { bucket1: count1, bucket2: count2, ... }
    /// }
    Mozilla,
}

///
/// A value that can be represented as a u32.
///
pub trait Flatten {
    fn as_u32(&self) -> u32;
}

impl Flatten for u32 {
    fn as_u32(&self) -> u32 {
        *self
    }
}

impl Flatten for () {
    fn as_u32(&self) -> u32 {
        0
    }
}

impl Flatten for bool {
    fn as_u32(&self) -> u32 {
        if *self {
            1
        } else {
            0
        }
    }
}

//
// Representation of buckets shared by both plain and keyed linear histograms.
//
pub struct LinearBuckets {
    min: u32,
    max: u32, // Invariant: max > min
    buckets: usize,
}

impl LinearBuckets {
    pub fn new(min: u32, max: u32, buckets: usize) -> LinearBuckets {
        assert!(min < max);
        assert!(buckets > 0);
        assert!(buckets < (max - min) as usize);
        LinearBuckets {
            min: min,
            max: max,
            buckets: buckets
        }
    }

    pub fn get_bucket(&self, value: u32) -> usize {
        if value <= self.min {
            0
        } else if value >= self.max {
            self.buckets - 1 as usize
        } else {
            let num = value as f32 - self.min as f32;
            let den = self.max as f32 - self.min as f32;
            let res = (num / den) * self.buckets as f32;
            res as usize
        }
    }

    pub fn get_min(&self) -> u32 {
        self.min
    }

    pub fn get_max(&self) -> u32 {
        self.max
    }

    pub fn get_bucket_count(&self) -> usize {
        self.buckets
    }
}

pub struct LinearStats {
    sum: u64,
    sum_squares: u64,
}

impl LinearStats {
    pub fn new() -> Self {
        LinearStats {
            sum: 0,
            sum_squares: 0,
        }
    }

    pub fn record(&mut self, value: u32) {
        self.sum_squares += (value as u64) * (value as u64);
        self.sum += value as u64;
    }
}


/// Partial reimplementation of `Vec::resize`, until this method has
/// reached the stable version of Rust.
pub fn vec_resize<T>(vec: &mut Vec<T>, min_len: usize, value: T)
    where T: Clone
{
    let len = vec.len();
    if min_len <= len {
        return;
    }
    let delta = min_len - len;
    vec.reserve(delta);
    unsafe {
        let mut ptr = vec.as_mut_ptr().offset(len as isize);
        // Write all elements except the last one
        for i in 1..delta {
            ptr::write(ptr, value.clone());
            ptr = ptr.offset(1);
            // Increment the length in every step in case clone() panics
            vec.set_len(len + i);
        }

        // We can write the last element directly without cloning needlessly
        ptr::write(ptr, value);
        vec.set_len(len + delta);
    }
}

pub fn vec_with_size<T>(size: usize, value: T) -> Vec<T>
    where T: Clone
{
    let mut vec = Vec::with_capacity(size);
    unsafe {
        // Resize. In future versions of Rust, we should
        // be able to use `vec.resize`.
        vec.set_len(size);
        for i in 0 .. size {
            vec[i] = value.clone();
        }
    }
    vec
}


pub struct MozillaIntermediateFormat<'a> {
    pub min: i64,
    pub max: i64,
    pub bucket_count: i64,
    pub histogram_type: HistogramType,
    pub linear: Option<&'a LinearStats>,
    pub counts: Cow<'a, Vec<u32>>,
}

impl<'a> MozillaIntermediateFormat<'a> {
    /// Port of Mozilla's histogram packing algorithm, as seen
    /// here:
    /// https://dxr.mozilla.org/mozilla-central/rev/01e37977f8da2e1f8b9ce9b777e556ffb1437960/toolkit/components/telemetry/TelemetrySession.jsm#903
    pub fn to_json(&self) -> Json {
        let mut tree = BTreeMap::new();
        tree.insert("range".to_owned(),
                    Json::Array(vec![Json::I64(self.min),
                                     Json::I64(self.max)]));
        tree.insert("bucket_count".to_owned(), Json::I64(self.bucket_count));
        let histogram_type = match self.histogram_type {
            HistogramType::Linear => 1,
            HistogramType::Boolean => 2,
            HistogramType::Flag => 3,
            HistogramType::Count => 4,
            HistogramType::Custom => 5,
        };
        tree.insert("histogram_type".to_owned(), Json::I64(histogram_type));

        let mut values_tree = BTreeMap::new();

        // Index of the first non-0 value.
        let first = self.counts.iter().cloned().position(|x| x != 0);
        // Index of the last non-0 value.
        let last = self.counts.iter().cloned().rposition(|x| x != 0);
        match (first, last) {
            (None, None) => {} //Nothing to copy
            (Some(f), Some (l)) => {
                // Copy non-0 values, padding with 0 on both ends if this fits
                // within the bounds.
                let start = if f > 0 { f - 1 } else { f };
                let stop = if l < self.counts.len() - 1 { l + 1 } else { l };
                for i in start .. stop {
                    values_tree.insert(format!("{}", i), Json::I64(self.counts[i] as i64));
                }
            }
            _ => unreachable!()
        }

        tree.insert("values".to_owned(), Json::Object(values_tree));

        if let Some(ref unpacked) = self.linear {
            let sum = unpacked.sum;
            tree.insert("sum".to_owned(), Json::I64(sum as i64));

            let sum_squares = unpacked.sum_squares;
            // Emulate a u64 with two JS numbers.
            tree.insert("sum_squares_lo".to_owned(), Json::I64((sum_squares as u32) as i64));
            tree.insert("sum_squares_hi".to_owned(), Json::I64((sum_squares >> 32) as i64));
        }

        Json::Object(tree)
    }
}

pub enum HistogramType {
    Linear,
    Boolean,
    Flag,
    Count,
    Custom
}

