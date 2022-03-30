use serde_json::Value;
use std::time::SystemTime;

pub fn timestamp() -> String {
    humantime_serde::re::humantime::format_rfc3339(SystemTime::now()).to_string()
}

pub fn map(k: impl Into<String>, v: impl Into<String>) -> serde_json::Map<String, Value> {
    let mut map = serde_json::Map::new();
    map.insert(k.into(), Value::String(v.into()));
    map
}
