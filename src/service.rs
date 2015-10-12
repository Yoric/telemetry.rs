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
/// A group of histograms observed by Telemetry.
///
/// Many applications need to activate/deactivate measurements
/// dynamically, based e.g. on user preferences, privacy settings,
/// startup/shutdown sequence, etc. All histograms are created as part
/// of a `Feature`, which can be used to turn on/off all the
/// histograms that it owns.
///
impl Feature {
    ///
    /// Create a new feature.
    ///
    /// New features are **deactivated** by default.
    ///
    pub fn new(service: &Arc<Service>) -> Feature {
        Feature {
            is_active: Arc::new(Cell::new(false)),
            sender: service.sender.clone(),
            service: service.clone(),
        }
    }

    pub fn set_active(&self, value: bool) {
        self.is_active.set(value);
    }

    pub fn is_active(&self) -> bool {
        self.is_active.get()
    }
}

///
/// The Telemetry service.
///
/// Generally, an application will have only a single instance of this
/// service but may have any number of instances of `Feature` which may
/// be activated and deactivated individually.
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
        }
    }

    ///
    /// Serialize all histograms as json, in a given format.
    ///
    /// Returns a pair with plain histograms/keyed histograms.
    pub fn to_json(&self, format: SerializationFormat, sender: Sender<(Json, Json)>) {
        self.sender.send(Op::Serialize(format, sender)).unwrap();
    }

    fn register_single(&self, name: String, storage: Box<SingleRawStorage>) -> Option<Key<Single>> {
        let key = self.keys_single.next();
        let named = NamedStorage { name: name, contents: storage };
        self.sender.send(Op::RegisterSingle(key.index, named)).unwrap();
        Some(key)
    }

    fn register_keyed<T>(&self, name: String, storage: Box<KeyedRawStorage>) -> Option<Key<Keyed<T>>> {
        let key = self.keys_keyed.next();
        let named = NamedStorage { name: name, contents: storage };
        self.sender.send(Op::RegisterKeyed(key.index, named)).unwrap();
        Some(key)
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

    // Connection to the thread holding all the storage of this
    // instance of telemetry.
    sender: Sender<Op>,
}

pub struct Feature {
    // Are measurements active for this feature?
    is_active: Arc<Cell<bool>>,
    sender: Sender<Op>,
    service: Arc<Service>,
}


// Backstage pass used inside the crate.
pub struct PrivateAccess;

impl PrivateAccess {
    pub fn register_single(feature: &Feature, name: String, storage: Box<SingleRawStorage>) -> Option<Key<Single>> {
        feature.service.register_single(name, storage)
    }

    pub fn register_keyed<T>(feature: &Feature, name: String, storage: Box<KeyedRawStorage>) -> Option<Key<Keyed<T>>> {
        feature.service.register_keyed(name, storage)
    }

    pub fn get_sender(feature: &Feature) -> &Sender<Op> {
        &feature.sender
    }

    pub fn get_is_active(feature: &Feature) -> &Arc<Cell<bool>> {
        &feature.is_active
    }
}
