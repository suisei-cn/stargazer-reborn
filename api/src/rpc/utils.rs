use serde_json::Value;

pub fn map(k: impl Into<String>, v: impl Into<String>) -> serde_json::Map<String, Value> {
    let mut map = serde_json::Map::new();
    map.insert(k.into(), Value::String(v.into()));
    map
}
