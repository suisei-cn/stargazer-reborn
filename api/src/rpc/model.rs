crate::new_method! {
    "getUser",
    GetUser {
        user_id: String
    },
    success: [
        UserFound {
            user_id: String,
            user_info: String
        },
        User {
            user_id: String,
            user_info: String
        }

    ],
    failed: [
        UserNotFound ,
        Unauthorized {
            expired: bool
        }
    ]
}

#[cfg(test)]
mod test_macro {
    use crate::{model::*, timestamp, Request, Requests};

    #[test]
    fn test_gen() {
        let req = r#"{"method":"getUser","params":{"user_id":"foo"}}"#;
        let req_obj = GetUser {
            user_id: "foo".to_string(),
        };
        let req_wrapped = Requests::GetUser(req_obj);

        assert_eq!(req, serde_json::to_string(&req_wrapped).unwrap());

        assert_eq!(GetUser::METHOD, "getUser");

        let ts = timestamp();
        let resp = format!(
            "{{\"data\":{{\"user_id\":\"foo\",\"user_info\":\"bar\"}},\"success\":true,\"time\":{ts}}}",
        );
        let resp_obj = GetUserResp::UserFound(UserFound {
            user_id: "foo".to_string(),
            user_info: "bar".to_string(),
        });

        assert_eq!(resp, serde_json::to_string(&resp_obj.packed()).unwrap(),);

        let ts = timestamp();
        let resp = format!(
            "{{\"data\":{{\"errors\":[\"UserNotFound\"]}},\"success\":false,\"time\":{ts}}}",
        );
        let resp_obj = GetUserResp::UserNotFound(UserNotFound::new());

        assert_eq!(resp, serde_json::to_string(&resp_obj.packed()).unwrap(),);
    }
}
