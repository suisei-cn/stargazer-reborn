//! RPC definitions. Contains all the RPC methods and model, but does not have any implemtentation.
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
//!
//! In order to invoke the method, send a POST http request to `/v1/:method_name` with request param as the body.
//!
//! Here is [all defined methods](model)
//!
//! A [`Request`] is always bind with a [`Response`] type.
//! Handler for this request will return the corresponding [`Response`] object,
//! or an [`ApiError`] object represent an error during handling the request.
//!
//! For server, a convenient enum [`Requests`](model::Requests) is generated
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
//! - If `client` feature is enabled, generate methods for [`Client`](crate::client::Client) to invoke RPC methods.
//!
//! **Notice**: This macro **MUST** only be called once in the module,
//! otherwise duplicate definitions of [`Requests`](model::Requests) will be generated.

mod_use::mod_use![wrapper, traits, error, ext];

pub mod model;

/// A convenient macro to generate all RPC methods.
///
/// # Example
///
/// ```rust
/// # use api::methods; use sg_core::model::User;
///  methods! {
///     // If response object has fields, define it and implement `Response` for it.
///     get_user := GetUserSettings {
///         user_id: String
///     } -> UserSettings {
///         settings: String
///     },
///
///     // If response object is defined elsewhere, do not add brace.
///     // This will only implement the trait instead of re-define it.
///     get_user := GetUser {
///         user_id: String
///     } -> User
/// }
/// ```
#[macro_export]
macro_rules! methods {
    ($(
        $( #[ $method_meta:meta ] )*
        $method:ident :=
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
            #[doc = concat!("Request param of RPC method `", stringify!($method), "`.")]
            #[doc = ""]
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
                #[must_use]
                pub const fn new($( $req_field_name : $req_field_type, )*) -> Self {
                    Self {
                        $( $req_field_name, )*
                    }
                }
            }

            impl $crate::rpc::Request for $req {
                const METHOD: &'static str = stringify!($method);
                type Res = $resp;
            }

            $(
                #[doc = concat!("Response of RPC method [`", stringify!($method), "`](", stringify!($req), ").")]
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
                    #[must_use]
                    pub const fn new($( $resp_field_name : $resp_field_type, )*) -> Self {
                        Self {
                            $( $resp_field_name, )*
                        }
                    }
                }

                $crate::successful_response!($resp);
            )?
        )*

        #[test]
        fn test_requests_size() {
            use ::std::mem::size_of;
            $(
                println!(
                    "{} ({}B) -> {} ({}B)",
                    stringify!($req),
                    size_of::<$req>(),
                    stringify!($resp),
                    size_of::<$resp>()
                );
            )*
        }

        #[cfg(any(feature = "client", feature = "client_blocking"))]
        use $crate::{ApiResult, client::Result as ClientResult};

        #[cfg(feature = "client")]
        #[allow(clippy::missing_errors_doc)]
        impl $crate::client::Client {
            $(
                $( #[ $method_meta ] )*
                ///
                /// # Errors
                /// This returns a [`ClientResult`] that represents error occurred on client side,
                /// like network error or serialization error.
                /// It then wraps around an [`ApiResult`] which represents error occurred on server side.
                pub async fn $method (&self, $( $req_field_name : $req_field_type,)* ) -> ClientResult<ApiResult<$resp>> {
                    self.invoke($req { $( $req_field_name, )* }).await
                }
            )*
        }

        #[cfg(feature = "client_blocking")]
        #[allow(clippy::missing_errors_doc)]
        impl $crate::client::blocking::Client {
            $(
                $( #[ $method_meta ] )*
                ///
                /// # Errors
                /// This returns a [`ClientResult`] that represents error occurred on client side,
                /// like network error or serialization error.
                /// It then wraps around an [`ApiResult`] which represents error occurred on server side.
                pub fn $method (&self, $( $req_field_name : $req_field_type,)* ) -> ClientResult<ApiResult<$resp>> {
                    self.invoke(& $req { $( $req_field_name, )* })
                }
            )*
        }
    };
}

/// Implement [`Response`] for a series of types.s
/// All of them are successful.
///
/// # Example
/// ```rust
/// # use api::successful_response;
/// #[derive(Debug, Clone, Eq, PartialEq, ::serde::Serialize, ::serde::Deserialize)]
/// struct Foo { foo: String };
/// #[derive(Debug, Clone, Eq, PartialEq, ::serde::Serialize, ::serde::Deserialize)]
/// struct Bar { bar: usize };
///
/// successful_response![Foo, Bar];
/// ```
#[macro_export]
macro_rules! successful_response {
    [ $( $ty:ty ),* ] => {
        $(
            impl $crate::rpc::Response for $ty {
                fn status(&self) -> ::http::StatusCode {
                    ::http::StatusCode::OK
                }
            }
        )*
    };
}

#[cfg(test)]
#[allow(dead_code)]
mod test_macro {
    use mongodb::bson::Uuid;

    use crate::{
        rpc::{ApiError, Request, Response},
        timestamp,
    };

    crate::methods! {
        get_user :=
        GetUser {
            user_id: String
        } -> User {
            user_id: String,
            user_info: String
        }
    }

    #[test]
    fn test_gen() {
        assert_eq!(GetUser::METHOD, "get_user");
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
    fn test_serialize_api_error() {
        let now = timestamp();
        let id = "26721d57-37f5-458c-afea-2b18baf34925";
        let resp = format!(
            r#"{{"data":{{"error":["Cannot find user with ID `{id}`"]}},"success":false,"time":"{now}"}}"#,
        );

        let mut resp_obj = ApiError::user_not_found_with_id(&Uuid::parse_str(id).unwrap()).packed();
        resp_obj.time = now;

        assert_eq!(resp, resp_obj.to_json());
    }
}
