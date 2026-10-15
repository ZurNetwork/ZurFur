//! Proc macros for the `application` crate: `#[use_case]` injects ports and a
//! unit of work into a use-case method, and `#[derive(WithPorts)]` wires a
//! namespace to the ports bag. They expand only inside `application`, and a
//! `#[unit]` parameter beside `#[ports]` is rejected.

use proc_macro::TokenStream;
use syn::{ItemFn, parse_macro_input};

use crate::use_case::expand_use_case;
use crate::with_ports::expand_with_ports;

mod use_case;
mod with_ports;

/// Turns an async `&self` method returning a `Result` into a use case: `#[ports]`, `#[unit]`
/// and `#[lazy_unit]` parameters are injected and leave the public signature. A unit commits
/// on `Ok` and rolls back on `Err`. Every use case runs in a `debug` span `use_case{name}`
/// that logs its start and its outcome.
#[proc_macro_attribute]
pub fn use_case(args: TokenStream, item: TokenStream) -> TokenStream {
    let _args = parse_macro_input!(args as syn::parse::Nothing);
    let function = parse_macro_input!(item as ItemFn);
    expand_use_case(&function)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Derives `WithPorts` from the single `#[ports]` field. A child namespace
/// delegates to its parent and the parent gains a snake_case accessor building
/// it; `#[ports(is_root = true)]` marks the struct that holds the bag itself.
#[proc_macro_derive(WithPorts, attributes(ports))]
pub fn with_ports(item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as syn::DeriveInput);
    expand_with_ports(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
