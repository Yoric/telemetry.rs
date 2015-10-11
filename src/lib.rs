extern crate rustc_serialize;


mod misc;
pub use misc::{Flatten, Metadata, SerializationFormat, Version};

mod indexing;

mod task;

pub mod single;
pub mod keyed;
mod service;
pub use service::{Feature, Service};


// Telemetry is a mechanism used to capture metrics in an application,
// to later store the data locally or upload it to a server for
// statistical analysis.
//
// Examples of usage:
// - capturing the speed of an operation;
// - finding out if users are actually using a feature;
// - finding out how the duration of a session;
// - determine the operating system on which the application is executed;
// - determining the configuration of the application;
// - capturing the operations that slow down the application;
// - determining the amount of I/O performed by the application;
// - ...
//
// The abstraction used by this library is the Histogram. Each
// Histogram serves to capture a specific measurement, store it
// locally and/or upload it to the server. Several types of Histograms
// are provided, suited to distinct kinds of measures.
//
//
// Memory note: the memory used by a histogram is recollected only
// when its instance of `telemetry` is garbage-collected. In other words,
// if a histogram goes out of scope for some reason, its data remains
// in telemetry and will be stored and/or uploaded in accordance with the
// configuration of this telemetry instance.
//



//////////////////////////////////// Tests

#[cfg(test)]
mod tests {
    extern crate rustc_serialize;
    use self::rustc_serialize::json::Json;

    use std::sync::Arc;
    use std::sync::mpsc::channel;
    use std::collections::BTreeMap;

    use super::*;


    #[test]
    fn create_flags() {
        let telemetry = Arc::new(Service::new([0, 0, 0, 0]));
        let feature = Feature::new(&telemetry);
        let flag_single = SingleFlag::new(&feature, Metadata { key: "Test linear single".to_string(), expires: None});
        let flag_map = KeyedFlag::new(&feature, Metadata { key: "Test flag map".to_string(), expires: None});

        flag_single.record(());
        flag_map.record("key".to_string(), ());

        feature.is_active.set(true);
        flag_single.record(());
        flag_map.record("key".to_string(), ());
    }

    #[test]
    fn create_linears() {
        let telemetry = Arc::new(Service::new([0, 0, 0, 0]));
        let feature = Feature::new(&telemetry);
        let linear_single =
            SingleLinear::new(&feature,
                              Metadata {
                                  key: "Test linear single".to_string(),
                                  expires: None
                              }, 0, 100, 10);
        let linear_map =
            KeyedLinear::new(&feature,
                              Metadata {
                                  key: "Test linear map".to_string(),
                                  expires: None
                              }, 0, 100, 10);

        linear_single.record(0);
        linear_map.record("key".to_string(), 0);

        feature.is_active.set(true);
        linear_single.record(0);
        linear_map.record("key".to_string(), 0);
    }

    #[test]
    fn test_serialize_simple() {
        let telemetry = Arc::new(Service::new([0, 0, 0, 0]));
        let feature = Feature::new(&telemetry);

        feature.is_active.set(true);

        // A single flag that will remain untouched.
        let flag_single_1_name = "Test linear single 1".to_string();
        let flag_single_1 = SingleFlag::new(&feature, Metadata { key: flag_single_1_name.clone(), expires: None});

        // A single flag that will be recorded once.
        let flag_single_2_name = "Test linear single 2".to_string();
        let flag_single_2 = SingleFlag::new(&feature, Metadata { key: flag_single_2_name.clone(), expires: None});
        flag_single_2.record(());

        // A map flag.
        let flag_map_name = "Test flag map".to_string();
        let flag_map = KeyedFlag::new(&feature, Metadata { key: flag_map_name.clone(), expires: None});
        let key1 = "key 1".to_string();
        let key2 = "key 2".to_string();
        flag_map.record(key1.clone(), ());
        flag_map.record(key2.clone(), ());

        // Serialize and check the results.
        let (sender, receiver) = channel();
        telemetry.serialize(SerializationFormat::Simple, sender);
        let (single, keyed) = receiver.recv().unwrap();

        // Compare the single stuff.
        let mut all_flag_single = BTreeMap::new();
        all_flag_single.insert(flag_single_1_name.clone(), Json::Boolean(false));
        all_flag_single.insert(flag_single_2_name.clone(), Json::Boolean(true));
        assert_eq!(single, Json::Object(all_flag_single));

        // Compare the map stuff.
        let mut all_flag_map = BTreeMap::new();
        all_flag_map.insert(flag_map_name.clone(),
                            Json::Array(vec![
                                Json::String(key2.clone()),
                                Json::String(key1.clone())
                                    ]));

        assert_eq!(keyed, Json::Object(all_flag_map));
    }
}
