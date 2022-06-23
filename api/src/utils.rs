use std::time::SystemTime;

#[must_use]
pub fn timestamp() -> String {
    humantime_serde::re::humantime::format_rfc3339(SystemTime::now()).to_string()
}
