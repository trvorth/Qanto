use qanto::qanto_serde::*;
use std::collections::HashMap;

#[test]
fn test_primitive_serialization() {
    let value = 42u32;
    let bytes = to_bytes(&value).unwrap();
    let deserialized: u32 = from_bytes(&bytes).unwrap();
    assert_eq!(value, deserialized);
}

#[test]
fn test_string_serialization() {
    let value = "Hello, Qanto!".to_string();
    let bytes = to_bytes(&value).unwrap();
    let deserialized: String = from_bytes(&bytes).unwrap();
    assert_eq!(value, deserialized);
}

#[test]
fn test_vec_serialization() {
    let value = vec![1u32, 2, 3, 4, 5];
    let bytes = to_bytes(&value).unwrap();
    let deserialized: Vec<u32> = from_bytes(&bytes).unwrap();
    assert_eq!(value, deserialized);
}

#[test]
fn test_option_serialization() {
    let some_value = Some(42u32);
    let none_value: Option<u32> = None;

    let some_bytes = to_bytes(&some_value).unwrap();
    let none_bytes = to_bytes(&none_value).unwrap();

    let some_deserialized: Option<u32> = from_bytes(&some_bytes).unwrap();
    let none_deserialized: Option<u32> = from_bytes(&none_bytes).unwrap();

    assert_eq!(some_value, some_deserialized);
    assert_eq!(none_value, none_deserialized);
}

#[test]
fn test_hashmap_serialization() {
    let mut map = HashMap::new();
    map.insert("key1".to_string(), 42u32);
    map.insert("key2".to_string(), 84u32);

    let bytes = to_bytes(&map).unwrap();
    let deserialized: HashMap<String, u32> = from_bytes(&bytes).unwrap();

    assert_eq!(map, deserialized);
}

#[test]
fn test_tuple_serialization() {
    let value = ("hello".to_string(), 42u32);
    let bytes = to_bytes(&value).unwrap();
    let deserialized: (String, u32) = from_bytes(&bytes).unwrap();
    assert_eq!(value, deserialized);
}

#[test]
fn test_result_serialization() {
    let ok_value: Result<u32, String> = Ok(42);
    let err_value: Result<u32, String> = Err("error".to_string());

    let ok_bytes = to_bytes(&ok_value).unwrap();
    let err_bytes = to_bytes(&err_value).unwrap();

    let ok_deserialized: Result<u32, String> = from_bytes(&ok_bytes).unwrap();
    let err_deserialized: Result<u32, String> = from_bytes(&err_bytes).unwrap();

    assert_eq!(ok_value, ok_deserialized);
    assert_eq!(err_value, err_deserialized);
}

#[test]
fn test_variable_length_encoding() {
    // Test small values (< 253)
    let small_value = 100usize;
    let mut serializer = BinarySerializer::new();
    serializer.write_length(small_value).unwrap();
    let bytes = serializer.finish();

    let mut deserializer = BinaryDeserializer::new(&bytes).unwrap();
    let deserialized = deserializer.read_length().unwrap();
    assert_eq!(small_value, deserialized);

    // Test medium values (253-65535)
    let medium_value = 1000usize;
    let mut serializer = BinarySerializer::new();
    serializer.write_length(medium_value).unwrap();
    let bytes = serializer.finish();

    let mut deserializer = BinaryDeserializer::new(&bytes).unwrap();
    let deserialized = deserializer.read_length().unwrap();
    assert_eq!(medium_value, deserialized);

    // Test large values (65536+)
    let large_value = 100000usize;
    let mut serializer = BinarySerializer::new();
    serializer.write_length(large_value).unwrap();
    let bytes = serializer.finish();

    let mut deserializer = BinaryDeserializer::new(&bytes).unwrap();
    let deserialized = deserializer.read_length().unwrap();
    assert_eq!(large_value, deserialized);
}
