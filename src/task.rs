//!
//! The dedicated telemetry thread and everything it owns.
//!
//! The thread is launched upon creation of `Service`, owned by it and
//! shutdown when the `Service` is dropped. This thread owns all the
//! storage for the histograms. Communication takes place through a
//! `channel`.

extern crate vec_map;
use self::vec_map::VecMap;

extern crate rustc_serialize;
use self::rustc_serialize::json::Json;

use std::collections::{BTreeMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;

use indexing::Key;
use misc::*;
use service::{PrivateAccess, Service};

///
/// Low-level, untyped, implementation of plain histogram storage.
///
pub trait PlainRawStorage: Send {
    fn store(&mut self, value: u32);
    fn to_json(&self, &SerializationFormat) -> Json;
}

///
/// Low-level, untyped, implementation of keyed histogram storage.
///
pub trait KeyedRawStorage: Send {
    fn store(&mut self, key: String, value: u32);
    fn to_json(&self, format: &SerializationFormat) -> Json;
}

/// Operations used to communicate with the TelemetryTask.
pub enum Op {
    /// `RegisterPlain(key, storage)` returns a plain histogram with
    /// key `key`. The key must be previously unused, otherwise panic.
    /// Unicity of the key is enforced through the use of a
    /// [KeyGenerator](../misc/struct.KeyGenerator.html).
    RegisterPlain(usize, NamedStorage<PlainRawStorage>),

    /// `RegisterPlain(key, storage)` returns a plain histogram with
    /// key `key`. The key must be previously unused, otherwise panic.
    /// Unicity of the key is enforced through the use of a
    /// [KeyGenerator](../misc/struct.KeyGenerator.html).
    RegisterKeyed(usize, NamedStorage<KeyedRawStorage>),

    /// `RecordPlain(key, value)` records value `value` in the plain
    /// histogram registered with key `key`.` The key must be
    /// registered to a plain histogram, otherwise panic.
    RecordPlain(usize, u32),

    /// `RecordKeyed(key, userkey, value)` records value `(userkey,
    /// value)` in the plain histogram registered with histogram key
    /// `key`.` The key must be registered to a plain histogram,
    /// otherwise panic.
    RecordKeyed(usize, String, u32),

    /// Proceed to serialization in a given format.
    Serialize(Subset, SerializationFormat, Sender<Json>),

    /// Terminate the thread immediately. Any further attempt to
    /// communicate with the tread will panic.
    Terminate,
}

///
/// The thread responsible for storing, bucketing and serializing data.
///
impl TelemetryTask {
    /// Create a new thread listening on a given channel.
    pub fn new(receiver: Receiver<Op>) -> TelemetryTask {
        TelemetryTask {
            plain: VecMap::new(),
            keyed: VecMap::new(),
            receiver: receiver,
            keys: HashSet::new(),
        }
    }

    /// Code executed by the thread.
    /// This thread runs until it receives message `Terminate`.
    pub fn run(&mut self) {
        for msg in &self.receiver {
            match msg {
                Op::RegisterPlain(index, storage) => {
                    assert!(self.keys.insert(storage.name.clone()));
                    self.plain.insert(index, storage);
                }
                Op::RegisterKeyed(index, storage) => {
                    assert!(self.keys.insert(storage.name.clone()));
                    self.keyed.insert(index, storage);
                }
                Op::RecordPlain(index, value) => {
                    let ref mut storage = self.plain.get_mut(&index).unwrap();
                    storage.contents.store(value);
                }
                Op::RecordKeyed(index, key, value) => {
                    let ref mut storage = self.keyed.get_mut(&index).unwrap();
                    storage.contents.store(key, value);
                }
                Op::Serialize(what, format, sender) => {
                    let mut object = BTreeMap::new();
                    match what {
                        Subset::AllPlain => {
                            for ref histogram in self.plain.values() {
                                object.insert(
                                    histogram.name.clone(),
                                    histogram.contents.to_json(&format),
                                );
                            }
                        }
                        Subset::AllKeyed => {
                            for ref histogram in self.keyed.values() {
                                object.insert(
                                    histogram.name.clone(),
                                    histogram.contents.to_json(&format),
                                );
                            }
                        }
                    }
                    sender.send(Json::Object(object)).unwrap();
                }
                Op::Terminate => {
                    return;
                }
            }
        }
    }
}

pub struct TelemetryTask {
    /// Plain histograms.
    plain: VecMap<NamedStorage<PlainRawStorage>>,

    /// Keyed histograms.
    keyed: VecMap<NamedStorage<KeyedRawStorage>>,

    /// The channel used by the task to receive data.
    receiver: Receiver<Op>,

    /// The set of all histogram names, used for sanity checking only.
    keys: HashSet<String>,
}

///
/// Features shared by all histograms
///
/// `K` is the kind of user keys, either `Plain` for a plain
/// histogram or `Keyed<T>` for a keyed histogram with user keys of
/// type `T`.
impl<K> BackEnd<K>
where
    K: Clone,
{
    /// Create a new back-end attached to a service and a key.
    pub fn new(service: &Service, key: Key<K>) -> BackEnd<K> {
        BackEnd {
            key: key,
            is_active: PrivateAccess::get_is_active(service).clone(),
            sender: PrivateAccess::get_sender(service).clone(),
        }
    }

    /// Get the key _if_ the service is currently active.
    pub fn get_key(&self) -> Option<&Key<K>> {
        if self.is_active.load(Ordering::Relaxed) {
            Some(&self.key)
        } else {
            None
        }
    }
}

#[derive(Clone)]
pub struct BackEnd<K>
where
    K: Clone,
{
    /// The key used to communicate with the `TelemetryTask`.
    key: Key<K>,

    /// The channel used to communicate with the `TelemetryTask`.
    pub sender: Sender<Op>,

    /// `true` if the Service is active, `false` otherwise.
    is_active: Arc<AtomicBool>,
}
