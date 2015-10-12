extern crate vec_map;
use self::vec_map::VecMap;

extern crate rustc_serialize;
use self::rustc_serialize::json::Json;

use std::sync::Arc;
use std::cell::Cell;
use std::collections::{BTreeMap, HashSet};
use std::sync::mpsc::{Receiver, Sender};

use indexing::{Key};
use misc::*;
use service::{PrivateAccess, Service};


//
// Low-level, untyped, implementation of histogram storage.
//
pub trait SingleRawStorage: Send {
    fn store(&mut self, value: u32);
    fn to_json(&self, &SerializationFormat) -> Json;
}
pub trait KeyedRawStorage: Send {
    fn store(&mut self, key: String, value: u32);
    fn to_json(&self, format: &SerializationFormat) -> Json;
}


// Operations used to communicate with the TelemetryTask.
pub enum Op {
    RegisterSingle(usize, NamedStorage<SingleRawStorage>),
    RegisterKeyed(usize, NamedStorage<KeyedRawStorage>),
    RecordSingle(usize, u32),
    RecordKeyed(usize, String, u32),
    Serialize(SerializationFormat, Sender<(Json, Json)>),
    Terminate
}


pub struct TelemetryTask {
    single: VecMap<NamedStorage<SingleRawStorage>>,
    keyed: VecMap<NamedStorage<KeyedRawStorage>>,
    receiver: Receiver<Op>,
    // The set of all keys, used for sanity checking only.
    keys: HashSet<String>,
}

impl TelemetryTask {
    pub fn new(receiver: Receiver<Op>) -> TelemetryTask {
        TelemetryTask {
            single: VecMap::new(),
            keyed: VecMap::new(),
            receiver: receiver,
            keys: HashSet::new(),
        }
    }

    pub fn run(&mut self) {
        for msg in &self.receiver {
            match msg {
                Op::RegisterSingle(index, storage) => {
                    assert!(self.keys.insert(storage.name.clone()));
                    self.single.insert(index, storage);
                }
                Op::RegisterKeyed(index, storage) => {
                    assert!(self.keys.insert(storage.name.clone()));
                    self.keyed.insert(index, storage);
                }
                Op::RecordSingle(index, value) => {
                    let ref mut storage = self.single.get_mut(&index).unwrap();
                    storage.contents.store(value);
                }
                Op::RecordKeyed(index, key, value) => {
                    let ref mut storage = self.keyed.get_mut(&index).unwrap();
                    storage.contents.store(key, value);
                }
                Op::Serialize(format, sender) => {
                    let mut single_object = BTreeMap::new();
                    for ref histogram in self.single.values() {
                        single_object.insert(histogram.name.clone(), histogram.contents.to_json(&format));
                    }

                    let mut keyed_object = BTreeMap::new();
                    for ref histogram in self.keyed.values() {
                        keyed_object.insert(histogram.name.clone(), histogram.contents.to_json(&format));
                    }

                    sender.send((Json::Object(single_object), Json::Object(keyed_object))).unwrap();
                },
                Op::Terminate => {
                    return;
                }
            }
        }
    }
}


//
// Features shared by all histograms
//
pub struct BackEnd<K> {
    key: Key<K>,
    is_active: Arc<Cell<bool>>,
    pub sender: Sender<Op>,
}

impl<K> BackEnd<K> {
    pub fn new(service: &Service, key: Key<K>) -> BackEnd<K> {
        BackEnd {
            key: key,
            is_active: PrivateAccess::get_is_active(service).clone(),
            sender: PrivateAccess::get_sender(service).clone(),
        }
    }

    pub fn get_key(&self) -> Option<&Key<K>> {
        if self.is_active.get() {
            Some(&self.key)
        } else {
            None
        }
    }
}
