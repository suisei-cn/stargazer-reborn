use mongodb::{bson::doc, Collection};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Permission {
    #[serde(rename = "ro")]
    ReadOnly,
    #[serde(rename = "rw")]
    ReadWrite,
}

#[non_exhaustive]
#[derive(Clone, Debug, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionSet {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api: Option<Permission>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<Permission>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coordinator: Option<Permission>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionRecord {
    pub username: String,
    pub hash: String,
    pub permissions: PermissionSet,
}

#[derive(Clone, Debug)]
pub struct AuthClient {
    pub(crate) col: Collection<PermissionRecord>,
}
