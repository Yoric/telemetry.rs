use std::marker::PhantomData;
use std::sync::atomic::{AtomicUsize, Ordering};


// Witness type, used to specify that the data is specific to a single histogram.
pub struct Single;

// Witness type, used to specify that the data is specific to a map histogram.
pub struct Map;

// Witness type, used to specify that the data is specific to a map
// histogram with keys of a specific type `T`.
pub struct Keyed<T> {
    pub witness: PhantomData<T>
}

pub struct Key<T> {
    pub witness: PhantomData<T>,
    pub index: usize,
}
pub struct KeyGenerator<T> {
    counter: AtomicUsize,
    witness: PhantomData<T>,
}
impl<T> KeyGenerator<T> {
    pub fn new() -> KeyGenerator<T> {
        KeyGenerator {
            counter: AtomicUsize::new(0),
            witness: PhantomData,
        }
    }
}
impl KeyGenerator<Single> {
    pub fn next(&self) -> Key<Single> {
        Key {
            index: self.counter.fetch_add(1, Ordering::Relaxed),
            witness: PhantomData
        }
    }
}
impl KeyGenerator<Map> {
    pub fn next<T>(&self) -> Key<Keyed<T>> {
        Key {
            index: self.counter.fetch_add(1, Ordering::Relaxed),
            witness: PhantomData
        }
    }
}


