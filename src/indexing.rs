use std::marker::PhantomData;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Witness type, used to specify that the data is specific to a plain histogram.
#[derive(Clone)]
pub struct Plain;

/// Witness type, used to specify that the data is specific to a map histogram.
#[derive(Clone)]
pub struct Map;

/// Witness type, used to specify that the data is specific to a map
/// histogram with keys of a specific type `T`.
pub struct Keyed<T> {
    pub witness: PhantomData<T>,
}
impl<T> Clone for Keyed<T> {
    fn clone(&self) -> Self {
        Keyed {
            witness: PhantomData,
        }
    }
}

/// A key used to communicate with the back-end for a given kind of histograms.
pub struct Key<T> {
    pub witness: PhantomData<T>,
    pub index: usize,
}
impl<T> Clone for Key<T> {
    fn clone(&self) -> Self {
        Key {
            witness: PhantomData,
            index: self.index,
        }
    }
}

/// A key generator. It produces consecutive numbers.
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
impl KeyGenerator<Plain> {
    pub fn next(&self) -> Key<Plain> {
        Key {
            index: self.counter.fetch_add(1, Ordering::Relaxed),
            witness: PhantomData,
        }
    }
}
impl KeyGenerator<Map> {
    pub fn next<T>(&self) -> Key<Keyed<T>> {
        Key {
            index: self.counter.fetch_add(1, Ordering::Relaxed),
            witness: PhantomData,
        }
    }
}
