use proc_macro2::TokenStream;
use quote::quote;
use syn::{ItemFn, LitStr, Result, Token};

pub struct RoleList(pub Vec<LitStr>);

impl syn::parse::Parse for RoleList {
    fn parse(input: syn::parse::ParseStream) -> Result<Self> {
        let roles = input.parse_terminated(|input| input.parse::<LitStr>(), Token![,])?;
        Ok(RoleList(roles.into_iter().collect()))
    }
}

pub fn expand_allow_roles(attr: TokenStream, item: TokenStream) -> Result<TokenStream> {
    let roles: RoleList = syn::parse2(attr)?;
    let func: ItemFn = syn::parse2(item)?;
    let role_strs: Vec<&LitStr> = roles.0.iter().collect();

    Ok(quote! {
        #[middleware(modo_tenant::guard::require_roles(&[#(#role_strs),*]))]
        #func
    })
}

pub fn expand_deny_roles(attr: TokenStream, item: TokenStream) -> Result<TokenStream> {
    let roles: RoleList = syn::parse2(attr)?;
    let func: ItemFn = syn::parse2(item)?;
    let role_strs: Vec<&LitStr> = roles.0.iter().collect();

    Ok(quote! {
        #[middleware(modo_tenant::guard::exclude_roles(&[#(#role_strs),*]))]
        #func
    })
}
