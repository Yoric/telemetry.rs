pub struct NamedStorage<T: ?Sized> {
    pub name: String,
    pub contents: Box<T>,
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
/// A software version, e.g. [2015, 10, 10, 0].
///
pub type Version = [u32;4];

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

//
// Representation of buckets shared by both single and keyed linear histograms.
//
pub struct LinearBuckets {
    pub min: u32,
    pub max: u32, // Invariant: max > min
    pub buckets: usize,
}

impl LinearBuckets {
    pub fn get_bucket(&self, value: u32) -> usize {
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


pub fn vec_with_size<T>(size: usize, value: T) -> Vec<T> where T: Clone {
    let mut vec = Vec::with_capacity(size);
    unsafe {
        // Resize. In future versions of Rust, we should
        // be able to use `vec.resize`.
        vec.set_len(size);
        for i in 0 .. size - 1 {
            vec[i] = value.clone();
        }
    }
    vec
}
