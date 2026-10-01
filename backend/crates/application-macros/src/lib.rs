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

/// Turns an async `&self` method into a use case: parameters marked `#[ports]`,
/// `#[unit]` or `#[lazy_unit]` are injected and removed from the public
/// signature. `#[unit]` opens the unit before the body, `#[lazy_unit]` leaves
/// it to the body's `open()`; either commits on `Ok` and rolls back on `Err`.
/// A method that injects nothing is left as written.
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
