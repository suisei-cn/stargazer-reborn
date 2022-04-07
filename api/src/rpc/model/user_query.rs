use mongodb::bson::{doc, Document, Uuid};

use crate::ApiError;

/// Two ways of query a user:
///
/// - By IM: use `im` and `im_payload` to find the corresponding user. This is usually used by the bot.
/// - By ID: use `id` to find the corresponding user. This is usually used by the admin.
#[must_use]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum UserQuery {
    ById { user_id: Uuid },
    ByIm { im: String, im_payload: String },
}

impl UserQuery {
    #[must_use]
    pub fn as_document(&self) -> Document {
        match self {
            UserQuery::ById { user_id: id } => doc! { "id": id },
            UserQuery::ByIm { im, im_payload } => doc! { "im": im, "im_payload": im_payload },
        }
    }

    pub fn as_error(&self) -> ApiError {
        match self {
            UserQuery::ById { user_id: id } => ApiError::user_not_found_with_id(id),
            UserQuery::ByIm { im, im_payload } => ApiError::user_not_found_with_im(im, im_payload),
        }
    }
}

#[cfg(test)]
mod test {
    use mongodb::bson::Uuid;

    use crate::model::UserQuery;

    #[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]

    struct Test {
        #[serde(flatten)]
        query: UserQuery,
    }

    #[test]
    fn test_user_query() {
        let obj = serde_json::json!({
            "user_id": "5e9f8f8f-f8f8-f8f8-f8f8-f8f8f8f8f8f8",
        });

        let test: Test = serde_json::from_value(obj).unwrap();
        assert_eq!(
            test.query,
            UserQuery::ById {
                user_id: Uuid::parse_str("5e9f8f8f-f8f8-f8f8-f8f8-f8f8f8f8f8f8").unwrap()
            }
        );
    }
}
