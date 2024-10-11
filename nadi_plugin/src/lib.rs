//! Procedural Macros for plugin development for NADI system.
//!
//! Do not use this library by itself, it should be reexported from
//! [`nadi_core`] crate
use convert_case::{Case, Casing};
use std::collections::HashMap;

use proc_macro::TokenStream;
use quote::{format_ident, quote, quote_spanned, ToTokens};
use syn::{
    parse_macro_input, punctuated::Punctuated, token::Comma, Attribute, Expr, FnArg, Ident, ItemFn,
    ItemMod, Lit, MetaNameValue, Type,
};

fn nadi_struct_name(name: &Ident, suff: &str) -> Ident {
    format_ident!("{}{suff}", name.to_string().to_case(Case::UpperCamel))
}

fn nadi_func_impl(node: bool) -> proc_macro2::TokenStream {
    if node {
        quote! {::nadi_core::functions::NodeFunction}
    } else {
        quote! {::nadi_core::functions::NetworkFunction}
    }
}

/// register this function as a node function on nadi plugin
#[proc_macro_attribute]
pub fn node_func(args: TokenStream, item: TokenStream) -> TokenStream {
    nadi_func(args, item, true)
}

/// register this function as a network function on nadi plugin
#[proc_macro_attribute]
pub fn network_func(args: TokenStream, item: TokenStream) -> TokenStream {
    nadi_func(args, item, false)
}

#[derive(Clone, Copy, Default)]
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

fn nadi_func(args: TokenStream, item: TokenStream, node: bool) -> TokenStream {
    let args = parse_macro_input!(args with Punctuated<MetaNameValue, Comma>::parse_terminated);
    let default_args: HashMap<Ident, Expr> = args
        .into_iter()
        .map(|p| (p.path.segments.first().unwrap().ident.clone(), p.value))
        .collect();
    let item = parse_macro_input!(item as ItemFn);
    let arg0 = item
        .sig
        .inputs
        .first()
        .expect("Needs at least one parameter");
    let func_args: Vec<(&Ident, &Type, FuncArgType)> = item
        .sig
        .inputs
        .iter()
        .skip(1) // skip first argument, which is probably node or network
        .map(get_fn_arg)
        .collect();
    let warnings = check_args_kwargs_order(&func_args, &default_args);

    let func_struct_name = nadi_struct_name(&item.sig.ident, if node { "Node" } else { "Network" });

    let name_func = get_name_func(&item);
    let code_func = get_code_func(&item);
    let help_func = get_help_func(&item, &default_args);
    let call_func = get_call_func(
        &item,
        arg0,
        node,
        &func_args,
        &default_args,
        &func_struct_name,
    );
    let impl_trait = nadi_func_impl(node);

    let clean_func = clean_function(&item);

    quote! {
    #warnings

        #[derive(Debug)]
        pub struct #func_struct_name;

        impl #impl_trait for #func_struct_name {
            #name_func
            #code_func
            #help_func

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
        if type_is_opt(t) {
            flag = true;
        } else if default_args.contains_key(a) {
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
                let (_, remain): (Vec<_>, Vec<_>) = attrs
                    .into_iter()
                    .partition(|at| FUNC_ARG_ATTRS.iter().any(|a| at.path().is_ident(a.0)));

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

/// Register the plugin for NADI system. This should be on the top
/// level of the `mod`, with access to all the functions so it can
/// register them
///
/// # Example Use:
/// ```rust
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
    let node_funcs = get_nadi_functions(&item, "node_func");
    let network_funcs = get_nadi_functions(&item, "network_func");
    let regis_node_funcs = node_funcs
        .iter()
        .map(|n| nadi_struct_name(n, "Node"))
        .map(|n| {
            quote! {
                    nf.register_node_function(
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

fn get_code_func(item: &ItemFn) -> proc_macro2::TokenStream {
    let func_code = prettyplease::unparse(&syn::parse2(item.to_token_stream()).unwrap());

    quote! {
    fn code(&self) -> ::nadi_core::abi_stable::std_types::RString {
    #func_code .into()
    }}
}

fn get_call_func(
    item: &ItemFn,
    arg0: &FnArg,
    node: bool,
    args: &[(&Ident, &Type, FuncArgType)],
    defaults: &HashMap<Ident, Expr>,
    func_struct_name: &Ident,
) -> proc_macro2::TokenStream {
    let extract_args: Vec<proc_macro2::TokenStream> = args
        .iter()
        .enumerate()
        .map(|(i, (arg, ty, at))| {
            let arg_name = arg.to_string();
            let ty_name = ty.to_token_stream().to_string();
            let def = if let Some(val) = defaults.get(arg) {
		match ty {
                    Type::Reference(r) => {
			let inner_ty = &r.elem;
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
			quote!{
			    #warn
			    #inner_ty :: from ( #val )
			    }
		    },
		    _ => quote! {
                    #ty :: from ( #val )
                }
		}
            } else {

                quote! {
                    return ::nadi_core::abi_stable::std_types::RResult::RErr(
			format!("Argument {} ({} [{}]) is required", #i + 1, #arg_name, #ty_name).into()
                    );
                }
            };
            let isopt = type_is_opt(ty);
            let patterns = if isopt {

                quote! {
                    Some(Ok(v)) => Some(v),
                    Some(Err(e)) => return ::nadi_core::abi_stable::std_types::RResult::RErr(e.into()),
                    None => None,
                }
            } else {

                quote! {
                    Some(Ok(v)) => v,
                    Some(Err(e)) => return ::nadi_core::abi_stable::std_types::RResult::RErr(e.into()),
                    None => {#def},
                }
            };
            let arg_func = match at {
                FuncArgType::Arg => quote! { ctx.arg_kwarg },
                FuncArgType::Relaxed => quote! { ctx.arg_kwarg_relaxed },
                FuncArgType::Args => {
                    return quote! {
                        let #arg: #ty = ctx.args().into();
                    }
                }
                FuncArgType::KwArgs => {
                    return quote! {
                        let #arg: #ty = ctx.kwargs().into();
                    }
                }
            };
            match ty {
                Type::Reference(r) => {
                    let arg_o = format_ident!("{}_o", arg);
		    let inner_ty = &r.elem;
		    // althought mut is used here for type confirmity,
		    // mut is not supported and will not compile
		    let m = r.mutability;
                    quote! {
			let #m #arg_o : #inner_ty = match #arg_func (#i, #arg_name) {
			    #patterns
			};
			let #arg : #ty = & #m #arg_o;
                    }
		},
		_ => {

                    quote! {
			let #arg : #ty = match #arg_func (#i, #arg_name) {
			    #patterns
			};
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
    let arg0_name = get_fn_arg(arg0).0;

    if node {
        quote! {
            fn call(&self,
            nodes: ::nadi_core::abi_stable::std_types::RSlice<::nadi_core::node::Node>,
            ctx: &::nadi_core::functions::FunctionCtx)
                    -> ::nadi_core::abi_stable::std_types::RResult<(), ::nadi_core::abi_stable::std_types::RString>
            {
        #(#extract_args)*
        for #arg0_name in nodes {
            if let ::nadi_core::functions::FunctionRet::Error(e) = ::nadi_core::functions::FunctionRet::from(
            #func_struct_name :: #func_name(&mut #arg0_name .lock(), #(#args_n),*)
            ) {
            return ::nadi_core::abi_stable::std_types::RResult::RErr(e);
            }
        }
        ::nadi_core::abi_stable::std_types::ROk(())
            }
        }
    } else {
        quote! {
            fn call(&self,
                    #arg0_name : &mut ::nadi_core::network::Network,
                    ctx: &::nadi_core::functions::FunctionCtx)
                    -> ::nadi_core::abi_stable::std_types::RResult<(), ::nadi_core::abi_stable::std_types::RString> {

                #(#extract_args)*
        if let ::nadi_core::functions::FunctionRet::Error(e) = ::nadi_core::functions::FunctionRet::from(
                    #func_struct_name :: #func_name(#arg0_name, #(#args_n),*)

                ) {
                    ::nadi_core::abi_stable::std_types::RResult::RErr(e)
        } else {
            ::nadi_core::abi_stable::std_types::RResult::ROk(())
        }
            }
        }
    }
}

// get an expression that can generate function documentation
fn get_help_func(item: &ItemFn, default_args: &HashMap<Ident, Expr>) -> proc_macro2::TokenStream {
    let mut docs = get_doc(&item.attrs);

    let args: Vec<String> = item
        .sig
        .inputs
        .iter()
        .skip(1) // skip first argument, which should be node or network
        .map(|a| match a {
            syn::FnArg::Typed(a) => {
                // args and kwargs function signature
                if a.attrs.iter().any(|at| at.path().is_ident("args")) {
                    "*args".into()
                } else if a.attrs.iter().any(|at| at.path().is_ident("kwargs")) {
                    "**kwargs".into()
                } else {
                    match a.pat.as_ref() {
                        syn::Pat::Ident(i) => {
                            if default_args.contains_key(&i.ident) {
                                format!(
                                    "{} [{}] = {{{}:?}}",
                                    i.ident,
                                    a.ty.as_ref().into_token_stream(),
                                    i.ident
                                )
                            } else {
                                format!("{} [{}]", i.ident, a.ty.as_ref().into_token_stream())
                            }
                        }
                        _ => panic!("Not supported"),
                    }
                }
            }
            _ => panic!("Not supported"),
        })
        .collect();

    // function signature showing the function name, arguments and
    // their default values
    docs.push("\n# Signature:".into());
    docs.push(format!("{}({})\n", &item.sig.ident, args.join(", ")));
    let docs = docs.join("\n");
    if default_args.is_empty() {
        quote! {
            fn help(&self) -> ::nadi_core::abi_stable::std_types::RString {
        #docs .into()
            }
        }
    } else {
        let values: Vec<proc_macro2::TokenStream> = default_args
            .iter()
            .map(|(k, v)| quote! { let #k = #v; })
            .collect();

        quote! {
            fn help(&self) -> ::nadi_core::abi_stable::std_types::RString {
        #(#values)*
        format!(#docs) .into()
            }
        }
    }
}

/// collect doc attributes
fn get_doc(attrs: &[Attribute]) -> Vec<String> {
    attrs
        .iter()
        .filter(|a| a.path().is_ident("doc"))
        .filter_map(|a| match &a.meta {
            syn::Meta::NameValue(val) => match &val.value {
                Expr::Lit(lit) => match &lit.lit {
                    Lit::Str(c) => {
                        let c = c.value();
                        Some(c.strip_prefix(' ').unwrap_or(c.as_str()).to_string())
                    }
                    _ => None,
                },
                _ => None,
            },
            _ => None,
        })
        .collect()
}
