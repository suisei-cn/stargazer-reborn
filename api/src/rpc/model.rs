crate::new_method! {
    "getUser" :=
    GetUser {
        user_id: String
    } => User {
        user_id: String,
        user_info: String
    }
}

#[cfg(test)]
mod test_macro {
    use crate::{
        rpc::{
            model::{GetUser, User},
            ApiError, Request, Requests,
        },
        timestamp,
    };

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
        let resp_obj = User {
            user_id: "foo".to_string(),
            user_info: "bar".to_string(),
        };

        assert_eq!(resp, serde_json::to_string(&resp_obj.packed()).unwrap(),);

        let ts = timestamp();
        let resp = format!(
            "{{\"data\":{{\"error\":[\"User `foo` not found\"]}},\"success\":false,\"time\":{ts}}}",
        );
        let resp_obj = ApiError::user_not_found("foo").packed();

        assert_eq!(resp, serde_json::to_string(&resp_obj).unwrap(),);
    }
}
