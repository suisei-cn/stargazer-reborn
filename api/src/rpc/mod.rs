mod_use::mod_use![wrapper, traits, error];

pub mod models;

#[macro_export]
macro_rules! methods {
    ($(
        $method:literal :=
        $req:ident { $( $req_field_name:ident : $req_field_type:ty ),* }
        ->
        $resp:ident $( {  $( $resp_field_name:ident : $resp_field_type:ty ),*  } )?
        $(,)?
    )*) => {
        $(
            #[derive(Debug, Clone, PartialEq, Eq, ::serde::Serialize, ::serde::Deserialize)]
            pub struct $req {
                $(pub $req_field_name : $req_field_type, )*
            }

            impl $req {
                #[inline]
                pub fn new($( $req_field_name : $req_field_type, )*) -> Self {
                    Self {
                        $( $req_field_name, )*
                    }
                }
            }

            impl $crate::rpc::Request for $req {
                const METHOD: &'static str = $method;
                type Res = $resp;
            }

            $(
                #[derive(Debug, Clone, PartialEq, Eq, ::serde::Serialize, ::serde::Deserialize)]
                pub struct $resp {
                    $(pub $resp_field_name : $resp_field_type, )*
                }


                impl $resp {
                    #[inline]
                    #[allow(clippy::new_without_default)]
                    pub fn new($( $resp_field_name : $resp_field_type, )*) -> Self {
                        Self {
                            $( $resp_field_name, )*
                        }
                    }
                }
            )?

            impl $crate::rpc::Response for $resp {
                fn is_successful(&self) -> bool {
                    true
                }
            }
        )*

        #[derive(Debug, Clone, Eq, PartialEq, ::serde::Serialize, ::serde::Deserialize)]
        #[serde(tag = "method", content = "params")]
        #[serde(rename_all = "camelCase")]
        #[non_exhaustive]
        pub enum Requests {
            $( $req($req) ),*
        }
    };
}

#[cfg(test)]
#[allow(dead_code)]
mod test_macro {
    use crate::rpc::{ApiError, Request, Response};

    crate::methods! {
        "getUser" :=
        GetUser {
            user_id: String
        } -> User {
            user_id: String,
            user_info: String
        }
    }

    #[test]
    fn test_gen() {
        assert_eq!(GetUser::METHOD, "getUser");
    }

    #[test]
    fn test_parse_param() {
        let req = r#"{"method":"getUser","params":{"user_id":"foo"}}"#;
        let req_obj = GetUser {
            user_id: "foo".to_string(),
        };
        let req_wrapped = Requests::GetUser(req_obj);

        assert_eq!(req_wrapped, serde_json::from_str(req).unwrap());

        let req = r#"{"method":"getUser","params":{"user_foo":"bar"}}"#;

        assert!(serde_json::from_str::<Requests>(req).is_err());
    }

    #[test]
    fn test_serialize_success() {
        let resp = r#"{"data":{"user_id":"foo","user_info":"bar"},"success":true,"time":0}"#;
        let mut resp_obj = User {
            user_id: "foo".to_string(),
            user_info: "bar".to_string(),
        }
        .packed();
        resp_obj.time = 0;

        assert_eq!(resp, resp_obj.to_json());
    }

    #[test]
    fn test_serialize_error() {
        let resp = r#"{"data":{"error":["User `foo` not found"]},"success":false,"time":0}"#;

        let mut resp_obj = ApiError::user_not_found("foo").packed();
        resp_obj.time = 0;

        assert_eq!(resp, resp_obj.to_json());
    }
}
