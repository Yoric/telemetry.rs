///!
///! Misc stuff used throughout the crate.
///!

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
}

///
/// A subformat of Json to use for serialization.
///
pub enum SerializationFormat {
    ///
    /// Simple Json:
    /// - `Flag` are represented as a single boolean;
    /// - `KeyedFlag` are represented as an array;
    /// - `Linear` are represented as an array of numbers, one cell per bucket;
    /// - `KeyedLinear` are represented as an object, one field per histogram,
    ///    with name = key, value = array of numbers as for `Linear`;
    /// - ...
    ///
    SimpleJson,
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

//
// Representation of buckets shared by both plain and keyed linear histograms.
//
pub struct LinearBuckets {
    min: u32,
    max: u32, // Invariant: max > min
    pub buckets: usize,
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
}


pub fn vec_with_size<T>(size: usize, value: T) -> Vec<T> where T: Clone {
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
