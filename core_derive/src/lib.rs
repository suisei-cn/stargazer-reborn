use darling::{
    ast::Data,
    util::{Flag, Ignored, Override, SpannedValue},
    Error,
    FromDeriveInput,
    FromField,
    FromMeta,
};
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, Ident, Path, PathSegment, Type};

fn default_core_crate() -> Path {
    syn::parse_str("sg_core").expect("a path")
}

#[derive(Debug, FromDeriveInput)]
#[darling(attributes(config), supports(struct_named))]
struct ConfigStruct {
    ident: Ident,
    data: Data<Ignored, ConfigField>,
    #[darling(default = "default_core_crate")]
    core: Path,
}

#[derive(Debug, FromField)]
#[darling(attributes(config))]
struct ConfigField {
    ident: Option<Ident>,
    default: Option<Override<String>>,
    default_str: Option<SpannedValue<String>>,
    inherit: Option<SpannedValue<Override<InheritAttr>>>,
    ty: Type,
}

#[derive(Debug, FromMeta)]
struct InheritAttr {
    flatten: Flag,
}

trait InheritAttrExt {
    fn is_flatten(&self) -> bool;
}

impl InheritAttrExt for Override<InheritAttr> {
    fn is_flatten(&self) -> bool {
        matches!(self, Self::Explicit(InheritAttr { flatten }) if flatten.is_present())
    }
}

macro_rules! tri {
    ($e:expr) => {
        match $e {
            Ok(v) => v,
            Err(e) => return TokenStream::from(e.write_errors()),
        }
    };
}

fn serde_json_crate(core_crate: Path) -> Path {
    let Path {
        leading_colon,
        mut segments,
        ..
    } = core_crate;
    segments.extend([
        PathSegment::from(Ident::new("utils", proc_macro2::Span::call_site())),
        PathSegment::from(Ident::new("serde_json", proc_macro2::Span::call_site())),
    ]);
    Path {
        leading_colon,
        segments,
    }
}

fn value_from_str(serde_json: &Path, v: &str) -> proc_macro2::TokenStream {
    quote! {#serde_json::Value::String(#v.to_string())}
}

fn value_from_json_str(serde_json: &Path, v: &str) -> proc_macro2::TokenStream {
    quote! {
        #serde_json::from_str::<#serde_json::Value>(#v)
            .expect("Given string literal is not a valid json value.")
    }
}

fn value_from_default_serialized(serde_json: &Path, ty: &Type) -> proc_macro2::TokenStream {
    quote! {
        #serde_json::to_value(<#ty as Default>::default())
            .expect("Given expression can't be serialized into a json value.")
    }
}

fn value_from_config_trait(core_crate: &Path, ty: &Type) -> proc_macro2::TokenStream {
    quote! {
        <#ty as #core_crate::utils::ConfigDefault>::config_defaults()
    }
}

fn action_from_default(
    serde_json: &Path,
    default: &Override<String>,
    ident: &Ident,
    ty: &Type,
    flatten: bool,
) -> Action {
    let key = ident.to_string();
    let action = Action::Append(Field {
        key,
        value: match default {
            Override::Inherit => value_from_default_serialized(serde_json, ty),
            Override::Explicit(v) => value_from_json_str(serde_json, v),
        },
    });
    if flatten {
        Action::Merge(value_from_actions(serde_json, [action]))
    } else {
        action
    }
}

fn action_from_inherit(core_crate: &Path, ident: &Ident, ty: &Type, flatten: bool) -> Action {
    let key = ident.to_string();
    let value = value_from_config_trait(core_crate, ty);
    if flatten {
        Action::Merge(value)
    } else {
        Action::Append(Field { key, value })
    }
}

struct Field {
    key: String,
    value: proc_macro2::TokenStream,
}

enum Action {
    Append(Field),
    Merge(proc_macro2::TokenStream),
    Wrapped(String, Vec<Action>),
}

fn value_from_actions(
    serde_json: &Path,
    actions: impl IntoIterator<Item = Action>,
) -> proc_macro2::TokenStream {
    let stmts: Vec<_> = actions
        .into_iter()
        .map(|action| match action {
            Action::Append(field) => {
                let Field { key, value } = field;
                quote! {dict.insert(#key.to_string(), #value);}
            }
            Action::Merge(value) => {
                quote! {
                    if let #serde_json::Value::Object(map) = #value {
                        dict.extend(map);
                    } else {
                        panic!("Invariant not hold: #value.config_defaults must be an object.");
                    }
                }
            }
            Action::Wrapped(key, actions) => {
                let actions = wrap_in_object(serde_json, &value_from_actions(serde_json, actions));
                quote! {
                    dict.insert(#key.to_string(), #actions);
                }
            }
        })
        .collect();

    quote! {
        {
            {
                let mut dict = #serde_json::Map::new();
                #(#stmts)*
                dict
            }
        }
    }
}

fn wrap_in_object(serde_json: &Path, dict: &proc_macro2::TokenStream) -> proc_macro2::TokenStream {
    quote! {
        #serde_json::Value::Object(#dict)
    }
}

/// Example of user-defined [derive mode macro][1]
///
/// [1]: https://doc.rust-lang.org/reference/procedural-macros.html#derive-mode-macros
#[proc_macro_derive(Config, attributes(config))]
pub fn derive_config(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let input = tri!(ConfigStruct::from_derive_input(&input));
    let core_crate = input.core;
    let serde_json = serde_json_crate(core_crate.clone());
    let actions: Vec<_> = input
        .data
        .take_struct()
        .expect("a struct")
        .fields
        .into_iter()
        .flat_map(
            |ConfigField {
                 ident,
                 default,
                 default_str,
                 inherit,
                 ty,
             }| {
                let ident = ident.expect("a named field");
                let key = ident.to_string();
                match (default, default_str, inherit) {
                    (Some(_), Some(default_str), _) => vec![Action::Append(Field {
                        key,
                        value: Error::custom("Cannot set both `default` and `default_str`")
                            .with_span(&default_str)
                            .write_errors(),
                    })],
                    (_, Some(_), Some(inherit)) => vec![Action::Append(Field {
                        key,
                        value: Error::custom("Cannot set both `default_str` and `inherit`")
                            .with_span(&inherit)
                            .write_errors(),
                    })],
                    // Only `default_str` is present.
                    (None, Some(default_str), None) => vec![Action::Append(Field {
                        key,
                        value: value_from_str(&serde_json, &default_str),
                    })],
                    // Only `default` is present.
                    (Some(default), None, None) => {
                        vec![action_from_default(
                            &serde_json,
                            &default,
                            &ident,
                            &ty,
                            false,
                        )]
                    }
                    // Both `inherit` and `default` are present.
                    (Some(default), None, Some(inherit)) => {
                        let flatten = inherit.is_flatten();
                        vec![
                            action_from_inherit(&core_crate, &ident, &ty, flatten),
                            action_from_default(&serde_json, &default, &ident, &ty, flatten),
                        ]
                    }
                    // Only `inherit` is present.
                    (None, None, Some(inherit)) => {
                        let flatten = inherit.is_flatten();
                        vec![action_from_inherit(&core_crate, &ident, &ty, flatten)]
                    }
                    // No attributes are present.
                    (None, None, None) => vec![],
                }
            },
        )
        .collect();

    let value = wrap_in_object(&serde_json, &value_from_actions(&serde_json, actions));

    let struct_ident = input.ident;
    let tokens = quote! {
        impl #core_crate::utils::ConfigDefault for #struct_ident {
            fn config_defaults() -> #core_crate::utils::serde_json::Value {
                #value
            }
        }
    };

    tokens.into()
}
