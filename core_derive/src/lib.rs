use proc_macro::TokenStream;

use darling::ast::Data;
use darling::util::{Flag, Ignored, Override, SpannedValue};
use darling::{Error, FromDeriveInput, FromField};
use quote::{quote, ToTokens};
use syn::Ident;
use syn::{parse_macro_input, DeriveInput, Path, Type};

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
    inherit: Flag,
    ty: Type,
}

macro_rules! tri {
    ($e:expr) => {
        match $e {
            Ok(v) => v,
            Err(e) => return TokenStream::from(e.write_errors()),
        }
    };
}

fn value_from_json_str(core_crate: &Path, v: &str) -> proc_macro2::TokenStream {
    quote! {
        #core_crate::utils::serde_json::from_str::<#core_crate::utils::serde_json::Value>(#v)
            .expect("Given string literal is not a valid json value.")
    }
}

fn value_from_default_serialized(core_crate: &Path, ty: &Type) -> proc_macro2::TokenStream {
    quote! {
        #core_crate::utils::serde_json::to_value(<#ty as Default>::default())
            .expect("Given expression can't be serialized into a json value.")
    }
}

fn value_from_config_trait(core_crate: &Path, ty: &Type) -> proc_macro2::TokenStream {
    quote! {
        <#ty as #core_crate::utils::ConfigDefault>::config_defaults()
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
    let (idents, exprs): (Vec<_>, Vec<_>) = input
        .data
        .take_struct()
        .expect("a struct")
        .fields
        .into_iter()
        .filter_map(
            |ConfigField {
                 ident,
                 default,
                 default_str,
                 inherit,
                 ty,
             }| {
                let is_inherit = inherit.is_present();
                let default = match (default, default_str, is_inherit) {
                    (Some(_), Some(default_str), _) => Some(
                        Error::custom("Cannot set both `default` and `default_str`")
                            .with_span(&default_str)
                            .write_errors(),
                    ),
                    (Some(_), None, true) => Some(
                        Error::custom("Cannot set both `default` and `inherit`")
                            .with_span(&inherit)
                            .write_errors(),
                    ),
                    (None, Some(_), true) => Some(
                        Error::custom("Cannot set both `default_str` and `inherit`")
                            .with_span(&inherit)
                            .write_errors(),
                    ),
                    // Only `default` is present and has an explicit value.
                    (Some(Override::Explicit(default)), _, false) => {
                        Some(value_from_json_str(&core_crate, &default))
                    }
                    // Only `default` is present and has an implicit value.
                    (Some(Override::Inherit), _, false) => {
                        Some(value_from_default_serialized(&core_crate, &ty))
                    }
                    // Only `default_str` is present.
                    (_, Some(default_str), false) => Some(default_str.to_token_stream()),
                    // Only `inherit` is present.
                    (None, None, true) => Some(value_from_config_trait(&core_crate, &ty)),
                    // No attributes are present.
                    (None, None, false) => None,
                };
                Some((ident.expect("a named field").to_string(), default?))
            },
        )
        .unzip();

    let json_expr = quote! {
        #core_crate::utils::serde_json::json!({
            #(
                #idents: #exprs
            ),*
        })
    };

    let struct_ident = input.ident;
    let tokens = quote! {
        impl #core_crate::utils::ConfigDefault for #struct_ident {
            fn config_defaults() -> #core_crate::utils::serde_json::Value {
                #json_expr
            }
        }
    };

    tokens.into()
}
