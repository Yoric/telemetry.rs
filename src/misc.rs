pub struct NamedStorage<T: ?Sized> {
    pub name: String,
    pub contents: Box<T>,
}


pub enum SerializationFormat {
    Simple,
}

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


pub trait Flatten {
    fn as_u32(&self) -> u32;
}

impl Flatten for u32 {
    fn as_u32(&self) -> u32 {
        *self
    }
}


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
