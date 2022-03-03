use serde::{ser::SerializeMap, Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
pub struct Request<T> {
    data: T,
}

impl<T> Request<T> {
    pub fn data(&self) -> &T {
        &self.data
    }

    pub fn into_data(self) -> T {
        self.data
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Response<T> {
    data: T,
    #[serde(flatten)]
    status: ResponseStatus,
}

impl<T> Response<T> {
    #[inline]
    pub fn new(data: T, status: ResponseStatus) -> Self {
        Self { data, status }
    }
}

#[derive(Debug, Clone)]
pub enum ResponseStatus {
    Success(Option<Vec<String>>),
    Error(Option<Vec<String>>),
}

impl ResponseStatus {
    #[inline]
    pub fn success() -> Self {
        ResponseStatus::Success(None)
    }

    #[inline]
    pub fn success_with(message: Vec<String>) -> Self {
        ResponseStatus::Success(Some(message))
    }

    #[inline]
    pub fn error() -> Self {
        ResponseStatus::Error(None)
    }

    #[inline]
    pub fn error_with(error: Vec<String>) -> Self {
        ResponseStatus::Error(Some(error))
    }
}

impl Serialize for ResponseStatus {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        const EMPTY_SLICE: &[String] = &[];

        match self {
            ResponseStatus::Success(message) => {
                let mut map = serializer.serialize_map(Some(2))?;
                map.serialize_entry("success", &true)?;
                map.serialize_entry(
                    "message",
                    message
                        .as_ref()
                        .map(|x| x.as_slice())
                        .unwrap_or(EMPTY_SLICE),
                )?;
                map.end()
            }
            ResponseStatus::Error(message) => {
                let mut map = serializer.serialize_map(Some(2))?;
                map.serialize_entry("success", &false)?;
                map.serialize_entry(
                    "error",
                    message
                        .as_ref()
                        .map(|x| x.as_slice())
                        .unwrap_or(EMPTY_SLICE),
                )?;
                map.end()
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Empty {}

#[test]
fn test_response_status() {
    let mut status = ResponseStatus::Success(None);
    let mut serialized = serde_json::to_string(&status).unwrap();
    assert_eq!(serialized, r#"{"success":true,"message":[]}"#);

    status = ResponseStatus::Success(Some(vec!["test".to_string()]));
    serialized = serde_json::to_string(&status).unwrap();
    assert_eq!(serialized, r#"{"success":true,"message":["test"]}"#);

    status = ResponseStatus::Error(None);
    serialized = serde_json::to_string(&status).unwrap();
    assert_eq!(serialized, r#"{"success":false,"error":[]}"#);

    status = ResponseStatus::Error(Some(vec!["test".to_string()]));
    serialized = serde_json::to_string(&status).unwrap();
    assert_eq!(serialized, r#"{"success":false,"error":["test"]}"#);
}

#[test]
fn test_response() {
    use std::collections::BTreeMap;

    let status = Response::new(Empty {}, ResponseStatus::Success(None));
    assert_eq!(
        serde_json::to_string(&status).unwrap(),
        r#"{"data":{},"success":true,"message":[]}"#
    );

    let status = Response::new(
        // use btree map to ensure kv order
        BTreeMap::<&str, &str>::from_iter([
            ("session_id", "484adcb9-35b7-450f-b7a8-9984ab466b4d"),
            ("expire_at", "1626200000"),
        ]),
        ResponseStatus::Success(Some(vec!["Successfully created a new session.".to_owned()])),
    );
    assert_eq!(
        serde_json::to_string(&status).unwrap(),
        r#"{"data":{"expire_at":"1626200000","session_id":"484adcb9-35b7-450f-b7a8-9984ab466b4d"},"success":true,"message":["Successfully created a new session."]}"#
    );
}
