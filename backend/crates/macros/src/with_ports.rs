use syn::{Data, DeriveInput, Fields};

pub(crate) fn expand_with_ports(input: &syn::DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let Data::Struct(data) = &input.data else {
        return Err(unsupported(input));
    };
    let Fields::Named(fields) = &data.fields else {
        return Err(unsupported(input));
    };

    let implementer = &input.ident;
    let name = quote::format_ident!("{}", implementer.to_string().to_ascii_lowercase());

    let mut field_names = Vec::new();
    let mut parent_field: Option<syn::Field> = None;
    let mut is_root = false;
    for field in &fields.named {
        for attr in &field.attrs {
            if !attr.path().is_ident("ports") {
                continue;
            }

            // Bare `#[ports]` marks a parent; `#[ports(is_root = true)]` marks
            // the bag itself.
            if !matches!(attr.meta, syn::Meta::Path(_)) {
                attr.parse_nested_meta(|meta| {
                    if !meta.path.is_ident("is_root") {
                        return Err(meta.error("expected `is_root = true`"));
                    }
                    let value: syn::LitBool = meta.value()?.parse()?;
                    is_root = value.value;
                    Ok(())
                })?;
            }

            if let Some(previous_parent) = &parent_field {
                return Err(syn::Error::new_spanned(
                    &field.ident,
                    format!(
                        "`#[ports]` may only be used once in `WithPorts`. Previous parent: {}",
                        previous_parent.ident.as_ref().unwrap()
                    ),
                ));
            }
            parent_field = Some(field.clone());
            field_names.push(field.ident.as_ref().unwrap().to_string());
        }
    }
    if parent_field.is_none() {
        return Err(syn::Error::new_spanned(
            &input.ident,
            "`#[ports]` must be used once in `WithPorts`",
        ));
    }

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let parent_field = parent_field.unwrap();
    let parent_ident = &parent_field.ident;
    let ty = &parent_field.ty;
    let syn::Type::Reference(parent_ref) = ty else {
        return Err(syn::Error::new_spanned(
            ty,
            "the `#[ports]` field must be a reference to the parent",
        ));
    };
    let parent_ty = &parent_ref.elem;
    let lifetime = &parent_ref.lifetime;
    // A root holds the bag and is built by hand; a child reaches the bag
    // through its parent, which also gains the accessor that builds the child.
    let ports_body = if is_root {
        quote::quote! { self.#parent_ident }
    } else {
        quote::quote! { crate::ports::WithPorts::ports(self.#parent_ident) }
    };
    let accessor = (!is_root).then(|| {
        quote::quote! {
            impl #impl_generics #parent_ty {
                pub fn #name(&#lifetime self) -> #implementer #ty_generics {
                    #implementer { #parent_ident: self }
                }
            }
        }
    });

    Ok(quote::quote! {
        impl #impl_generics crate::ports::WithPorts<#lifetime> for #implementer #ty_generics #where_clause {
            fn ports(&self) -> &#lifetime crate::Ports {
                #ports_body
            }
        }

        #accessor
    })
}

fn unsupported(input: &DeriveInput) -> syn::Error {
    syn::Error::new_spanned(input, "Only structs are supported")
}
