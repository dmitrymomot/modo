use proc_macro::TokenStream;

mod roles;

#[proc_macro_attribute]
pub fn allow_roles(attr: TokenStream, item: TokenStream) -> TokenStream {
    roles::expand_allow_roles(attr.into(), item.into())
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

#[proc_macro_attribute]
pub fn deny_roles(attr: TokenStream, item: TokenStream) -> TokenStream {
    roles::expand_deny_roles(attr.into(), item.into())
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}
