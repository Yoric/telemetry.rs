extern crate rustc_serialize;
use self::rustc_serialize::json::Json;

extern crate telemetry;

use std::collections::BTreeMap;
use std::sync::mpsc::channel;
use std::sync::Arc;

use telemetry::*;

#[test]
fn create_flags() {
    let telemetry = Arc::new(Service::new(false));
    let flag_plain = plain::Flag::new(&telemetry, "Test linear plain".to_string());
    let flag_map = keyed::KeyedFlag::new(&telemetry, "Test flag map".to_string());

    flag_plain.record(());
    flag_map.record("key".to_string(), ());

    telemetry.set_active(true);
    flag_plain.record(());
    flag_map.record("key".to_string(), ());
}

#[test]
fn create_linears() {
    let telemetry = Arc::new(Service::new(false));
    let linear_plain = plain::Linear::new(&telemetry, "Test linear plain".to_string(), 0, 100, 10);
    let linear_map = keyed::KeyedLinear::new(&telemetry, "Test linear map".to_string(), 0, 100, 10);

    linear_plain.record(0);
    linear_map.record("key".to_string(), 0);

    telemetry.set_active(true);
    linear_plain.record(0);
    linear_map.record("key".to_string(), 0);
}

#[test]
#[should_panic]
fn create_linears_bad_1() {
    let telemetry = Arc::new(Service::new(false));
    let _: plain::Linear<u32> =
        plain::Linear::new(&telemetry, "Test linear plain".to_string(), 0, 100, 0);
    // Not enough histograms.
}

#[test]
#[should_panic]
fn create_linears_bad_2() {
    let telemetry = Arc::new(Service::new(false));
    let _: plain::Linear<u32> =
        plain::Linear::new(&telemetry, "Test linear plain".to_string(), 0, 0, 1);
    // min >= max
}

#[test]
#[should_panic]
fn create_linears_bad_3() {
    let telemetry = Arc::new(Service::new(false));
    let _: plain::Linear<u32> =
        plain::Linear::new(&telemetry, "Test linear plain".to_string(), 0, 10, 20);
    // Not enough histograms.
}

enum TestEnum {
    Case1,
    Case2,
    Case3(String),
}

impl Flatten for TestEnum {
    fn as_u32(&self) -> u32 {
        match self {
            &TestEnum::Case1 => 0,
            &TestEnum::Case2 => 1,
            &TestEnum::Case3(_) => 2,
        }
    }
}

fn get_all_serialized(telemetry: &Service) -> (Json, Json) {
    let (sender, receiver) = channel();
    telemetry.to_json(
        Subset::AllPlain,
        SerializationFormat::SimpleJson,
        sender.clone(),
    );
    let plain = receiver.recv().unwrap();

    telemetry.to_json(
        Subset::AllKeyed,
        SerializationFormat::SimpleJson,
        sender.clone(),
    );
    let keyed = receiver.recv().unwrap();
    (plain, keyed)
}

#[test]
fn test_serialize_simple() {
    let telemetry = Service::new(false);

    telemetry.set_active(true);

    ////////// Test flags

    // A plain flag that will remain untouched.
    let flag_plain_1_name = "Test flag plain 1".to_string();
    let flag_plain_1 = plain::Flag::new(&telemetry, flag_plain_1_name.clone());
    let _ = flag_plain_1; // Silence an unused variable warning.

    // A plain flag that will be recorded once.
    let flag_plain_2_name = "Test flag plain 2".to_string();
    let flag_plain_2 = plain::Flag::new(&telemetry, flag_plain_2_name.clone());
    flag_plain_2.record(());

    // A map flag.
    let flag_map_name = "Test flag map".to_string();
    let flag_map = keyed::KeyedFlag::new(&telemetry, flag_map_name.clone());
    let key1 = "key 1".to_string();
    let key2 = "key 2".to_string();
    flag_map.record(key1.clone(), ());
    flag_map.record(key2.clone(), ());

    // Serialize and check the results.
    let (plain, keyed) = get_all_serialized(&telemetry);

    // Compare the plain stuff.
    // We're making sure that only our histograms appear.
    let mut all_flag_plain = BTreeMap::new();
    all_flag_plain.insert(flag_plain_1_name.clone(), Json::I64(0));
    all_flag_plain.insert(flag_plain_2_name.clone(), Json::I64(1));
    assert_eq!(plain, Json::Object(all_flag_plain));

    // Compare the map stuff.
    // We're making sure that only our histograms appear.
    let mut all_flag_map = BTreeMap::new();
    all_flag_map.insert(
        flag_map_name.clone(),
        Json::Array(vec![Json::String(key1.clone()), Json::String(key2.clone())]),
    );

    assert_eq!(keyed, Json::Object(all_flag_map));

    ////////// Test linears

    // Add a plain linear histogram and fill it.
    let linear_plain_1 =
        plain::Linear::new(&telemetry, "Test linear plain".to_string(), 0, 100, 10);
    linear_plain_1.record(100);
    linear_plain_1.record(99);
    linear_plain_1.record(98);
    linear_plain_1.record(25);

    // Add a keyed linear
    let linear_keyed_1 =
        keyed::KeyedLinear::new(&telemetry, "Test linear dynamic".to_string(), 0, 100, 10);
    linear_keyed_1.record("Key 1".to_string(), 120);
    linear_keyed_1.record("Key 1".to_string(), 98);
    linear_keyed_1.record("Key 2".to_string(), 35);
    linear_keyed_1.record("Key 2".to_string(), 55);

    // Compare stuff.

    let (plain, keyed) = get_all_serialized(&telemetry);
    if let Json::Object(plain_btree) = plain {
        if let Some(&Json::Array(ref array)) = plain_btree.get(&"Test linear plain".to_string()) {
            let expect: Vec<Json> = vec![0, 0, 1, 0, 0, 0, 0, 0, 0, 3]
                .iter()
                .cloned()
                .map(Json::I64)
                .collect();
            assert_eq!(*array, expect);
        } else {
            panic!("No record for the histogram");
        }
    } else {
        panic!("Not a Json object");
    }

    if let Json::Object(keyed_btree) = keyed {
        if let Some(&Json::Object(ref hist_btree)) =
            keyed_btree.get(&"Test linear dynamic".to_string())
        {
            assert_eq!(hist_btree.len(), 2);
            if let Some(&Json::Array(ref array)) = hist_btree.get(&"Key 1".to_string()) {
                let expect: Vec<Json> = vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 2]
                    .iter()
                    .cloned()
                    .map(Json::I64)
                    .collect();
                assert_eq!(*array, expect);
            } else {
                panic!("No key 1");
            }
            if let Some(&Json::Array(ref array)) = hist_btree.get(&"Key 2".to_string()) {
                let expect: Vec<Json> = vec![0, 0, 0, 1, 0, 1, 0, 0, 0, 0]
                    .iter()
                    .cloned()
                    .map(Json::I64)
                    .collect();
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
    let count_1 = plain::Count::new(&telemetry, "Count 1".to_string());
    count_1.record(5);
    count_1.record(3);
    count_1.record(7);

    let keyed_count_1 = keyed::KeyedCount::new(&telemetry, "Keyed count 1".to_string());
    keyed_count_1.record("Key A".to_string(), 31);
    keyed_count_1.record("Key B".to_string(), 100);
    keyed_count_1.record("Key A".to_string(), 61);
    keyed_count_1.record("Key C".to_string(), 1);

    let (plain, keyed) = get_all_serialized(&telemetry);
    if let Json::Object(plain_btree) = plain {
        if let Some(&Json::I64(ref num)) = plain_btree.get(&"Count 1".to_string()) {
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

    ////////// Test Enum
    let enum_1 = plain::Enum::new(&telemetry, "Enum 1".to_string());
    enum_1.record(TestEnum::Case2);
    enum_1.record(TestEnum::Case2);
    enum_1.record(TestEnum::Case3("foobar".to_string()));

    let keyed_enum_1 = keyed::KeyedEnum::new(&telemetry, "Keyed enum 1".to_string());
    keyed_enum_1.record("Key 2".to_string(), TestEnum::Case1);
    keyed_enum_1.record("Key 1".to_string(), TestEnum::Case1);
    keyed_enum_1.record("Key 1".to_string(), TestEnum::Case2);
    keyed_enum_1.record("Key 1".to_string(), TestEnum::Case2);

    let (plain, keyed) = get_all_serialized(&telemetry);
    if let Json::Object(plain_btree) = plain {
        if let Some(ref hist) = plain_btree.get(&"Enum 1".to_string()) {
            let json = format!("{}", hist);
            assert_eq!(json, "[0,2,1]");
        } else {
            panic!("No record for the histogram");
        }
    } else {
        panic!("Not a Json object");
    }

    if let Json::Object(keyed_btree) = keyed {
        if let Some(ref hist) = keyed_btree.get(&"Keyed enum 1".to_string()) {
            let json = format!("{}", hist);
            assert_eq!(json, "{\"Key 1\":[1,2],\"Key 2\":[1]}");
        } else {
            panic!("No record for the histogram");
        }
    } else {
        panic!("Not a Json object");
    }
}
