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
#[should_panic]
fn create_linears_bad_1() {
    let telemetry = Arc::new(Service::new());
    let feature = Feature::new(&telemetry);
    let _ : single::Linear<u32> =
        single::Linear::new(&feature,
                            "Test linear single".to_string(),
                            0, 100, 0); // Not enough histograms.

}

#[test]
#[should_panic]
fn create_linears_bad_2() {
    let telemetry = Arc::new(Service::new());
    let feature = Feature::new(&telemetry);
    let _ : single::Linear<u32> =
        single::Linear::new(&feature,
                            "Test linear single".to_string(),
                            0, 0, 1); // min >= max

}

#[test]
#[should_panic]
fn create_linears_bad_3() {
    let telemetry = Arc::new(Service::new());
    let feature = Feature::new(&telemetry);
    let _ : single::Linear<u32> =
        single::Linear::new(&feature,
                            "Test linear single".to_string(),
                            0, 10, 20); // Not enough histograms.

}

#[test]
fn test_serialize_simple() {
    let telemetry = Arc::new(Service::new());
    let feature = Feature::new(&telemetry);

    feature.set_active(true);

    ////////// Test flags

    // A single flag that will remain untouched.
    let flag_single_1_name = "Test flag single 1".to_string();
    let flag_single_1 = single::Flag::new(&feature, flag_single_1_name.clone());
    let _ = flag_single_1; // Silence an unused variable warning.

    // A single flag that will be recorded once.
    let flag_single_2_name = "Test flag single 2".to_string();
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
    telemetry.serialize(SerializationFormat::SimpleJson, sender.clone());
    let (single, keyed) = receiver.recv().unwrap();

    // Compare the single stuff.
    // We're making sure that only our histograms appear.
    let mut all_flag_single = BTreeMap::new();
    all_flag_single.insert(flag_single_1_name.clone(), Json::I64(0));
    all_flag_single.insert(flag_single_2_name.clone(), Json::I64(1));
    assert_eq!(single, Json::Object(all_flag_single));

    // Compare the map stuff.
    // We're making sure that only our histograms appear.
    let mut all_flag_map = BTreeMap::new();
    all_flag_map.insert(flag_map_name.clone(),
                        Json::Array(vec![
                            Json::String(key1.clone()),
                            Json::String(key2.clone())
                                ]));

    assert_eq!(keyed, Json::Object(all_flag_map));

    ////////// Test linears

    // Add a single linear histogram and fill it.
    let linear_single_1 = single::Linear::new(&feature, "Test linear single".to_string(), 0, 100, 10);
    linear_single_1.record(100);
    linear_single_1.record(99);
    linear_single_1.record(98);
    linear_single_1.record(25);

    // Add a keyed linear
    let linear_keyed_1 = keyed::KeyedLinear::new(&feature, "Test linear dynamic".to_string(), 0, 100, 10);
    linear_keyed_1.record("Key 1".to_string(), 120);
    linear_keyed_1.record("Key 1".to_string(), 98);
    linear_keyed_1.record("Key 2".to_string(), 35);
    linear_keyed_1.record("Key 2".to_string(), 55);

    // Compare stuff.
    telemetry.serialize(SerializationFormat::SimpleJson, sender.clone());
    let (single, keyed) = receiver.recv().unwrap();
    if let Json::Object(single_btree) = single {
        if let Some(&Json::Array(ref array)) = single_btree.get(&"Test linear single".to_string()) {
            let expect : Vec<Json> = vec![0, 0, 1, 0, 0, 0, 0, 0, 0, 3].iter().cloned().map(Json::I64).collect();
            assert_eq!(*array, expect);
        } else {
            panic!("No record for the histogram");
        }
    } else {
        panic!("Not a Json object");
    }

    if let Json::Object(keyed_btree) = keyed {
        if let Some(&Json::Object(ref hist_btree)) = keyed_btree.get(&"Test linear dynamic".to_string()) {
            assert_eq!(hist_btree.len(), 2);
            if let Some(&Json::Array(ref array)) = hist_btree.get(&"Key 1".to_string()) {
                let expect : Vec<Json> = vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 2].iter().cloned().map(Json::I64).collect();
                assert_eq!(*array, expect);
            } else {
                panic!("No key 1");
            }
            if let Some(&Json::Array(ref array)) = hist_btree.get(&"Key 2".to_string()) {
                let expect : Vec<Json> = vec![0, 0, 0, 1, 0, 1, 0, 0, 0, 0].iter().cloned().map(Json::I64).collect();
                assert_eq!(*array, expect);
            } else {
                panic!("No key 2");
            }
        } else {
            panic!("No record for the histogram");
        }
    } else {
        panic!("Not a Json object");
    }


    ////////// Test count
    let count_1 = single::Count::new(&feature, "Count 1".to_string());
    count_1.record(5);
    count_1.record(3);
    count_1.record(7);

    let keyed_count_1 = keyed::KeyedCount::new(&feature, "Keyed count 1".to_string());
    keyed_count_1.record("Key A".to_string(), 31);
    keyed_count_1.record("Key B".to_string(), 100);
    keyed_count_1.record("Key A".to_string(), 61);
    keyed_count_1.record("Key C".to_string(), 1);

    telemetry.serialize(SerializationFormat::SimpleJson, sender.clone());
    let (single, keyed) = receiver.recv().unwrap();
    if let Json::Object(single_btree) = single {
        if let Some(&Json::I64(ref num)) = single_btree.get(&"Count 1".to_string()) {
            assert_eq!(*num, 15);
        } else {
            panic!("No record for the histogram or not a num");
        }
    } else {
        panic!("Not a Json object");
    }

    if let Json::Object(keyed_btree) = keyed {
        if let Some(ref hist) = keyed_btree.get(&"Keyed count 1".to_string()) {
            let json = format!("{}", hist);
            assert_eq!(json, "{\"Key A\":92,\"Key B\":100,\"Key C\":1}");
        } else {
            panic!("No record for the histogram or not an object");
        }
    } else {
        panic!("Not a Json object");
    }
}

