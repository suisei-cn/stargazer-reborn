use mongodb::bson::doc;
use password_hash::{Encoding, PasswordHash, PasswordVerifier};
use serde::{Deserialize, Serialize};

use crate::Result;

#[must_use]
#[derive(Clone, Debug, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Permission {
    #[serde(rename = "ro")]
    ReadOnly,
    #[serde(rename = "rw")]
    ReadWrite,
}

#[must_use]
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

#[must_use]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionRecord {
    hash: String,
    pub username: String,
    pub permissions: PermissionSet,
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

    /// Note that this function parse hash with default encoding.
    /// This is fine as long as the hash is generated with [`Pbkdf2::hash_password`]
    /// since it's using [`Output::init_with`], which calls [`Encoding::default`].
    ///
    /// To use a different encoding, use [`decode_with`].
    ///
    /// [`Pbkdf2::hash_password`]: pbkdf2::Pbkdf2
    /// [`Output::init_with`]: password_hash::Output::init_with
    /// [`decode_with`]: Self::decode_with
    pub fn decode(&self) -> Result<PasswordHash> {
        self.decode_with(Encoding::default())
    }

    /// Decode hash with a custom encoding.
    ///
    /// This will normally not be needed since hash generated should all be using the default encoding, which is base64.
    pub fn decode_with(&self, encoding: Encoding) -> Result<PasswordHash> {
        PasswordHash::parse(&self.hash, encoding).map_err(Into::into)
    }

    /// Validate if a password is correct
    pub fn validate(&self, password: &[u8]) -> Result<()> {
        let hash = self.decode()?;

        pbkdf2::Pbkdf2
            .verify_password(password, &hash)
            .map_err(Into::into)
    }
}

impl PermissionSet {
    pub const EMPTY: Self = Self::empty();
    pub const FULL: Self = Self::full();

    pub(crate) const fn empty() -> Self {
        Self {
            api: None,
            method: None,
            coordinator: None,
        }
    }

    pub(crate) const fn full() -> Self {
        Self {
            api: Some(Permission::ReadWrite),
            method: Some(Permission::ReadWrite),
            coordinator: Some(Permission::ReadWrite),
        }
    }
}
