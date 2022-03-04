mod_use::mod_use![wrapper, traits];

pub mod model;

#[macro_export]
macro_rules! new_method {
    ($method:literal,
        $name:ident { $( $field_name:ident : $field_type:ty ),* }
        $(,)?
        success: [
            $(
                $success_resp:ident {
                    $( $success_resp_field_name:ident : $success_resp_field_type:ty ), *
                }
                $(,)?
            ),*
        ]
        $(,)?
        failed: [
            $(
                $failed_resp:ident
                $([ $( $reason:literal $(,)? ) * ])?
                $({ $( $failed_resp_field_name:ident : $failed_resp_field_type:ty ), * })?
                 $(,)?
            ),*
        ]
    ) => {
        ::paste::paste! {
            #[derive(Debug, Clone, PartialEq, Eq, ::serde::Serialize, ::serde::Deserialize)]
            pub struct $name {
                $( pub $field_name : $field_type, )*
            }

            impl $crate::Request for $name {
                const METHOD: &'static str = $method;
                type Res = [<$name Resp>];
            }

            $(
                #[derive(Debug, Clone, PartialEq, Eq, ::serde::Serialize, ::serde::Deserialize)]
                pub struct $success_resp {
                    $( $success_resp_field_name : $success_resp_field_type, )*
                }

                impl $success_resp {
                    #[inline]
                    #[allow(clippy::new_without_default)]
                    pub fn new($( $success_resp_field_name : $success_resp_field_type, )*) -> Self {
                        Self {
                            $( $success_resp_field_name, )*
                        }
                    }
                }
            )*

            $(
                #[derive(Debug, Clone, PartialEq, Eq, ::serde::Serialize)]
                pub struct $failed_resp {
                    $( pub $( $failed_resp_field_name : $failed_resp_field_type, )* )?
                    errors: Vec<String>
                }

                impl $failed_resp {
                    #[inline]
                    #[allow(clippy::new_without_default)]
                    pub fn new($( $( $failed_resp_field_name : $failed_resp_field_type, )* )?) -> Self {
                        Self {
                            errors: vec![
                                stringify!($failed_resp).to_owned(),
                                $( $($reason .to_owned() ,)*)?
                            ],
                            $( $( $failed_resp_field_name, )* )?
                        }
                    }

                    #[inline]
                    pub fn with_reason(&mut self, error: String) -> &mut Self {
                        self.errors.push(error);
                        self
                    }

                    #[inline]
                    pub fn with_reasons(&mut self, error: &[String]) -> &mut Self {
                        self.errors.extend_from_slice(error);
                        self
                    }
                }
            )*

            #[derive(Debug, Clone, PartialEq, Eq, ::serde::Serialize)]
            #[serde(untagged)]
            pub enum [<$name Resp>] {
                $(
                    $success_resp ( $success_resp ),
                )*
                $(
                    $failed_resp ( $failed_resp ),
                )*
            }

            impl [<$name Resp>] {
                pub fn packed(self) -> $crate::ResponseObject<Self> {
                    let success = $crate::Response::is_successful(&self);
                    $crate::ResponseObject::new(self, success)
                }
            }

            impl $crate::Response for [<$name Resp>] {
                fn is_successful(&self) -> bool {
                    match self {
                        $(
                            [<$name Resp>]::$success_resp{ .. } => true,
                        )*
                        $(
                            [<$name Resp>]::$failed_resp{ .. } => false,
                        )*
                    }
                }
            }
        }
    };
}
