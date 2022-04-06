use argon2::{
    password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
    Argon2, PasswordHash, PasswordVerifier,
};
use mongodb::{
    bson::{doc, to_bson},
    options::UpdateOptions,
    Collection, Cursor,
};

mod_use::mod_use![model, error];

/// Provides marjor functions that one will need.
///
/// This is the primary type for using the `auth` module.
/// It will interact with the database, generate and validate records.
#[derive(Clone)]
pub struct AuthClient {
    collection: Collection<PermissionRecord>,
    argon: Argon2<'static>,
}

impl AuthClient {
    /// Create a new [`AuthClient`] with the given [`Collection`].
    pub fn new(collection: Collection<PermissionRecord>) -> AuthClient {
        AuthClient {
            collection,
            argon: Default::default(),
        }
    }

    /// Get the inner [`Collection`].
    pub fn collection(&self) -> Collection<PermissionRecord> {
        self.collection.clone()
    }

    /// List all records in the database.
    pub async fn list(&self) -> Result<Cursor<PermissionRecord>> {
        self.collection.find(None, None).await.map_err(Into::into)
    }

    /// Return the count of records in the database.
    ///
    /// In debug mode, this will do a more expensive but accurate [`count_documents`].
    /// In release mode, this will use [`estimated_document_count`],
    /// which returns metadata of the database without actually call a `find`.
    ///
    /// For more detail, see document of [`count`] and [`countDocuments`] of mongodb.
    ///
    /// [`count_documents`]: Collection::count_documents
    /// [`estimated_document_count`]: Collection::estimated_document_count
    /// [`count`]: https://www.mongodb.com/docs/manual/reference/method/db.collection.count/
    /// [`countDocuments`]: https://www.mongodb.com/docs/manual/reference/method/db.collection.countDocuments/
    pub async fn count(&self) -> Result<u64> {
        if cfg!(debug_assertions) {
            self.collection.count_documents(None, None).await
        } else {
            self.collection.estimated_document_count(None).await
        }
        .map_err(Into::into)
    }

    /// Try insert a new record.
    ///
    /// Return wether the record is inserted.
    /// If one record with same username exists, this will leave it intact.
    pub async fn new_record(
        &self,
        username: impl Into<String>,
        password: impl AsRef<[u8]>,
        permission: PermissionSet,
    ) -> Result<bool> {
        let username = username.into();
        let salt = SaltString::generate(&mut OsRng);
        let hash = self.argon.hash_password(password.as_ref(), &salt)?;

        let record = PermissionRecord::new(hash, username, permission);

        let doc = to_bson(&record)?;
        let res = self
            .collection
            .update_one(
                doc! {
                  "username" : record.username()
                },
                doc! {
                 "$setOnInsert": doc
                },
                UpdateOptions::builder().upsert(true).build(),
            )
            .await?;

        Ok(res.upserted_id.is_some())
    }

    /// Look up permission of a user by username and password.
    ///
    /// When the username and password combination are invalid, this will return [`PermissionSet::EMPTY`].
    pub async fn look_up(
        &self,
        username: impl AsRef<str>,
        password: impl AsRef<[u8]>,
    ) -> Result<PermissionSet> {
        let res = self
            .collection
            .find_one(doc! { "username": username.as_ref() }, None)
            .await?;
        let res = match res {
            Some(rec) if self.validate(&rec.decode()?, password.as_ref()).is_ok() => {
                rec.permissions()
            }
            _ => PermissionSet::EMPTY,
        };
        Ok(res)
    }

    /// Validate if a password is correct
    pub fn validate(&self, hash: &PasswordHash, password: impl AsRef<[u8]>) -> Result<()> {
        self.argon
            .verify_password(password.as_ref(), hash)
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod test {
    use crate::*;

    #[tokio::test]
    async fn test_db() {
        let client = mongodb::Client::with_uri_str(
            std::env::var("MONGODB_URI").unwrap_or_else(|_| "mongodb://localhost:27017".to_owned()),
        )
        .await
        .unwrap();

        let db = client.database("test");
        let col = db.collection("permissions");

        col.drop(None).await.unwrap();

        // Begin testing
        let client = AuthClient::new(col);
        let username = "test_user";
        let password = b"test_password";
        let per = PermissionSet {
            api: Some(Permission::ReadOnly),
            mq: Some(Permission::ReadWrite),
            coordinator: None,
        };

        // New record will be inserted
        let inserted = client.new_record(username, password, per).await.unwrap();
        assert!(inserted);

        // Duplicate record should not be inserted
        let inserted = client
            .new_record(username, password, PermissionSet::EMPTY)
            .await
            .unwrap();
        assert!(!inserted);

        // Now should have one record in db
        let c = client.count().await.unwrap();
        assert_eq!(c, 1);

        // Record in DB should not be modified
        let record = client.list().await.unwrap().next().await.unwrap().unwrap();
        assert_eq!(record.permissions(), per);

        // Valid username and hash combination should return correct permissions
        let res = client.look_up(username, password).await.unwrap();
        assert_eq!(res, per);

        // Invalid username and hash combination should return empty permissions
        let res = client.look_up(username, b"bad_password").await.unwrap();
        assert_eq!(res, PermissionSet::empty());
        let res = client.look_up("bad_username", password).await.unwrap();
        assert_eq!(res, PermissionSet::empty());
        let res = client.look_up("bad_username", b"bad_pw").await.unwrap();
        assert_eq!(res, PermissionSet::empty());

        // Clean up
        client.collection().drop(None).await.unwrap();
    }

    use futures::StreamExt;
}
