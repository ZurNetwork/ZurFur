use proc_macro::TokenStream;
use syn::{ItemFn, parse_macro_input};

use crate::use_case::expand_transaction;
use crate::with_ports::expand_with_ports;

mod use_case;
mod with_ports;

#[proc_macro_attribute]
pub fn use_case(args: TokenStream, item: TokenStream) -> TokenStream {
    let _args = parse_macro_input!(args as syn::parse::Nothing);
    let function = parse_macro_input!(item as ItemFn);
    expand_transaction(&function)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[proc_macro_derive(WithPorts, attributes(ports))]
pub fn with_ports(item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as syn::DeriveInput);
    expand_with_ports(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
