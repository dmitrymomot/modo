use proc_macro2::TokenStream;
use syn::Result;

pub fn expand(_attr: TokenStream, item: TokenStream) -> Result<TokenStream> {
    Ok(item)
}
