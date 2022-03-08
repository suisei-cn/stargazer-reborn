mod_use::mod_use![wrapper, traits, error];

pub mod model;

#[macro_export]
macro_rules! new_method {
    ($method:literal :=
        $req:ident { $( $req_field_name:ident : $req_field_type:ty ),* }
        =>
        $resp:ident { $( $resp_field_name:ident : $resp_field_type:ty ),* }
    ) => {

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

        #[derive(Debug, Clone, PartialEq, Eq, ::serde::Serialize, ::serde::Deserialize)]
        pub struct $resp {
            $(pub $resp_field_name : $resp_field_type, )*
        }

        impl $resp {
            #[inline]
            pub fn new($( $resp_field_name : $resp_field_type, )*) -> Self {
                Self {
                    $( $resp_field_name, )*
                }
            }

            pub fn packed(self) -> $crate::rpc::ResponseObject<Self> {
                let success = $crate::rpc::Response::is_successful(&self);
                    $crate::rpc::ResponseObject::new(self, success)
            }
        }

        impl $crate::rpc::Response for $resp {
            fn is_successful(&self) -> bool {
                true
            }
        }

        impl ::axum::response::IntoResponse for $resp {
            fn into_response(self) -> ::axum::response::Response {
                self.packed().into_response()
            }
        }
    };
}
