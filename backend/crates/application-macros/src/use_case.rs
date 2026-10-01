use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use syn::punctuated::Punctuated;
use syn::{Attribute, FnArg, Ident, ItemFn, Pat, PatType};

/// Expands a `#[use_case]` method into a private `<name>_inner_injected` body
/// and a same-named wrapper without the injected parameters. A fn that injects
/// nothing passes through unchanged.
pub(crate) fn expand_use_case(function: &ItemFn) -> syn::Result<TokenStream2> {
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
    let lazy_unit = Ident::new("lazy_unit", Span::mixed_site());
    let mut inner_inputs = Punctuated::new();
    let mut outer_inputs = Punctuated::new();
    let mut forwarded = Vec::new();
    let mut markers = Markers::default();
    let mut has_query_param = false;

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

        has_query_param |= is_query_type(&param.ty);

        let mut plain = param.clone();
        plain.attrs.retain(|attr| !is_marker(attr));
        inner_inputs.push(FnArg::Typed(plain.clone()));

        match injection_of(param)? {
            Some((Injection::Ports, attr)) => {
                markers.record(Injection::Ports, attr)?;
                forwarded.push(quote! { crate::ports::WithPorts::ports(self) });
            }
            Some((Injection::Unit, attr)) => {
                markers.record(Injection::Unit, attr)?;
                forwarded.push(quote! { &mut *#unit });
            }
            Some((Injection::LazyUnit, attr)) => {
                markers.record(Injection::LazyUnit, attr)?;
                forwarded.push(quote! { &mut #lazy_unit });
            }
            None => {
                let forwarded_name = &name.ident;
                forwarded.push(quote! { #forwarded_name});
                outer_inputs.push(FnArg::Typed(without_mut(plain)));
            }
        }
    }

    markers.validate()?;
    if let (true, Some(unit)) = (has_query_param, markers.unit.or(markers.lazy_unit)) {
        let message = "a Query use case never writes, so it cannot inject a unit";
        return Err(syn::Error::new_spanned(unit, message));
    }

    if !markers.injects_anything() {
        return Ok(quote! { #(#attrs)* #vis #sig #block });
    }

    let mut inner_sig = sig.clone();
    inner_sig.ident = format_ident!("{}_inner_injected", sig.ident);
    inner_sig.inputs = inner_inputs;
    let inner_name = &inner_sig.ident;
    let mut outer_sig = sig.clone();
    outer_sig.inputs = outer_inputs;

    let call = quote! { self.#inner_name(#(#forwarded),*) };
    let wrapper_body = if markers.unit.is_some() {
        quote! {
            crate::lazy_unit::assert_no_unit_open();
            #[allow(clippy::disallowed_methods)]
            let mut #unit = crate::ports::WithPorts::ports(self).database.begin().await?;
            let outcome = crate::lazy_unit::within_unit(true, #call).await;
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
    } else if markers.lazy_unit.is_some() {
        quote! {
            let mut #lazy_unit =
                crate::LazyUnit::new(&*crate::ports::WithPorts::ports(self).database);
            let outcome = crate::lazy_unit::within_unit(false, #call).await;
            #lazy_unit.settle(outcome).await
        }
    } else {
        quote! { #call.await }
    };

    Ok(quote! {
        #[doc(hidden)]
        #inner_sig #block

        #(#attrs)*
        #vis #outer_sig { #wrapper_body }
    })
}

enum Injection {
    Ports,    // #[ports]
    Unit,     // #[unit]
    LazyUnit, // #[lazy_unit]
}

/// The first marker attribute of each kind seen on a fn's parameters.
#[derive(Default)]
struct Markers<'a> {
    ports: Option<&'a Attribute>,
    unit: Option<&'a Attribute>,
    lazy_unit: Option<&'a Attribute>,
}

impl<'a> Markers<'a> {
    fn record(&mut self, injection: Injection, attr: &'a Attribute) -> syn::Result<()> {
        let (slot, name) = match injection {
            Injection::Ports => (&mut self.ports, "ports"),
            Injection::Unit => (&mut self.unit, "unit"),
            Injection::LazyUnit => (&mut self.lazy_unit, "lazy_unit"),
        };
        if slot.is_some() {
            let message = format!("a `#[use_case]` takes at most one `#[{name}]` parameter");
            return Err(syn::Error::new_spanned(attr, message));
        }
        *slot = Some(attr);
        Ok(())
    }

    fn validate(&self) -> syn::Result<()> {
        if let (Some(unit), Some(_)) = (self.unit, self.ports) {
            let message = "a `#[unit]` use case cannot also inject `#[ports]`; use `#[lazy_unit]` \
                           so pool reads run before the unit opens";
            return Err(syn::Error::new_spanned(unit, message));
        }
        if let (Some(unit), Some(_)) = (self.unit, self.lazy_unit) {
            let message = "`#[unit]` and `#[lazy_unit]` cannot be combined; a use case opens \
                           its unit either before the body or on demand";
            return Err(syn::Error::new_spanned(unit, message));
        }
        Ok(())
    }

    fn injects_anything(&self) -> bool {
        self.ports.is_some() || self.unit.is_some() || self.lazy_unit.is_some()
    }
}

fn injection_of(param: &PatType) -> syn::Result<Option<(Injection, &Attribute)>> {
    let mut found = None;
    for attr in &param.attrs {
        let injection = if attr.path().is_ident("ports") {
            Injection::Ports
        } else if attr.path().is_ident("unit") {
            Injection::Unit
        } else if attr.path().is_ident("lazy_unit") {
            Injection::LazyUnit
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
    ["ports", "unit", "lazy_unit"]
        .iter()
        .any(|marker| attr.path().is_ident(marker))
}

fn without_mut(mut param: PatType) -> PatType {
    if let Pat::Ident(name) = &mut *param.pat {
        name.mutability = None;
    }
    param
}

/// Whether `ty` is a path whose last segment is `Query` or ends in `Query`.
fn is_query_type(ty: &syn::Type) -> bool {
    let syn::Type::Path(path) = ty else {
        return false;
    };
    path.path
        .segments
        .last()
        .is_some_and(|segment| segment.ident.to_string().ends_with("Query"))
}
