use argon2::password_hash::{Encoding, PasswordHash};
use serde::{Deserialize, Serialize};

use crate::Result;

/// Permission of either read-only and read-write
#[must_use]
#[derive(Clone, Debug, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Permission {
    #[serde(rename = "ro")]
    ReadOnly,
    #[serde(rename = "rw")]
    ReadWrite,
}

/// A partial map whose domain are central components and co-domain are read-only and read-write.
#[must_use]
#[non_exhaustive]
#[derive(Clone, Debug, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionSet {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api: Option<Permission>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mq: Option<Permission>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coordinator: Option<Permission>,
}

impl PermissionSet {
    /// Empty permission set, does not have access to any of the components.
    pub const EMPTY: Self = Self::empty();

    /// Full permission set, has access to all of the components.
    pub const FULL: Self = Self::full();

    pub(crate) const fn empty() -> Self {
        Self {
            api: None,
            mq: None,
            coordinator: None,
        }
    }

    pub(crate) const fn full() -> Self {
        Self {
            api: Some(Permission::ReadWrite),
            mq: Some(Permission::ReadWrite),
            coordinator: Some(Permission::ReadWrite),
        }
    }
}

impl Default for PermissionSet {
    fn default() -> Self {
        Self::EMPTY
    }
}

/// Record of user's permission set in the database.
#[must_use]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionRecord {
    hash: String,
    username: String,
    permissions: PermissionSet,
}

impl PermissionRecord {
    pub fn new(
        hash: PasswordHash,
        username: impl Into<String>,
        permissions: PermissionSet,
    ) -> Self {
        Self {
            hash: hash.serialize().as_str().into(),
            username: username.into(),
            permissions,
        }
    }

    /// Get hash string
    pub fn hash(&self) -> &str {
        &self.hash
    }

    /// Get the username
    pub fn username(&self) -> &str {
        &self.username
    }

    /// Get the permissions
    pub fn permissions(&self) -> PermissionSet {
        self.permissions
    }

    /// Decode hash with default [`Encoding`].
    /// To use a different encoding, see [`decode_with`].
    ///
    /// [`Output::init_with`]: argon2::password_hash::Output::init_with
    /// [`decode_with`]: Self::decode_with
    pub fn decode(&self) -> Result<PasswordHash> {
        self.decode_with(Encoding::default())
    }

    /// Decode hash with a custom encoding.
    ///
    /// This will normally not be needed since hash generated by [`AuthClient::new_record`]
    /// should all be using the default encoding, which is base64.
    ///
    /// [`AuthClient::new_record`]: crate::AuthClient::new_record
    pub fn decode_with(&self, encoding: Encoding) -> Result<PasswordHash> {
        PasswordHash::parse(&self.hash, encoding).map_err(Into::into)
    }
}
