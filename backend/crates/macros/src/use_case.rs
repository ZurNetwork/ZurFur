use proc_macro2::{Span, TokenStream as TokenStreamv2};
use quote::{format_ident, quote};
use syn::punctuated::Punctuated;
use syn::{Attribute, FnArg, Ident, ItemFn, Pat, PatType};

pub(crate) fn expand_transaction(function: &ItemFn) -> syn::Result<TokenStreamv2> {
    let ItemFn {
        attrs,
        block,
        vis,
        sig,
    } = function;

    if sig.asyncness.is_none() {
        return Err(syn::Error::new_spanned(
            sig.fn_token,
            "a `#[use_case]` fn must be `async`",
        ));
    };

    if sig.receiver().is_none() {
        return Err(syn::Error::new_spanned(
            &sig.ident,
            "`#[use_case]` needs access to `self` for injection",
        ));
    }

    let unit = Ident::new("unit", Span::mixed_site());
    let mut inner_inputs = Punctuated::new();
    let mut outer_inputs = Punctuated::new();
    let mut forwarded = Vec::new();
    let mut opens_unit = false;
    let mut injects_anything = false;

    for input in &sig.inputs {
        let FnArg::Typed(param) = input else {
            inner_inputs.push(input.clone());
            outer_inputs.push(input.clone());
            continue;
        };
        let Pat::Ident(name) = &*param.pat else {
            return Err(syn::Error::new_spanned(
                &param.pat,
                "a `#[use_case]` takes plainly named parameters",
            ));
        };

        let mut plain = param.clone();
        plain.attrs.retain(|attr| !is_marker(attr));
        inner_inputs.push(FnArg::Typed(plain.clone()));

        match injection_of(param)? {
            Some((Injection::Ports, _)) => {
                injects_anything = true;
                forwarded.push(quote! { crate::ports::WithPorts::ports(self) });
            }
            Some((Injection::Unit, _)) => {
                injects_anything = true;
                opens_unit = true;
                forwarded.push(quote! { &mut *#unit });
            }
            None => {
                let forwarded_name = &name.ident;
                forwarded.push(quote! { #forwarded_name});
                outer_inputs.push(FnArg::Typed(without_mut(plain)));
            }
        }
    }

    if !injects_anything {
        return Err(syn::Error::new_spanned(
            &sig.ident,
            "a `#[use_case]` must inject at least one dependency as `#[ports]` or `#[unit]`",
        ));
    }

    let mut inner_sig = sig.clone();
    inner_sig.ident = format_ident!("{}_inner_injected", sig.ident);
    inner_sig.inputs = inner_inputs;
    let inner_name = &inner_sig.ident;
    let mut outer_sig = sig.clone();
    outer_sig.inputs = outer_inputs;

    let call = quote! { self.#inner_name(#(#forwarded),*) };
    let wrapper_body = if opens_unit {
        quote! {
            #[allow(clippy::disallowed_methods)]
            let mut #unit = self.ports().database.begin().await?;
            let outcome = #call.await;
            match outcome {
                Ok(value) => {
                    #[allow(clippy::disallowed_methods)]
                    #unit.commit().await?;
                    Ok(value)
                }
                Err(error) => {
                    #[allow(clippy::disallowed_methods)]
                    let _ = #unit.rollback().await;
                    Err(error)
                }
            }
        }
    } else {
        quote! { #call.await }
    };

    Ok(quote! {
        #(#attrs)*
        #vis #inner_sig #block

        #(#attrs)*
        #vis #outer_sig { #wrapper_body }
    })
}

enum Injection {
    Ports, // #[ports]
    Unit,  // #[unit]
}

fn injection_of(param: &PatType) -> syn::Result<Option<(Injection, &Attribute)>> {
    let mut found = None;
    for attr in &param.attrs {
        let injection = if attr.path().is_ident("ports") {
            Injection::Ports
        } else if attr.path().is_ident("unit") {
            Injection::Unit
        } else {
            continue;
        };
        if found.is_some() {
            return Err(syn::Error::new_spanned(
                attr,
                "a parameter takes one injection marker",
            ));
        }
        found = Some((injection, attr))
    }
    Ok(found)
}

fn is_marker(attr: &Attribute) -> bool {
    attr.path().is_ident("ports") || attr.path().is_ident("unit")
}

fn without_mut(mut param: PatType) -> PatType {
    if let Pat::Ident(name) = &mut *param.pat {
        name.mutability = None;
    }
    param
}
