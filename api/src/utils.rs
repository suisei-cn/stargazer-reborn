use isolanguage_1::LanguageCode;
use serde_json::Value;
use std::time::SystemTime;

use crate::{ApiError, ApiResult};

pub fn timestamp() -> String {
    humantime_serde::re::humantime::format_rfc3339(SystemTime::now()).to_string()
}

pub fn map(k: impl Into<String>, v: impl Into<String>) -> serde_json::Map<String, Value> {
    let mut map = serde_json::Map::new();
    map.insert(k.into(), Value::String(v.into()));
    map
}

/// Used as a temporary solution to
/// [this error that mongodb db 2.2.x cannot deserialize HashMap where key is an enum](https://github.com/suisei-cn/stargazer-reborn/issues/46)
pub fn validate_names<'a>(mut names: impl Iterator<Item = &'a str>) -> ApiResult<()> {
    if names.any(|k| <&str as TryInto<LanguageCode>>::try_into(k).is_err()) {
        Err(ApiError::bad_request(
            "Invalid language code. All key in `name` should comform to ISO 639-1",
        ))
    } else {
        Ok(())
    }
}
