extern crate rustc_serialize;
use self::rustc_serialize::json::Json;

use std::sync::Arc;
use std::cell::Cell;
use std::thread;
use std::sync::mpsc::{channel, Sender};

use misc::*;
use task::{Op, SingleRawStorage, KeyedRawStorage, TelemetryTask};
use indexing::*;

///
/// The Telemetry service.
///
///
impl Service {
    pub fn new() -> Service {
        let (sender, receiver) = channel();
        thread::spawn(|| {
            let mut task = TelemetryTask::new(receiver);
            task.run()
        });
        Service {
            keys_single: KeyGenerator::new(),
            keys_keyed: KeyGenerator::new(),
            sender: sender,
            is_active: Arc::new(Cell::new(false)),
        }
    }

    ///
    /// Serialize all histograms as json, in a given format.
    ///
    /// Returns a pair with plain histograms/keyed histograms.
    pub fn to_json(&self, format: SerializationFormat, sender: Sender<(Json, Json)>) {
        self.sender.send(Op::Serialize(format, sender)).unwrap();
    }

    pub fn set_active(&self, value: bool) {
        self.is_active.set(value);
    }

    pub fn is_active(&self) -> bool {
        self.is_active.get()
    }

    fn register_single(&self, name: String, storage: Box<SingleRawStorage>) -> Key<Single> {
        let key = self.keys_single.next();
        let named = NamedStorage { name: name, contents: storage };
        self.sender.send(Op::RegisterSingle(key.index, named)).unwrap();
        key
    }

    fn register_keyed<T>(&self, name: String, storage: Box<KeyedRawStorage>) -> Key<Keyed<T>> {
        let key = self.keys_keyed.next();
        let named = NamedStorage { name: name, contents: storage };
        self.sender.send(Op::RegisterKeyed(key.index, named)).unwrap();
        key
    }
}

impl Drop for Service {
    /// Terminate the thread once the service is dead.
    fn drop(&mut self) {
        let _ = self.sender.send(Op::Terminate);
    }
}

pub struct Service {
    // A key generator for registration of new histograms. Uses atomic
    // to avoid the use of &mut.
    keys_single: KeyGenerator<Single>,
    keys_keyed: KeyGenerator<Map>,

    is_active: Arc<Cell<bool>>,

    // Connection to the thread holding all the storage of this
    // instance of telemetry.
    sender: Sender<Op>,
}


// Backstage pass used inside the crate.
pub struct PrivateAccess;

impl PrivateAccess {
    pub fn register_single(service: &Service, name: String, storage: Box<SingleRawStorage>) -> Key<Single> {
        service.register_single(name, storage)
    }

    pub fn register_keyed<T>(service: &Service, name: String, storage: Box<KeyedRawStorage>) -> Key<Keyed<T>> {
        service.register_keyed(name, storage)
    }

    pub fn get_sender(service: &Service) -> &Sender<Op> {
        &service.sender
    }

    pub fn get_is_active(service: &Service) -> &Arc<Cell<bool>> {
        &service.is_active
    }
}
