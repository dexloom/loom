use proc_macro::TokenStream;

use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Field, Fields, PathArguments, Type};

#[proc_macro_derive(Accessor, attributes(accessor))]
pub fn derive_access(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let name = input.ident;
    let generics = input.generics;

    let data = match input.data {
        Data::Struct(s) => s,
        _ => panic!("This macro only works for structs"),
    };

    let fields = match data.fields {
        Fields::Named(named) => named,
        _ => panic!("no named fields"),
    };

    let mut access_fields: Vec<Field> = Vec::new();

    for field_ in fields.named {
        for attr in field_.attrs.iter() {
            if attr.path().is_ident("accessor") {
                access_fields.push(field_.clone());
            }
        }
    }
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let traits = access_fields
        .into_iter()
        .map(|field| {
            let field_name = field.ident.expect("Field needs name");
            let field_type = match field.ty.clone() {
                Type::Path(type_path) => match &type_path.path.segments[0].arguments.clone() {
                    PathArguments::AngleBracketed(params) => match &params.args[0] {
                        syn::GenericArgument::Type(ty) => match &ty {
                            Type::Path(type_path) => match &type_path.path.segments[0].arguments.clone() {
                                PathArguments::AngleBracketed(params) => match &params.args[0] {
                                    syn::GenericArgument::Type(ty) => ty.clone(),
                                    _ => panic!("Expected type parameter"),
                                },
                                _ => panic!("Expected Angle Brackets"),
                            },
                            _ => panic!("Expected DataType parameter"),
                        },
                        _ => panic!("Excpected type parameter"),
                    },
                    _ => panic!("Expected Angle Brackets"),
                },
                _ => panic!("Expected Option"),
            };

            quote! {
                impl #impl_generics Accessor<#field_type> for #name #ty_generics #where_clause {
                    fn access(&mut self, value : SharedState<#field_type>) -> &mut Self {
                        self.#field_name = Some(value);
                        self
                    }
                }
            }
        })
        .collect::<Vec<_>>();

    let expanded = quote! {
        #(#traits)*
    };
    expanded.into()
}

#[proc_macro_derive(Consumer, attributes(consumer))]
pub fn derive_consumer(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let name = input.ident;
    let generics = input.generics;

    let data = match input.data {
        Data::Struct(s) => s,
        _ => panic!("This macro only works for structs"),
    };

    let fields = match data.fields {
        Fields::Named(named) => named,
        _ => panic!("no named fields"),
    };

    let mut access_fields: Vec<Field> = Vec::new();

    for field_ in fields.named {
        for attr in field_.attrs.iter() {
            if attr.path().is_ident("consumer") {
                access_fields.push(field_.clone());
            }
        }
    }
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let traits = access_fields
        .into_iter()
        .map(|field| {
            let field_name = field.ident.expect("Field needs name");
            let field_type = match field.ty.clone() {
                Type::Path(type_path) => match &type_path.path.segments[0].arguments.clone() {
                    PathArguments::AngleBracketed(params) => match &params.args[0] {
                        syn::GenericArgument::Type(ty) => match &ty {
                            Type::Path(type_path) => match &type_path.path.segments[0].arguments.clone() {
                                PathArguments::AngleBracketed(params) => match &params.args[0] {
                                    syn::GenericArgument::Type(ty) => ty.clone(),
                                    _ => panic!("Excpected type parameter"),
                                },
                                _ => panic!("Expected Angle Brackets"),
                            },
                            _ => panic!("Expected DataType parameter"),
                        },
                        _ => panic!("Expected type parameter"),
                    },
                    _ => panic!("Expected Angle Brackets"),
                },
                _ => panic!("Expected Option"),
            };

            quote! {
                impl #impl_generics Consumer<#field_type> for #name #ty_generics #where_clause {
                    fn consume(&mut self, value : Broadcaster<#field_type>) -> &mut Self {
                        self.#field_name = Some(value);
                        self
                    }
                }
            }
        })
        .collect::<Vec<_>>();

    let expanded = quote! {
        #(#traits)*
    };
    expanded.into()
}

#[proc_macro_derive(Producer, attributes(producer))]
pub fn derive_producer(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let name = input.ident;
    let generics = input.generics;

    let data = match input.data {
        Data::Struct(s) => s,
        _ => panic!("This macro only works for structs"),
    };

    let fields = match data.fields {
        Fields::Named(named) => named,
        _ => panic!("no named fields"),
    };

    let mut access_fields: Vec<Field> = Vec::new();

    for field_ in fields.named {
        for attr in field_.attrs.iter() {
            if attr.path().is_ident("producer") {
                access_fields.push(field_.clone());
            }
        }
    }
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let traits = access_fields
        .into_iter()
        .map(|field| {
            let field_name = field.ident.expect("Field needs name");
            let field_type = match field.ty.clone() {
                Type::Path(type_path) => match &type_path.path.segments[0].arguments.clone() {
                    PathArguments::AngleBracketed(params) => match &params.args[0] {
                        syn::GenericArgument::Type(ty) => match &ty {
                            Type::Path(type_path) => match &type_path.path.segments[0].arguments.clone() {
                                PathArguments::AngleBracketed(params) => match &params.args[0] {
                                    syn::GenericArgument::Type(ty) => ty.clone(),
                                    _ => panic!("Expected type parameter"),
                                },
                                _ => panic!("Expected Angle Brackets"),
                            },
                            _ => panic!("Expected DataType parameter"),
                        },
                        _ => panic!("Excected type parameter"),
                    },
                    _ => panic!("Expected Angle Brackets"),
                },
                _ => panic!("Expected Option"),
            };

            quote! {
                impl #impl_generics Producer<#field_type> for #name #ty_generics #where_clause {
                    fn produce(&mut self, value : Broadcaster<#field_type>) -> &mut Self {
                        self.#field_name = Some(value);
                        self
                    }
                }
            }
        })
        .collect::<Vec<_>>();

    let expanded = quote! {
        #(#traits)*
    };

    expanded.into()
}
