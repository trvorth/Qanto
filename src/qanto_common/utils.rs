use bincode;

pub fn serialize_bytes<T: serde::Serialize>(value: &T) -> Vec<u8> {
    bincode::serialize(value).unwrap_or_default()
}

pub fn deserialize_bytes<T: for<'de> serde::Deserialize<'de>>(data: &[u8]) -> Option<T> {
    bincode::deserialize(data).ok()
}
