//! RPC definitions. Contains all the RPC methods and models, but does not have any implemtentation.
//! For server, see [`rpc::server`](../server/index.html).
//! For client, see [`rpc::client`](../client/index.html).
//!
//! ## Traits
//! Two main traits are defined in this module:
//! - [`Request`](trait.Request.html)
//! - [`Response`](trait.Response.html)
//!
//! ### Request
//! **This trait Requires [`DeserializeOwned`](serde::de::DeserializeOwned)**
//!
//! Used to define a request object which represents a request body sent from client to server.
//! This is usually being represented in JSON as a struct looks like:
//!
//! ```json
//! {
//!    "method": "getUser",
//!    "params": {
//!         "user_id": "7765d13c-35f8-4294-bb6b-6b63b4a4be4d"
//!    }
//! }
//! ```
//!
//! A [`Request`] is always bind with a [`Response`] type.
//! Handler for this request will return the corresponding [`Response`] object,
//! or an [`ApiError`] object represent an error during handling the request.
//!
//! For server, a convenient enum [`Requests`](models::Requests) is generated
//! alone with all request objects to be deserialized from incoming JSON.
//!
//! ### Response
//! **This trait Requires [`Serialize`](serde::ser::Serialize)**
//!
//! Used to define a response payload sent from server to client.
//! All response should be wrapped in [`ResponseObject`], which includes extra information about the response,
//! e.g. time it's being processed and whether it's successful.
//!
//! To construct a [`ResponseObject`], method [`Response::packed`] should be used.
//! It's automatically implemented by [`Response`].
//!
//! ## Helper macros
//!
//! A convenient macro [`methods!`] is defined to generate all RPC methods.
//!
//! [`methods!`] will do following things:
//! - Define a request struct for each RPC method.
//! - Implement [`Request`] for that request struct.
//! - If response object has fields, define it and implement [`Response`] for it.
//! - Generate an enum [`Requests`](models::Requests) with all request objects.
//!
//! **Notice**: This macro **MUST** only be called once in the module,
//! otherwise duplicate definitions of [`Requests`](models::Requests) will be generated.

mod_use::mod_use![wrapper, traits, error];

pub mod models;

#[macro_export]
/// A convenient macro to generate all RPC methods.
///
/// # Example
///
/// ```rust
/// # use api::methods; use sg_core::models::User;
///  methods! {
///     // If response object has fields, define it and implement `Response` for it.
///     "getUserSettings" := GetUserSettings {
///         user_id: String
///     } -> UserSettings {
///         settings: String
///     },
///
///     // If response object is defined elsewhere, do not add brace.
///     // This will only implement the trait instead of re-define it.
///     "getUser" := GetUser {
///         user_id: String
///     } -> User
/// }
/// ```
macro_rules! methods {
    ($(
        $( #[ $method_meta:meta ] )*
        $method:literal :=
        $req:ident {
            $(
                $( #[ $req_field_meta:meta ] )*
                $req_field_name:ident : $req_field_type:ty $(,)?
            )*
        }
        ->
        $resp:ident $({
            $(
                $( #[ $res_field_meta:meta ] )*
                $resp_field_name:ident : $resp_field_type:ty $(,)?
            )*
        })?
        $(,)?
    )*) => {
        $(
            #[derive(Debug, Clone, PartialEq, Eq, ::serde::Serialize, ::serde::Deserialize)]
            $( #[ $method_meta ] )*
            pub struct $req {
                $(
                    $( #[ $req_field_meta ] )*
                    pub $req_field_name : $req_field_type,
                )*
            }

            impl $req {
                #[inline]
                #[allow(clippy::new_without_default)]
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
                    $(
                        $( #[ $res_field_meta ] )*
                        pub $resp_field_name : $resp_field_type,
                    )*
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

                impl $crate::rpc::Response for $resp {
                    fn is_successful(&self) -> bool {
                        true
                    }
                }
            )?
        )*

        #[derive(Debug, Clone, Eq, PartialEq, ::serde::Serialize, ::serde::Deserialize)]
        #[serde(tag = "method", content = "params")]
        #[serde(rename_all = "camelCase")]
        #[non_exhaustive]
        pub enum Requests {
            $( $req($req), )*
            #[serde(other)]
            Unknown
        }
    };
}

#[cfg(test)]
#[allow(dead_code)]
mod test_macro {
    use crate::{
        rpc::{ApiError, Request, Response},
        timestamp,
    };

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
        let now = timestamp();
        let resp = format!(
            r#"{{"data":{{"user_id":"foo","user_info":"bar"}},"success":true,"time":"{}"}}"#,
            now
        );
        let mut resp_obj = User {
            user_id: "foo".to_string(),
            user_info: "bar".to_string(),
        }
        .packed();
        resp_obj.time = now;

        assert_eq!(resp, resp_obj.to_json());
    }

    #[test]
    fn test_serialize_error() {
        let now = timestamp();
        let resp = format!(
            r#"{{"data":{{"error":["User `foo` not found"]}},"success":false,"time":"{}"}}"#,
            now
        );

        let mut resp_obj = ApiError::user_not_found("foo").packed();
        resp_obj.time = now;

        assert_eq!(resp, resp_obj.to_json());
    }
}
