use std::collections::HashMap;

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    parse_macro_input, punctuated::Punctuated, token::Comma, Expr, Ident, ItemFn, ItemMod, Lit,
    MetaNameValue,
};

/// macro to make plugin development easier
///
/// Right now it can only do automatic docstring extraction into a
/// separate function that can be called, but in future the plan is to
/// make it generate the part of the argument processing from the nadi
/// function args so that it is easier to write functions with rust
/// syntax, and all the errors will be on the parsing part that is
/// generated, and can be improved by the developers of the library.
#[proc_macro_attribute]
pub fn nadi_func(args: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(args with Punctuated<MetaNameValue, Comma>::parse_terminated);
    let item = parse_macro_input!(item as ItemFn);
    let func_name = &item.sig.ident;
    let func_help_name = format_ident!("{}_help", func_name);
    let mut docs = vec![];
    item.attrs
        .iter()
        .filter(|a| a.path().is_ident("doc"))
        .for_each(|a| match &a.meta {
            syn::Meta::NameValue(val) => match &val.value {
                Expr::Lit(lit) => match &lit.lit {
                    Lit::Str(c) => {
                        let c = c.value();
                        docs.push(c.strip_prefix(" ").unwrap_or(c.as_str()).to_string())
                    }
                    _ => (),
                },
                _ => (),
            },
            _ => (),
        });
    let docs = docs.join("\n");
    let mod_func = get_args_ext_function(&item, args);
    let new_func = quote!(
    #mod_func

    #[no_mangle]
    extern "C" fn #func_help_name () -> *const ::std::ffi::c_char {
        ::std::ffi::CString::new(#docs).unwrap().into_raw()
    }
    );
    // println!("{}", new_func);
    new_func.into()
}

fn get_args_ext_function(
    func: &ItemFn,
    args: Punctuated<MetaNameValue, Comma>,
) -> proc_macro2::TokenStream {
    let default_args: HashMap<String, Expr> = args
        .into_iter()
        .map(|p| (p.path.segments.first().unwrap().ident.to_string(), p.value))
        .collect();

    let sig = &func.sig;
    let mut inputs = sig.inputs.iter();
    let arg0 = if let Some(syn::FnArg::Typed(arg)) = inputs.next() {
        let arg0 = quote!(#arg).to_string();
        if !(arg0.contains("NodeInner") || arg0.contains("Network")) {
            panic!("Invalid Signature for the nadi function, needs either &mut NodeInner or &mut Network as the first argument")
        }
        arg
    } else {
        panic!("Needs at least one parameter")
    };
    let args: Vec<(Ident, proc_macro2::TokenStream)> = inputs
        .enumerate()
        .map(|(i, a)| match a {
            syn::FnArg::Typed(arg) => {
                let arg0 = quote!(#arg).to_string();
                if let Some((name, ty)) = arg0.split_once(":") {
		    let arg_name = name.trim();
                    let name = format_ident!("{}", arg_name);
		    // Default value given in nadi_func(name=value) pairs are inserted here
		    let def = if let Some(val) = default_args.get(&name.to_string()) {
			quote! {
			    #val
			}
		    } else {
			quote! {
			    ctx.set_error(::anyhow::Error::msg(format!("Value for argument {} ({}) is required", #i + 1, #arg_name)));
			    return;
			}
		    };
                    (
                        name.clone(),
                        match ty.trim() {
                            "String" => {
				// For letting anything with trait ToString be used for String ones
				let def_s = if let Some(val) = default_args.get(&name.to_string()) {
				    quote! {
					#val .to_string()
				    }
				} else {
				    quote! {
					ctx.set_error(::anyhow::Error::msg(format!("Value for argument {} ({}) is required", #i + 1, #arg_name)));
					return;
				    }
				};
                                quote!(let mut #name : String = if let Some(arg) = ctx.kwarg(#arg_name).or_else(||ctx.arg(#i)){
			 	if let Some(v) = arg.clone().into_string(){
				    v
				} else {
				    ctx.set_error(::anyhow::Error::msg(format!("Argument {} ({}) needs to be String", #i + 1, #arg_name)));
				    return;
				}
			    } else {
				#def_s
				};)
                            }
                            "i64" => {
                                quote!(let mut #name : i64 = if let Some(arg) = ctx.kwarg(#arg_name).or_else(||ctx.arg(#i)){
			 	if let Some(v) = arg.clone().into_loose_int(){
				    v
				} else {
				    ctx.set_error(::anyhow::Error::msg(format!("Argument {} ({}) needs to be int", #i + 1, #arg_name)));
				    return;
				}
			    } else {
				#def
				};)
                            }
                            "f64" => {
                                quote!(let mut #name : f64 = if let Some(arg) = ctx.kwarg(#arg_name).or_else(||ctx.arg(#i)) {
			 	if let Some(v) = arg.clone().into_loose_float(){
				    v
				} else {
				    ctx.set_error(::anyhow::Error::msg(format!("Argument {} ({}) needs to be float", #i + 1, #arg_name)));
				    return;
				}
			    } else {
				#def
				};)
                            }
                            "bool" => {
                                quote!(let mut #name : bool = if let Some(arg) = ctx.kwarg(#arg_name).or_else(||ctx.arg(#i)){
			 	arg.clone().into_loose_bool()
			    } else {
				#def
			    };)
                            }
                            _ => panic!("Unsupported Data type for nadi_function"),
                        },
                    )
                } else {
                    panic!("Invalid Argument to the function")
                }
            }
            _ => panic!("Invalid Argument to the function"),
        })
        .collect();
    let args_n: Vec<Ident> = args.iter().map(|v| v.0.clone()).collect();
    let args_s: Vec<proc_macro2::TokenStream> = args.into_iter().map(|v| v.1).collect();

    let func_name = &sig.ident;
    let func_inputs = &sig.inputs;
    let func_inner_name = format_ident!("__inner_{}", func_name);
    let func_block = &func.block;
    let func_ret = &sig.output;
    let arg0_name = format_ident!(
        "{}",
        quote!(#arg0).to_string().split(":").next().unwrap().trim()
    );

    quote! {
        #[no_mangle]
    extern "C" fn #func_name (#arg0, ctx: & mut ::nadi_core::plugins::FunctionCtx) {
        #(#args_s)*

        if let Err(e) = #func_inner_name(#arg0_name, #(#args_n),*) {
        ctx.set_error(e.into());
    }
    }

    fn #func_inner_name (#func_inputs) #func_ret #func_block
        }
}

/// this will be on the top level of the mod, will have access to all
/// the functions so it can see nadi_func entries and make the list.
/// untested right now.
#[proc_macro_attribute]
pub fn nadi_plugin(_args: TokenStream, item: TokenStream) -> TokenStream {
    // let args: syn::MetaSeq = syn::parse(args).unwrap();
    let item = parse_macro_input!(item as ItemMod);

    // todo: read the data from the module code and extract all the
    // function names
    let node_funcs = get_nadi_functions(&item, true);
    let network_funcs = get_nadi_functions(&item, false);
    quote!(
    #item

    #[no_mangle]
    extern "C" fn node_functions () -> *const ::std::ffi::c_char {
        ::std::ffi::CString::new(#node_funcs).unwrap().into_raw()
    }
    #[no_mangle]
    extern "C" fn network_functions () -> *const ::std::ffi::c_char {
        ::std::ffi::CString::new(#network_funcs).unwrap().into_raw()
    }
    )
    .into()
}

fn get_nadi_functions(item: &ItemMod, nodefunc: bool) -> String {
    if let Some((_, cont)) = &item.content {
        let funcs: Vec<String> = cont
            .iter()
            .filter_map(|c| {
                if let syn::Item::Fn(f) = c {
                    Some(f)
                } else {
                    None
                }
            })
            .filter_map(|f| {
                if f.attrs.iter().any(|a| match &a.meta {
                    syn::Meta::Path(p) => p.is_ident("nadi_func"),
		    syn::Meta::List(l) => l.path.is_ident("nadi_func"),
                    _ => false,
                }) {
                    if let Some(syn::FnArg::Typed(arg)) = f.sig.inputs.first() {
			let arg0 = quote!(#arg).to_string();
                        if arg0.contains("NodeInner") {
			    if nodefunc {
				Some(f.sig.ident.to_string())
			    } else {
				None
			    }
			} else if arg0.contains("Network") {
			    if !nodefunc {
				Some(f.sig.ident.to_string())
			    } else {
				None
			    }
			} else {
			    panic!("Invalid Signature for the nadi function, needs either &mut NodeInner or &mut Network as first argument")
			}
                    } else {
                        panic!("Invalid Signature for the nadi function, needs either &mut NodeInner or &mut Network as first argument")
                    }
                } else {
                    None
                }
            })
            .collect();
        funcs.join(",")
    } else {
        String::from("")
    }
}
