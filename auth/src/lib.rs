use mongodb::{
    bson::{doc, to_bson},
    options::UpdateOptions,
    Collection, Cursor,
};
use pbkdf2::{
    password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
    Pbkdf2,
};

mod_use::mod_use![model, error];

#[derive(Clone, Debug)]
pub struct AuthClient {
    pub(crate) collection: Collection<PermissionRecord>,
}

impl AuthClient {
    pub fn new(collection: Collection<PermissionRecord>) -> AuthClient {
        AuthClient { collection }
    }

    pub fn into_collection(self) -> Collection<PermissionRecord> {
        self.collection
    }

    pub async fn list(&self) -> Result<Cursor<PermissionRecord>> {
        self.collection.find(None, None).await.map_err(Into::into)
    }

    pub async fn count(&self) -> Result<u64> {
        if cfg!(debug_assertions) {
            self.collection.count_documents(None, None).await
        } else {
            self.collection.estimated_document_count(None).await
        }
        .map_err(Into::into)
    }

    pub async fn new_record(
        &self,
        username: impl Into<String>,
        password: &[u8],
        permission: PermissionSet,
    ) -> Result<bool> {
        let username = username.into();
        let salt = SaltString::generate(&mut OsRng);
        let hash = Pbkdf2.hash_password(password, &salt)?;

        let record = PermissionRecord::new(hash, username, permission);

        let doc = to_bson(&record)?;
        let res = self
            .collection
            .update_one(
                doc! {
                  "username" : &record.username
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
    pub async fn look_up(
        &self,
        username: impl AsRef<str>,
        password: &[u8],
    ) -> Result<PermissionSet> {
        let res = self
            .collection
            .find_one(doc! { "username": username.as_ref() }, None)
            .await?;
        let res = match res {
            Some(rec) if rec.validate(password).is_ok() => rec.permissions,
            _ => PermissionSet::EMPTY,
        };
        Ok(res)
    }
}

#[cfg(test)]
mod test {
    use crate::*;

    #[tokio::test]
    async fn test_db() {
        let client = mongodb::Client::with_uri_str("mongodb://localhost:27017")
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
            method: Some(Permission::ReadWrite),
            coordinator: None,
        };
        // New record will be inserted
        let inserted = client.new_record(username, password, per).await.unwrap();
        assert!(inserted);

        // Duplicate record should not be inserted
        let inserted = client.new_record(username, password, per).await.unwrap();
        assert!(!inserted);

        // Now should have one record in db
        let c = client.count().await.unwrap();
        assert_eq!(c, 1);

        // Valid username and hash combination should return correct permissions
        let res = client.look_up(username, password).await.unwrap();
        assert_eq!(res, per);

        // Invalid username and hash combination should return empty permissions
        let res = client.look_up(username, b"bad_password").await.unwrap();
        assert_eq!(res, PermissionSet::empty());

        // Clean up
        client.into_collection().drop(None).await.unwrap();
    }
}
