use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Expr, Fields, Lit};

#[proc_macro_attribute]
pub fn error(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);

    // Expect an enum as input
    let enum_name = input.ident;
    let data = match input.data {
        Data::Enum(data) => data,
        _ => panic!("define_backend_error can only be applied to enums"),
    };

    let variants_def = data
        .variants
        .iter()
        .map(|v| {
            let variant_ident = &v.ident;
            match &v.fields {
                Fields::Unit => quote! { #variant_ident },
                Fields::Unnamed(fields) => {
                    let types = fields.unnamed.iter().map(|f| &f.ty);
                    quote! { #variant_ident(#(#types),*) }
                }
                Fields::Named(fields) => {
                    let field_defs = fields.named.iter().map(|f| {
                        let name = &f.ident;
                        let ty = &f.ty;
                        quote! { #name: #ty }
                    });
                    quote! { #variant_ident { #(#field_defs),* } }
                }
            }
        })
        .collect::<Vec<_>>();

    let matches = data
        .variants
        .iter()
        .map(|variant| {
            let variant_ident = &variant.ident;
            let attrs = parse_baxe_attributes(variant);
            let (status, tag, code, message) = (attrs.status, attrs.tag, attrs.code, attrs.message);

            let pattern = match &variant.fields {
                Fields::Unit => quote!(#enum_name::#variant_ident),
                Fields::Unnamed(_) => quote!(#enum_name::#variant_ident(..)),
                Fields::Named(_) => quote!(#enum_name::#variant_ident{..}),
            };

            (pattern, status, tag, code, message)
        })
        .collect::<Vec<_>>();

    let (patterns, statuses, tags, codes, messages): (Vec<_>, Vec<_>, Vec<_>, Vec<_>, Vec<_>) =
        matches.into_iter().unzip_n_vec();

    let expanded = quote! {
        #[derive(Debug)]
        pub enum #enum_name {
            #(#variants_def,)*
        }

        impl std::fmt::Display for #enum_name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    #(#patterns => write!(f, #messages),)*
                }
            }
        }

        impl std::error::Error for #enum_name {}

        impl baxe::BackendError for #enum_name {
            fn to_status_code(&self) -> axum::http::StatusCode {
                match self {
                    #(#patterns => #statuses,)*
                }
            }

            fn to_error_tag(&self) -> &str {
                match self {
                    #(#patterns => #tags,)*
                }
            }

            fn to_error_code(&self) -> u16 {
                match self {
                    #(#patterns => #codes,)*
                }
            }
        }

        impl From<#enum_name> for baxe::BaxeError {
            fn from(error: #enum_name) -> Self {
                let status = error.to_status_code();
                baxe::BaxeError {
                    status_code: status,  // Store it separately since it's skipped in serialization
                    error_tag: error.to_error_tag().to_string(),
                    code: error.to_error_code(),
                    message: error.to_string(),
                }
            }
        }

        impl IntoResponse for #enum_name {
            fn into_response(self) -> Response {
                (self.to_status_code(), Json(baxe::BaxeError::from(self))).into_response()
            }
        }
    };

    TokenStream::from(expanded)
}

struct BaxeAttributes {
    status: proc_macro2::TokenStream,
    tag: proc_macro2::TokenStream,
    code: proc_macro2::TokenStream,
    message: proc_macro2::TokenStream,
}

fn parse_baxe_attributes(variant: &syn::Variant) -> BaxeAttributes {
    let mut status = None;
    let mut tag = None;
    let mut code = None;
    let mut message = None;

    for attr in &variant.attrs {
        if attr.path().is_ident("baxe") {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("status") {
                    let value = meta.value()?;
                    status = Some(value.parse::<Expr>()?);
                } else if meta.path.is_ident("tag") {
                    let value = meta.value()?;
                    if let Lit::Str(lit) = value.parse()? {
                        tag = Some(lit.value());
                    }
                } else if meta.path.is_ident("code") {
                    let value = meta.value()?;
                    code = Some(value.parse::<Expr>()?);
                } else if meta.path.is_ident("message") {
                    let value = meta.value()?;
                    if let Lit::Str(lit) = value.parse()? {
                        message = Some(lit.value());
                    }
                }
                Ok(())
            })
            .unwrap();
        }
    }

    BaxeAttributes {
        status: quote!(#status),
        tag: quote!(#tag),
        code: quote!(#code),
        message: quote!(#message),
    }
}

trait UnzipN<T1, T2, T3, T4, T5> {
    fn unzip_n_vec(self) -> (Vec<T1>, Vec<T2>, Vec<T3>, Vec<T4>, Vec<T5>);
}

impl<T1, T2, T3, T4, T5, I: Iterator<Item = (T1, T2, T3, T4, T5)>> UnzipN<T1, T2, T3, T4, T5>
    for I
{
    fn unzip_n_vec(self) -> (Vec<T1>, Vec<T2>, Vec<T3>, Vec<T4>, Vec<T5>) {
        let mut t1 = Vec::new();
        let mut t2 = Vec::new();
        let mut t3 = Vec::new();
        let mut t4 = Vec::new();
        let mut t5 = Vec::new();

        for (x1, x2, x3, x4, x5) in self {
            t1.push(x1);
            t2.push(x2);
            t3.push(x3);
            t4.push(x4);
            t5.push(x5);
        }

        (t1, t2, t3, t4, t5)
    }
}
