use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemFn};

#[proc_macro_attribute]
pub fn component(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);

    let vis = &input.vis;
    let sig = &input.sig;
    let block = &input.block;
    let attrs = &input.attrs;

    // Draft behavior: keep the function unchanged while reserving a stable
    // attribute that Axonyx can grow into later for component metadata.
    TokenStream::from(quote! {
        #(#attrs)*
        #vis #sig #block
    })
}

