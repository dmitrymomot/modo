use proc_macro2::TokenStream;
use quote::quote;
use syn::{parse2, Ident, ItemFn, LitStr, Result, Token};

struct HandlerArgs {
    method: Ident,
    path: LitStr,
}

impl syn::parse::Parse for HandlerArgs {
    fn parse(input: syn::parse::ParseStream) -> Result<Self> {
        let method: Ident = input.parse()?;
        input.parse::<Token![,]>()?;
        let path: LitStr = input.parse()?;
        Ok(HandlerArgs { method, path })
    }
}

pub fn expand(attr: TokenStream, item: TokenStream) -> Result<TokenStream> {
    let args: HandlerArgs = parse2(attr)?;
    let func: ItemFn = parse2(item)?;

    let func_name = &func.sig.ident;
    let method_ident = &args.method;
    let path = &args.path;

    let method_str = method_ident.to_string().to_uppercase();
    let rskit_method = match method_str.as_str() {
        "GET" => quote! { rskit::router::Method::GET },
        "POST" => quote! { rskit::router::Method::POST },
        "PUT" => quote! { rskit::router::Method::PUT },
        "PATCH" => quote! { rskit::router::Method::PATCH },
        "DELETE" => quote! { rskit::router::Method::DELETE },
        "HEAD" => quote! { rskit::router::Method::HEAD },
        "OPTIONS" => quote! { rskit::router::Method::OPTIONS },
        _ => {
            return Err(syn::Error::new_spanned(
                method_ident,
                format!("unsupported HTTP method: {method_str}"),
            ))
        }
    };

    let axum_method = match method_str.as_str() {
        "GET" => quote! { rskit::axum::routing::get },
        "POST" => quote! { rskit::axum::routing::post },
        "PUT" => quote! { rskit::axum::routing::put },
        "PATCH" => quote! { rskit::axum::routing::patch },
        "DELETE" => quote! { rskit::axum::routing::delete },
        "HEAD" => quote! { rskit::axum::routing::head },
        "OPTIONS" => quote! { rskit::axum::routing::options },
        _ => unreachable!(),
    };

    Ok(quote! {
        #func

        rskit::inventory::submit! {
            rskit::router::RouteRegistration {
                method: #rskit_method,
                path: #path,
                handler: || #axum_method(#func_name),
                middleware: vec![],
                module: None,
            }
        }
    })
}
