extern crate vec_map;
use self::vec_map::VecMap;

extern crate rustc_serialize;
use self::rustc_serialize::json::Json;

use std::collections::{BTreeMap, HashSet};
use std::sync::mpsc::{Receiver, Sender};

use misc::*;



//
// Low-level, untyped, implementation of histogram storage.
//
pub trait RawStorage: Send {
    fn store(&mut self, value: u32);
    fn serialize(&self, &SerializationFormat) -> Json;
}
pub trait RawStorageMap: Send {
    fn store(&mut self, key: String, value: u32);
    fn serialize(&self, format: &SerializationFormat) -> Json;
}


// Operations used to communicate with the TelemetryTask.
pub enum Op {
    RegisterSingle(usize, NamedStorage<RawStorage>),
    RegisterKeyed(usize, NamedStorage<RawStorageMap>),
    RecordSingle(usize, u32),
    RecordKeyed(usize, String, u32),
    Serialize(SerializationFormat, Sender<(Json, Json)>),
    Terminate
}


pub struct TelemetryTask {
    single: VecMap<NamedStorage<RawStorage>>,
    keyed: VecMap<NamedStorage<RawStorageMap>>,
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
                        single_object.insert(histogram.name.clone(), histogram.contents.serialize(&format));
                    }

                    let mut keyed_object = BTreeMap::new();
                    for ref histogram in self.keyed.values() {
                        keyed_object.insert(histogram.name.clone(), histogram.contents.serialize(&format));
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

