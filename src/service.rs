extern crate rustc_serialize;
use self::rustc_serialize::json::Json;

use std::sync::Arc;
use std::cell::Cell;
use std::thread;
use std::sync::mpsc::{channel, Sender};

use misc::*;
use task::{Op, PlainRawStorage, KeyedRawStorage, TelemetryTask};
use indexing::*;

///
/// The Telemetry service.
///
/// # Panics
///
/// The service will panic if an attempt is made to register two
/// histograms with the same key.
///
impl Service {
    pub fn new() -> Service {
        let (sender, receiver) = channel();
        thread::spawn(|| {
            let mut task = TelemetryTask::new(receiver);
            task.run()
        });
        Service {
            keys_plain: KeyGenerator::new(),
            keys_keyed: KeyGenerator::new(),
            sender: sender,
            is_active: Arc::new(Cell::new(false)),
        }
    }

    ///
    /// Serialize all histograms as json, in a given format.
    ///
    /// Returns a pair with plain histograms/keyed histograms.
    ///
    pub fn to_json(&self, format: SerializationFormat, sender: Sender<(Json, Json)>) {
        self.sender.send(Op::Serialize(format, sender)).unwrap();
    }

    ///
    /// Make the service (in)active.
    ///
    /// Any data recorded on a histogram while the service is inactive will be ignored.
    ///
    pub fn set_active(&self, value: bool) {
        self.is_active.set(value);
    }

    pub fn is_active(&self) -> bool {
        self.is_active.get()
    }

    ///
    /// Register a plain histogram, returning a fresh key.
    ///
    fn register_plain(&self, name: String, storage: Box<PlainRawStorage>) -> Key<Plain> {
        let key = self.keys_plain.next();
        let named = NamedStorage { name: name, contents: storage };
        self.sender.send(Op::RegisterPlain(key.index, named)).unwrap();
        key
    }

    ///
    /// Register a keyed histogram, returning a fresh key.
    ///
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
    /// A key generator for registration of new plain histograms. Uses
    /// atomic to avoid the use of &mut.
    keys_plain: KeyGenerator<Plain>,

    /// A key generator for registration of new keyed histograms. Uses
    /// atomic to avoid the use of &mut.
    keys_keyed: KeyGenerator<Map>,

    /// A shared cell that may be turned on/off to (de)activate
    /// Telemetry.
    is_active: Arc<Cell<bool>>,

    /// Connection to the thread holding all the storage of this
    /// instance of the service.
    sender: Sender<Op>,
}


// Backstage pass used inside the crate.
impl PrivateAccess {
    pub fn register_plain(service: &Service, name: String, storage: Box<PlainRawStorage>) -> Key<Plain> {
        service.register_plain(name, storage)
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

pub struct PrivateAccess;

