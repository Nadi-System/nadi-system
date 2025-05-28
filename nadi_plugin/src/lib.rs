//! Procedural Macros for plugin development for NADI system.
//!
//! Do not use this library by itself, it should be reexported from
//! [`nadi_core`] crate
use convert_case::{Case, Casing};
use std::collections::HashMap;

use proc_macro::TokenStream;
use quote::{format_ident, quote, quote_spanned, ToTokens};
use syn::{
    parse_macro_input, punctuated::Punctuated, token::Comma, Attribute, DeriveInput, Expr, FnArg,
    Ident, ItemFn, ItemMod, Lit, MetaNameValue, Type, TypeReference,
};

#[derive(PartialEq)]
enum FuncType {
    Env,
    Node,
    Network,
}

impl std::fmt::Display for FuncType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                FuncType::Env => "Env",
                FuncType::Node => "Node",
                FuncType::Network => "Network",
            }
        )
    }
}

/// Idents and Path for from_attr and try_from_attr
fn nadi_trait_idents(relaxed: bool) -> (proc_macro2::TokenStream, Ident, Ident) {
    let (tr, func1, func2) = if relaxed {
        (
            format_ident!("FromAttributeRelaxed"),
            format_ident!("from_attr_relaxed"),
            format_ident!("try_from_attr_relaxed"),
        )
    } else {
        (
            format_ident!("FromAttribute"),
            format_ident!("from_attr"),
            format_ident!("try_from_attr"),
        )
    };
    (quote! {::nadi_core::attrs::#tr}, func1, func2)
}

fn nadi_struct_name(name: &Ident, suff: &str) -> Ident {
    format_ident!("{}{suff}", name.to_string().to_case(Case::UpperCamel))
}

fn nadi_func_impl(ft: &FuncType) -> proc_macro2::TokenStream {
    match ft {
        FuncType::Env => quote! {::nadi_core::functions::EnvFunction},
        FuncType::Node => quote! {::nadi_core::functions::NodeFunction},
        FuncType::Network => quote! {::nadi_core::functions::NetworkFunction},
    }
}

#[proc_macro_derive(FromAttribute)]
pub fn from_attribute_derive(input: TokenStream) -> TokenStream {
    from_attr_derive(input, false)
}

#[proc_macro_derive(FromAttributeRelaxed)]
pub fn from_attribute_relaxed_derive(input: TokenStream) -> TokenStream {
    from_attr_derive(input, true)
}

/// this should support automatically deriving the FromAttribute trait for the following complex types
/// - Struct(ty) => wrapper values like this as long as ty has FromAttribute
/// - Struct {x:.., y:..}  => struct values will be made from Table() as long as each field type has FromAttribute
/// - Enum{...} => enum values will be made for the first type that is matched from Attribute types as long as each one has FromAttribute
fn from_attr_derive(input: TokenStream, relaxed: bool) -> TokenStream {
    let (trt, func, try_func) = nadi_trait_idents(relaxed);
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;
    let name_s = name.to_string();
    let data = input.data.clone();
    let conversion = match data {
        syn::Data::Struct(st) => {
            match st.fields {
                syn::Fields::Named(flds) => {
                    // try to parse a Table value into the fields of this struct
                    let conv: Vec<_> = flds.named.iter().map(|f| {
			let i = &f.ident;
			let is = i.as_ref().map(|l| l.to_string()).expect("should be named");
			quote!{
			    let val = match attrmap.get(#is) {
				Some(v) => v,
				None => return Err(format!("FieldError: Field {} not found in the value for {}", #is, #name_s)),
			    };
			    let #i = #trt :: #try_func (val)?;
			}
		    }).collect();
                    let names = flds.named.iter().map(|f| &f.ident);
                    quote! {
                    let attrmap: ::nadi_core::attrs::AttrMap = ::nadi_core::attrs::FromAttribute::try_from_attr (value)?;
                    #(#conv)*
                    Ok(Self {
                        #(#names),*
                    })
                    }
                }
                syn::Fields::Unnamed(_) => {
                    quote! {
                    Ok(Self(#trt :: #func (value)?))
                    }
                }
                _ => panic!("Not supported"),
            }
        }
        syn::Data::Enum(en) => {
            // try each variant and return the first one that succeeds
            let vars: Vec<_> = en
                .variants
                .iter()
                .map(|v| {
                    let i = &v.ident;
                    quote! {
                        if let Some(val) = #trt :: #func (value) {
                        return Ok(Self::#i(val));
                        }
                    }
                })
                .collect();
            let names: Vec<String> = en
                .variants
                .iter()
                .map(|v| v.fields.to_token_stream().to_string())
                .collect();
            // TODO extra () for unnamed fields should be renamed later
            let names = names.join(", ");
            quote! {
            #(#vars)*
            Err(format!("Incorrect Type: got {} instead of any of: [{}]", value.type_name(), #names))
            }
        }
        syn::Data::Union(_) => {
            panic!("Union derive not supported!")
        }
    };

    let expanded = quote! {
        impl #trt for #name {
            fn #func (value: &Attribute) -> Option<Self>{
        #trt :: #try_func(value).ok()
            }
            fn #try_func (value: &Attribute) -> Result<Self, String> {
        #conversion
            }
    }
    };
    // println!("{}", expanded);
    TokenStream::from(expanded)
}

/// register this function as a node function on nadi plugin
#[proc_macro_attribute]
pub fn env_func(args: TokenStream, item: TokenStream) -> TokenStream {
    let item = parse_macro_input!(item as ItemFn);
    nadi_func_inner(args, item, FuncType::Env)
}

/// register this function as a node function on nadi plugin
#[proc_macro_attribute]
pub fn node_func(args: TokenStream, item: TokenStream) -> TokenStream {
    let item = parse_macro_input!(item as ItemFn);
    nadi_func_inner(args, item, FuncType::Node)
}

/// register this function as a network function on nadi plugin
#[proc_macro_attribute]
pub fn network_func(args: TokenStream, item: TokenStream) -> TokenStream {
    let item = parse_macro_input!(item as ItemFn);
    nadi_func_inner(args, item, FuncType::Network)
}

#[derive(Clone, Copy, Default, PartialEq)]
enum FuncArgType {
    #[default]
    Arg,
    Relaxed,
    Args,
    KwArgs,
}

const FUNC_ARG_ATTRS: [(&str, FuncArgType); 3] = [
    ("args", FuncArgType::Args),
    ("kwargs", FuncArgType::KwArgs),
    ("relaxed", FuncArgType::Relaxed),
];

fn nadi_func_inner(args: TokenStream, item: ItemFn, ft: FuncType) -> TokenStream {
    let args = parse_macro_input!(args with Punctuated<MetaNameValue, Comma>::parse_terminated);
    let default_args: HashMap<Ident, Expr> = args
        .into_iter()
        .map(|p| (p.path.segments.first().unwrap().ident.clone(), p.value))
        .collect();
    let arg0 = if FuncType::Env == ft {
        None
    } else {
        item.sig.inputs.first()
    };
    let func_args: Vec<(&Ident, &Type, FuncArgType)> = item
        .sig
        .inputs
        .iter()
        // skip first argument, node or network
        .skip((FuncType::Env != ft) as usize)
        .map(get_fn_arg)
        .collect();
    let warnings = check_args_kwargs_order(&func_args, &default_args);

    let func_struct_name = nadi_struct_name(&item.sig.ident, &ft.to_string());

    let name_func = get_name_func(&item);
    let code_func = get_code_func(&item);
    let (call_func, default_exprs) = get_call_func(
        &item,
        arg0,
        &ft,
        &func_args,
        &default_args,
        &func_struct_name,
    );
    let help_func = get_help_func(&item);
    let signature_func = get_signature_func(&item, &ft, &default_args, default_exprs);
    let impl_trait = nadi_func_impl(&ft);

    let clean_func = clean_function(&item);

    quote! {
    #warnings

        #[derive(Debug, Clone)]
        pub struct #func_struct_name;

        impl #impl_trait for #func_struct_name {
            #name_func
            #code_func
            #help_func
            #signature_func
            #call_func
        }

        impl  #func_struct_name {
            #clean_func
        }
    }
    .into()
}

fn check_args_kwargs_order(
    args: &[(&Ident, &Type, FuncArgType)],
    default_args: &HashMap<Ident, Expr>,
) -> proc_macro2::TokenStream {
    let mut warnings: Vec<proc_macro2::TokenStream> = default_args
        .keys()
        .filter_map(|id| {
            if !args.iter().any(|a| a.0 == id) {
                Some(quote_spanned! {
                id.span() =>
                    compile_error!("Argument not in the inner function");
                })
            } else {
                None
            }
        })
        .collect();
    let mut flag = false;
    for (a, t, _) in args {
        if type_is_opt(t) || default_args.contains_key(a) {
            flag = true;
        } else if flag {
            warnings.push(quote_spanned! {
                a.span()=> compile_error!("Positional argument after default argument(s)");
            });
        }
    }

    quote! { #(#warnings)* }
}

/// Clean the function of function argument attributes like #[args], ...
fn clean_function(func: &ItemFn) -> ItemFn {
    let mut func = func.clone();
    for arg in &mut func.sig.inputs {
        match arg {
            syn::FnArg::Typed(ref mut a) => {
                let attrs = std::mem::take(&mut a.attrs);
                let (_, remain): (Vec<_>, Vec<_>) = attrs.into_iter().partition(|at| {
                    // remove all attrs for function arg, and the doc attribute
                    FUNC_ARG_ATTRS
                        .iter()
                        .map(|a| a.0)
                        .chain(["doc"])
                        .any(|a| at.path().is_ident(a))
                });

                a.attrs = remain;
            }
            _ => panic!("unsuported args"),
        }
    }
    func
}

fn get_fn_arg(arg: &FnArg) -> (&Ident, &Type, FuncArgType) {
    match arg {
        syn::FnArg::Typed(arg) => {
            let t: FuncArgType = FUNC_ARG_ATTRS
                .iter()
                .filter(|a| arg.attrs.iter().any(|at| at.path().is_ident(a.0)))
                .map(|a| a.1)
                .next()
                .unwrap_or_default();
            match arg.pat.as_ref() {
                syn::Pat::Ident(i) => (&i.ident, arg.ty.as_ref(), t),
                _ => panic!("Invalid Argument Type for Nadi function"),
            }
        }
        _ => panic!("Invalid Argument Type for Nadi function"),
    }
}

// HACK ignoring the path and assuming anything::Option is Option
fn type_is_opt(ty: &Type) -> bool {
    // if let Type::Path(p) = ty {
    //     p.path.is_ident("Option")
    // } else {
    //     false
    // }
    ty.to_token_stream()
        .to_string()
        .split('<')
        .next()
        .unwrap_or_default()
        .split("::")
        .last()
        .unwrap_or_default()
        .trim()
        == "Option"
}

fn get_opt_type(ty: &Type) -> Option<&Type> {
    if let Type::Path(p) = ty {
        // if p.path.is_ident("Option") {
        let op = p.path.segments.last().expect("should have path component");
        if let syn::PathArguments::AngleBracketed(ag) = &op.arguments {
            for a in &ag.args {
                if let syn::GenericArgument::Type(t) = a {
                    return Some(t);
                }
            }
        }
        // }
    }
    None
}

/// Register the plugin for NADI system. This should be on the top
/// level of the `mod`, with access to all the functions so it can
/// register them
///
/// # Example Use:
/// ```rust,ignore
/// // This requies you to have nadi_core in your direct dependency
/// #[nadi_plugin::nadi_plugin]
/// mod plugin_name {
///     #[nadi_plugin::node_func]
///     fn do_something(node: &mut NodeInner) {
///     // do something here
///     }
/// }
/// ```
///
/// Only one instance of `mod` should be exported as plugin for
/// external plugins compiled to cdynlib.
#[proc_macro_attribute]
pub fn nadi_plugin(args: TokenStream, item: TokenStream) -> TokenStream {
    nadi_export_plugin(args, item, true)
}

/// Register the `mod` as an internal plugin, only to be used on
/// plugins compiled in `nadi_core` crate. This should also be in the
/// top of the `mod` definition so it can see all the functions.
#[proc_macro_attribute]
pub fn nadi_internal_plugin(args: TokenStream, item: TokenStream) -> TokenStream {
    nadi_export_plugin(args, item, false)
}

fn nadi_export_plugin(_args: TokenStream, item: TokenStream, external: bool) -> TokenStream {
    let item = parse_macro_input!(item as ItemMod);

    let name = &item.ident;
    let name_s = name.to_string();
    let name_mod = nadi_struct_name(name, "Mod");
    let env_funcs = get_nadi_functions(&item, "env_func");
    let node_funcs = get_nadi_functions(&item, "node_func");
    let network_funcs = get_nadi_functions(&item, "network_func");
    let regis_env_funcs = env_funcs
        .iter()
        .map(|n| nadi_struct_name(n, "Env"))
        .map(|n| {
            quote! {
                    nf.register_env_function(
                #name_s,
                        ::nadi_core::functions::EnvFunction_TO::from_value(
                #n,
                ::nadi_core::abi_stable::sabi_trait::TD_CanDowncast
                        )
            );
                }
        });
    let regis_node_funcs = node_funcs
        .iter()
        .map(|n| nadi_struct_name(n, "Node"))
        .map(|n| {
            quote! {
                    nf.register_node_function(
                #name_s,
                        ::nadi_core::functions::NodeFunction_TO::from_value(
                #n,
                ::nadi_core::abi_stable::sabi_trait::TD_CanDowncast
                        )
            );
                }
        });
    let regis_network_funcs = network_funcs
        .iter()
        .map(|n| nadi_struct_name(n, "Network"))
        .map(|n| {
            quote! {
                nf.register_network_function(
            #name_s,
                    ::nadi_core::functions::NetworkFunction_TO::from_value(
                        #n,
                        ::nadi_core::abi_stable::sabi_trait::TD_CanDowncast
                    )
                );
            }
        });

    if external {
        quote! {
            #[::nadi_core::abi_stable::export_root_module]
            pub fn get_library() -> ::nadi_core::plugins::NadiExternalPlugin_Ref {
        ::nadi_core::abi_stable::prefix_type::PrefixTypeTrait::leak_into_prefix(
            ::nadi_core::plugins::NadiExternalPlugin {
            register_functions,
            plugin_name,
            })
            }

            #[::nadi_core::abi_stable::sabi_extern_fn]
            fn plugin_name() -> ::nadi_core::abi_stable::std_types::RString {
                #name_s .into()
            }

            #[::nadi_core::abi_stable::sabi_extern_fn]
            fn register_functions(nf: &mut ::nadi_core::functions::NadiFunctions) {

                #(#regis_env_funcs)*

                #(#regis_node_funcs)*

                #(#regis_network_funcs)*
            }

            use #name::*;

            #item
        }
    } else {
        quote! {
            pub struct #name_mod;

            impl ::nadi_core::plugins::NadiPlugin for #name_mod {
        fn name(&self) -> ::nadi_core::abi_stable::std_types::RString {
                    #name_s .into()
        }
        fn register(&self, nf: &mut ::nadi_core::functions::NadiFunctions) {

            #(#regis_env_funcs)*

            #(#regis_node_funcs)*

            #(#regis_network_funcs)*
        }
            }

            use #name::*;

            #item
        }
    }
    .into()
}

fn get_nadi_functions<'a>(item: &'a ItemMod, funct: &'_ str) -> Vec<&'a Ident> {
    if let Some((_, cont)) = &item.content {
        cont.iter()
            .filter_map(|c| {
                if let syn::Item::Fn(f) = c {
                    Some(f)
                } else {
                    None
                }
            })
            .filter_map(|f| {
                if f.attrs.iter().any(|a| match &a.meta {
                    syn::Meta::Path(p) => p.is_ident(funct),
                    syn::Meta::List(l) => l.path.is_ident(funct),
                    _ => false,
                }) {
                    Some(&f.sig.ident)
                } else {
                    None
                }
            })
            .collect()
    } else {
        vec![]
    }
}

fn get_name_func(item: &ItemFn) -> proc_macro2::TokenStream {
    let func_name = item.sig.ident.to_string();

    quote! {
        fn name(&self) -> ::nadi_core::abi_stable::std_types::RString {
            #func_name .into()
        }
    }
}

fn generic_construct(ty: &Type) -> proc_macro2::TokenStream {
    // if there are any generic parameters, then we need to have ::
    // there to call its methods like `Vec::<String>::new()`
    std::str::FromStr::from_str(&ty.to_token_stream().to_string().replace("<", "::<")).unwrap()
}

// returns the inner type, and if deref is required or not
fn ref_type_inner(ty: &TypeReference, construct: bool) -> (proc_macro2::TokenStream, bool) {
    // to provide slice we need to generate vec
    let inner_ty = match ty.elem.as_ref() {
        Type::Slice(i) => {
            let i = &i.elem;
            if construct {
                quote!(Vec::<#i>)
            } else {
                quote!(Vec<#i>)
            }
        }
        i => {
            if construct {
                generic_construct(i)
            } else {
                quote! {#i}
            }
        }
    };
    // similarly, str needs String, and Path needs PathBuf
    match inner_ty
        .to_token_stream()
        .to_string()
        .replace(" ", "")
        .as_str()
    {
        "str" => (quote!(String), true),
        "Path" | "std::path::Path" => (quote!(::std::path::PathBuf), false),
        _ => (inner_ty, false),
    }
}

fn get_code_func(item: &ItemFn) -> proc_macro2::TokenStream {
    let func_code = prettyplease::unparse(
        &syn::parse2(item.to_token_stream()).expect("code should be valid for prettyplease"),
    );

    quote! {
    fn code(&self) -> ::nadi_core::abi_stable::std_types::RString {
    #func_code .into()
    }}
}

fn get_call_func(
    item: &ItemFn,
    arg0: Option<&FnArg>,
    ft: &FuncType,
    args: &[(&Ident, &Type, FuncArgType)],
    defaults: &HashMap<Ident, Expr>,
    func_struct_name: &Ident,
) -> (proc_macro2::TokenStream, proc_macro2::TokenStream) {
    let mut defaults_expr: Vec<proc_macro2::TokenStream> = Vec::new();
    let ret_err = quote! { ::nadi_core::functions::FunctionRet::Error };
    let mut argc: usize = 0;
    let mut kwonly = false;
    let extract_args: Vec<proc_macro2::TokenStream> = args
        .iter()
        .map(|(arg, ty, at)| {
            argc += 1;
            let arg_name = arg.to_string();
            let ty_name = ty.to_token_stream().to_string();
            let arg_func_call = match at {
                FuncArgType::Arg if kwonly => quote! { ctx.just_kwarg (#arg_name) },
                FuncArgType::Arg => quote! { ctx.arg_kwarg (#argc - 1, #arg_name)},
                FuncArgType::Relaxed if kwonly => quote! { ctx.just_kwarg_relaxed (#arg_name)},
                FuncArgType::Relaxed => quote! { ctx.arg_kwarg_relaxed (#argc - 1, #arg_name)},
                FuncArgType::Args => {
                    kwonly = true;
                    return quote! {
                        let #arg: #ty = ctx.args().into();
                    };
                }
                FuncArgType::KwArgs => {
                    kwonly = true;
                    return quote! {
                        let #arg: #ty = ctx.kwargs().into();
                    };
                }
            };
            let def = if let Some(val) = defaults.get(arg) {
                match ty {
                    Type::Reference(r) => {
                        let inner_ty = ref_type_inner(r, true).0;
                        let warn = r.mutability.map(|m| {
                            // mut reference on the network functions
                            // are useless as they are one time
                            // execution; in node function they might
                            // have unexpected behaviour as the node
                            // functions are supposed to be able to
                            // run in parallel
                            quote_spanned! {
                                m.span=> compile_error!(
                                "Mutable Reference not supported for nadi function args"
                                );
                            }
                        });
                        defaults_expr.push(quote! {
                            let #arg = #inner_ty :: from ( #val );
                        });
                        quote! {
                        #warn
                        #inner_ty :: from ( #val )
                        }
                    }
                    _ => {
                        let ty = generic_construct(ty);
                        defaults_expr.push(quote! {
                            let #arg = #ty :: from ( #val );
                        });
                        quote! {
                        #ty :: from ( #val )
                        }
                    }
                }
            } else {
                quote! {
                        return #ret_err (
                format!("Argument {} ({} [{}]) is required", #argc, #arg_name, #ty_name).into()
                        );
                    }
            };
            let isopt = type_is_opt(ty);
            let patterns = if isopt {
                quote! {
                    Some(Ok(v)) => Some(v),
                    Some(Err(e)) => return #ret_err (e.into()),
                    None => None,
                }
            } else {
                quote! {
                    Some(Ok(v)) => v,
                    Some(Err(e)) => return #ret_err (e.into()),
                    None => {#def},
                }
            };
            match ty {
                Type::Reference(r) => {
                    let arg_o = format_ident!("{}_o", arg);
                    let inner_ty = ref_type_inner(r, false).0;
                    let m = r.mutability;
                    quote! {
                    let #m #arg_o : #inner_ty = match #arg_func_call {
                        #patterns
                    };
                    let #arg : #ty = & #arg_o;
                    }
                }
                _ => {
                    if let Some(Type::Reference(r)) = get_opt_type(ty) {
                        let arg_o = format_ident!("{}_o", arg);
                        let (inner_ty, deref) = ref_type_inner(r, false);
                        let m = r.mutability;
                        // all this since deref doesn't happen automatically inside option like with & #arg_o above
                        let asref = match (deref, m.is_some()) {
                            (false, true) => quote!(std::option::Option::as_mut),
                            (false, false) => quote!(std::option::Option::as_ref),
                            (true, true) => quote!(std::option::Option::as_deref_mut),
                            (true, false) => quote!(std::option::Option::as_deref),
                        };
                        quote! {
                            let #m #arg_o : Option<#inner_ty> = match #arg_func_call {
                            #patterns
                            };
                            let #arg : #ty = #asref (& #m #arg_o);
                        }
                    } else {
                        quote! {
                            let #arg : #ty = match #arg_func_call {
                            #patterns
                            };
                        }
                    }
                }
            }
        })
        .collect();
    let args_n: Vec<proc_macro2::TokenStream> = args
        .iter()
        .map(|(arg, _, _)| arg.into_token_stream())
        .collect();
    let func_name = &item.sig.ident;
    let (fname, arg0, fcall) = match ft {
        FuncType::Env => (quote! {call}, quote! {}, quote! {#func_name(#(#args_n),*)}),
        _ => {
            let a0 = get_fn_arg(arg0.expect("Should have at least one argument"));
            let arg0_name = a0.0;
            let arg0_ty = a0.1;
            let fname = match &arg0_ty {
                syn::Type::Reference(r) => {
                    if r.mutability.is_some() {
                        quote! {call_mut}
                    } else {
                        quote! {call}
                    }
                }
                _ => panic!("First argument should be a reference"),
            };
            (
                fname,
                quote! {#arg0_name : #arg0_ty,},
                quote! {#func_name(#arg0_name, #(#args_n),*)},
            )
        }
    };
    let call_func = quote! {
            fn #fname(&self,
                    #arg0
                    ctx: &::nadi_core::functions::FunctionCtx)
                    -> ::nadi_core::functions::FunctionRet {

                #(#extract_args)*
        ::nadi_core::functions::FunctionRet::from(
                    #func_struct_name :: #fcall
                )
            }
    };
    let default_exprs = quote! {
    #(#defaults_expr)*
    };
    (call_func, default_exprs)
}

fn get_help_func(item: &ItemFn) -> proc_macro2::TokenStream {
    let docs = get_doc(&item.attrs);
    quote! {
        fn help(&self) -> ::nadi_core::abi_stable::std_types::RString {
    #docs .into()
        }
    }
}

fn get_signature_func(
    item: &ItemFn,
    ft: &FuncType,
    default_args: &HashMap<Ident, Expr>,
    default_exprs: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let args: Vec<proc_macro2::TokenStream> = item
        .sig
        .inputs
        .iter()
        .skip((FuncType::Env != *ft) as usize)
        .map(|a| {
            match a {
                syn::FnArg::Typed(a) => {
                    match a.pat.as_ref() {
                        syn::Pat::Ident(i) => {
                            let doc = get_doc(&a.attrs);
                            let (n, t) = (
                                i.ident.to_string(),
                                a.ty.as_ref().into_token_stream().to_string(),
                            );
                            // args and kwargs function signature
                            let ft = if a.attrs.iter().any(|at| at.path().is_ident("args")) {
                                quote! { Args }
                            } else if a.attrs.iter().any(|at| at.path().is_ident("kwargs")) {
                                quote! { KwArgs }
                            } else if default_args.contains_key(&i.ident) {
                                let v = format!("{{{}:?}}", i.ident);
                                quote! { DefArg(format!(#v) .into()) }
                            } else if type_is_opt(&a.ty) {
                                quote! { OptArg }
                            } else {
                                quote! { Arg }
                            };

                            quote! {
                            ::nadi_core::functions::FuncArg {
                                            name: #n .into(),
                                            ty: #t .into(),
                                            help: #doc .into(),
                                category: ::nadi_core::functions::FuncArgType:: #ft
                            }
                            }
                        }
                        _ => panic!("Not supported"),
                    }
                }
                _ => panic!("Not supported"),
            }
        })
        .collect();

    // function signature showing the function name, arguments and
    // their default values
    quote! {
        fn args(&self) -> ::nadi_core::abi_stable::std_types::RVec<
        ::nadi_core::functions::FuncArg
        > {
            #default_exprs
        vec![
        #(#args),*
        ] .into()
        }
    }
}

/// collect doc attributes
fn get_doc(attrs: &[Attribute]) -> String {
    let docs: Vec<String> = attrs
        .iter()
        .filter(|a| a.path().is_ident("doc"))
        .filter_map(|a| match &a.meta {
            syn::Meta::NameValue(val) => match &val.value {
                Expr::Lit(lit) => match &lit.lit {
                    Lit::Str(c) => {
                        let c = c.value();
                        Some(c.trim_matches(' ').to_string())
                    }
                    _ => None,
                },
                _ => None,
            },
            _ => None,
        })
        .collect();
    format_docstrings(docs.join("\n"))
}

fn format_docstrings(string: String) -> String {
    match string.lines().count() {
        0 => {
            // panic!("Please add at least one line of documentation");
            String::new()
        }
        1 => string.trim().to_string(),
        _ => {
            let num_leading = string
                .lines()
                .skip(1)
                .filter(|s| !s.is_empty())
                .filter_map(|l| l.chars().position(|c| !c.is_whitespace()))
                .min()
                .unwrap_or(0);
            let lines = string
                .lines()
                .skip(1)
                .map(|line| {
                    if line.len() > num_leading {
                        &line[num_leading..]
                    } else {
                        line
                    }
                })
                .map(|l| l.trim_end())
                .collect::<Vec<_>>()
                .join("\n");
            format!(
                "{}\n{}",
                string
                    .lines()
                    .next()
                    .expect("There should be at least one line of documentation"),
                lines
            )
        }
    }
}
