extern crate rustc_serialize;
use self::rustc_serialize::json::Json;

extern crate telemetry;

use std::sync::Arc;
use std::sync::mpsc::channel;
use std::collections::BTreeMap;

use telemetry::*;

#[test]
fn create_flags() {
    let telemetry = Arc::new(Service::new());
    let feature = Feature::new(&telemetry);
    let flag_single = single::Flag::new(&feature, "Test linear single".to_string());
    let flag_map = keyed::KeyedFlag::new(&feature, "Test flag map".to_string());

    flag_single.record(());
    flag_map.record("key".to_string(), ());

    feature.set_active(true);
    flag_single.record(());
    flag_map.record("key".to_string(), ());
}

#[test]
fn create_linears() {
    let telemetry = Arc::new(Service::new());
    let feature = Feature::new(&telemetry);
    let linear_single =
        single::Linear::new(&feature,
                            "Test linear single".to_string(),
                            0, 100, 10);
    let linear_map =
        keyed::KeyedLinear::new(&feature,
                                "Test linear map".to_string(),
                                0, 100, 10);

    linear_single.record(0);
    linear_map.record("key".to_string(), 0);

    feature.set_active(true);
    linear_single.record(0);
    linear_map.record("key".to_string(), 0);
}

#[test]
fn test_serialize_simple() {
    let telemetry = Arc::new(Service::new());
    let feature = Feature::new(&telemetry);

    feature.set_active(true);

    // A single flag that will remain untouched.
    let flag_single_1_name = "Test linear single 1".to_string();
    let flag_single_1 = single::Flag::new(&feature, flag_single_1_name.clone());
    let _ = flag_single_1; // Silence an unused variable warning.

    // A single flag that will be recorded once.
    let flag_single_2_name = "Test linear single 2".to_string();
    let flag_single_2 = single::Flag::new(&feature, flag_single_2_name.clone());
    flag_single_2.record(());

    // A map flag.
    let flag_map_name = "Test flag map".to_string();
    let flag_map = keyed::KeyedFlag::new(&feature, flag_map_name.clone());
    let key1 = "key 1".to_string();
    let key2 = "key 2".to_string();
    flag_map.record(key1.clone(), ());
    flag_map.record(key2.clone(), ());

    // Serialize and check the results.
    let (sender, receiver) = channel();
    telemetry.serialize(SerializationFormat::SimpleJson, sender);
    let (single, keyed) = receiver.recv().unwrap();

    // Compare the single stuff.
    let mut all_flag_single = BTreeMap::new();
    all_flag_single.insert(flag_single_1_name.clone(), Json::I64(0));
    all_flag_single.insert(flag_single_2_name.clone(), Json::I64(1));
    assert_eq!(single, Json::Object(all_flag_single));

    // Compare the map stuff.
    let mut all_flag_map = BTreeMap::new();
    all_flag_map.insert(flag_map_name.clone(),
                        Json::Array(vec![
                            Json::String(key1.clone()),
                            Json::String(key2.clone())
                                ]));

    assert_eq!(keyed, Json::Object(all_flag_map));
}

