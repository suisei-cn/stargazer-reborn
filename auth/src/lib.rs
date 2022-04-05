use mongodb::{
    bson::{doc, to_bson},
    options::UpdateOptions,
    results::UpdateResult,
    Collection, Cursor,
};

mod_use::mod_use![model, error];

impl AuthClient {
    pub fn new(col: Collection<PermissionRecord>) -> AuthClient {
        AuthClient { col }
    }

    pub async fn list(&self) -> Result<Cursor<PermissionRecord>> {
        self.col.find(None, None).await.map_err(Into::into)
    }

    pub async fn count(&self) -> Result<u64> {
        if cfg!(debug_assertions) {
            self.col.count_documents(None, None).await
        } else {
            self.col.estimated_document_count(None).await
        }
        .map_err(Into::into)
    }

    pub async fn insert_record(&self, record: PermissionRecord) -> Result<UpdateResult> {
        let doc = to_bson(&record)?;
        self.col
            .update_one(
                doc! {
                  "username" : record.username
                },
                doc! {
                 "$setOnInsert": doc
                },
                UpdateOptions::builder().upsert(true).build(),
            )
            .await
            .map_err(Into::into)
    }

    pub async fn validate(&self, username: &str, hash: &str) -> Result<bool> {
        let doc = doc! {
            "username": username,
            "hash": hash
        };
        let cursor = self.col.find_one(doc, None).await?;
        Ok(cursor.is_some())
    }
}

#[cfg(test)]
mod test {
    use futures::{future::ready, StreamExt};

    use crate::*;

    #[tokio::test]
    async fn test_db() {
        let client = mongodb::Client::with_uri_str("mongodb://localhost:27017")
            .await
            .unwrap();
        let db = client.database("test");
        let col = db.collection("permissions");
        col.drop(None).await.unwrap();
        let client = AuthClient::new(col);
        let record = PermissionRecord {
            username: "test".to_string(),
            hash: "test".to_string(),
            permissions: PermissionSet {
                api: Some(Permission::ReadOnly),
                method: Some(Permission::ReadWrite),
                coordinator: None,
            },
        };
        let res = client.insert_record(record.clone()).await.unwrap();
        assert!(res.upserted_id.is_some());
        assert_eq!(res.matched_count, 0);
        assert_eq!(res.modified_count, 0);

        client
            .list()
            .await
            .unwrap()
            .for_each(|record| {
                println!("{:?}", record);
                println!("{}", serde_json::to_string(&record.unwrap()).unwrap());
                ready(())
            })
            .await;

        let res = client.insert_record(record).await.unwrap();

        let c = client.count().await.unwrap();
        assert!(res.upserted_id.is_none());
        assert_eq!(res.matched_count, 1);
        assert_eq!(res.modified_count, 0);
        assert_eq!(c, 1)
    }
}
