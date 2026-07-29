use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{ItemFn, parse_macro_input};

fn info_const_name(fn_name: &str) -> syn::Ident {
    format_ident!("ORC_FN_INFO_{}", fn_name.to_uppercase())
}

fn extract_doc(func: &ItemFn) -> String {
    func.attrs
        .iter()
        .filter(|a| a.path().is_ident("doc"))
        .filter_map(|a| match &a.meta {
            syn::Meta::NameValue(nv) => match &nv.value {
                syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Str(s),
                    ..
                }) => Some(s.value()),
                _ => None,
            },
            _ => None,
        })
        .map(|l| l.trim().to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

/// `#[orc_fn]` or `#[orc_fn(name = "add")]` — generates an `OrcFuncInfo` const alongside the function.
#[proc_macro_attribute]
pub fn orc_generate_fn_info(attr: TokenStream, item: TokenStream) -> TokenStream {
    let func = parse_macro_input!(item as ItemFn);
    let fn_name = &func.sig.ident;
    let fn_name_str = fn_name.to_string();
    let const_name = info_const_name(&fn_name_str);
    let display_name = if attr.is_empty() {
        fn_name_str.clone()
    } else {
        let lit = parse_macro_input!(attr as syn::LitStr);
        lit.value()
    };
    let desc = extract_doc(&func);
    let name_bytes = proc_macro2::Literal::byte_string(format!("{display_name}\0").as_bytes());
    let desc_bytes = proc_macro2::Literal::byte_string(format!("{desc}\0").as_bytes());
    quote! {
        #func
        const #const_name: orc_sdk::OrcFuncInfo = orc_sdk::OrcFuncInfo {
            name: #name_bytes.as_ptr().cast(),
            desc: #desc_bytes.as_ptr().cast(),
            func: Some(#fn_name),
        };
    }
    .into()
}

/// `orc_fn_info!(plugin_fn_add)` expands to `ORC_FN_INFO_PLUGIN_FN_ADD`.
#[proc_macro]
pub fn orc_fn_info(input: TokenStream) -> TokenStream {
    let ident = parse_macro_input!(input as syn::Ident);
    let const_name = info_const_name(&ident.to_string());
    quote! { #const_name }.into()
}

struct ValidatedParams {
    input_params: Box<[syn::PatType]>,
    output_params: Box<[syn::PatType]>,
}

fn is_output_param(ty: &syn::Type) -> Result<bool, proc_macro2::TokenStream> {
    match ty {
        syn::Type::Reference(r) if r.mutability.is_some() => Ok(true),
        syn::Type::Path(p)
            if p.path
                .segments
                .last()
                .map_or(false, |s| s.ident == "DeckWriter") =>
        {
            Ok(true)
        }
        syn::Type::Reference(_) | syn::Type::Path(_) => Ok(false),
        other => Err(syn::Error::new_spanned(
            other,
            "run parameter must be a value, &T, &mut T, DeckView<T>, or DeckWriter<T>",
        )
        .to_compile_error()),
    }
}

fn validate_orc_fn(
    run_fn: &syn::ItemFn,
    dims_fn: Option<&syn::ItemFn>,
    types: Option<&syn::Type>,
    registry: Option<&syn::Expr>,
    input_depths: Option<&syn::ExprArray>,
    output_depths: Option<&syn::ExprArray>,
) -> Result<ValidatedParams, proc_macro2::TokenStream> {
    // First parameter must be `ctx: u64`.
    let ctx_ok = matches!(
        run_fn.sig.inputs.first(),
        Some(syn::FnArg::Typed(pt))
            if matches!(pt.pat.as_ref(), syn::Pat::Ident(pi) if pi.ident == "ctx")
                && matches!(pt.ty.as_ref(), syn::Type::Path(p) if p.path.is_ident("u64"))
    );
    if !ctx_ok {
        return Err(syn::Error::new_spanned(
            &run_fn.sig,
            "first parameter of fn run must be `ctx: u64`",
        )
        .to_compile_error());
    }
    // Classify remaining params (skip ctx).
    let mut input_params: Vec<syn::PatType> = Vec::new();
    let mut output_params: Vec<syn::PatType> = Vec::new();
    let mut saw_output = false;
    for arg in run_fn.sig.inputs.iter().skip(1) {
        let pat_ty = match arg {
            syn::FnArg::Typed(pt) => pt,
            syn::FnArg::Receiver(r) => {
                return Err(
                    syn::Error::new_spanned(r, "run must not have a self parameter")
                        .to_compile_error(),
                );
            }
        };
        let is_out = is_output_param(pat_ty.ty.as_ref())?;
        if is_out {
            saw_output = true;
            output_params.push(pat_ty.clone());
        } else {
            if saw_output {
                return Err(syn::Error::new_spanned(
                    pat_ty,
                    "all inputs must precede all outputs in `fn run`",
                )
                .to_compile_error());
            }
            input_params.push(pat_ty.clone());
        }
    }
    // If run_fn has type generics, Types must be defined with matching arity.
    let generic_count = run_fn
        .sig
        .generics
        .params
        .iter()
        .filter(|p| matches!(p, syn::GenericParam::Type(_)))
        .count();
    if generic_count > 0 {
        match types {
            None => {
                return Err(syn::Error::new_spanned(
                    &run_fn.sig,
                    "fn run is generic; Types must be defined",
                )
                .to_compile_error());
            }
            Some(ty) => {
                let arity = match ty {
                    syn::Type::Tuple(outer) => match outer.elems.first() {
                        Some(syn::Type::Tuple(inner)) => inner.elems.len(),
                        Some(_) => 1,
                        None => 0,
                    },
                    _ => 0,
                };
                if arity != generic_count {
                    return Err(syn::Error::new_spanned(
                        ty,
                        format!(
                            "Types arity ({arity}) must match the number of type parameters in fn run ({generic_count})"
                        ),
                    )
                    .to_compile_error());
                }
            }
        }
    }
    // Outputs require a registry.
    if !output_params.is_empty() && registry.is_none() {
        return Err(syn::Error::new_spanned(
            &run_fn.sig,
            "fn run has output parameters; `let registry: &ObjectRegistry = ...` must be defined",
        )
        .to_compile_error());
    }
    // Depth arrays must be defined and match argument counts when count > 0.
    if !input_params.is_empty() {
        match input_depths {
            None => {
                return Err(syn::Error::new_spanned(
                    &run_fn.sig,
                    format!(
                        "INPUT_DEPTHS must be defined with {} element(s) to match fn run inputs",
                        input_params.len()
                    ),
                )
                .to_compile_error());
            }
            Some(arr) if arr.elems.len() != input_params.len() => {
                return Err(syn::Error::new_spanned(
                    arr,
                    format!(
                        "INPUT_DEPTHS has {} element(s) but fn run has {} input(s)",
                        arr.elems.len(),
                        input_params.len()
                    ),
                )
                .to_compile_error());
            }
            _ => {}
        }
    }
    if !output_params.is_empty() {
        match output_depths {
            None => {
                return Err(syn::Error::new_spanned(
                    &run_fn.sig,
                    format!(
                        "OUTPUT_DEPTHS must be defined with {} element(s) to match fn run outputs",
                        output_params.len()
                    ),
                )
                .to_compile_error());
            }
            Some(arr) if arr.elems.len() != output_params.len() => {
                return Err(syn::Error::new_spanned(
                    arr,
                    format!(
                        "OUTPUT_DEPTHS has {} element(s) but fn run has {} output(s)",
                        arr.elems.len(),
                        output_params.len()
                    ),
                )
                .to_compile_error());
            }
            _ => {}
        }
    }
    // Validate dims_fn if present.
    if let Some(dims) = dims_fn {
        let n_inputs = input_params.len();
        let n_outputs = output_params.len();
        let expected_total = n_inputs + n_outputs;
        let dims_args: Vec<_> = dims.sig.inputs.iter().collect();
        if dims_args.len() != expected_total {
            return Err(syn::Error::new_spanned(
                &dims.sig,
                format!(
                    "fn dims must have {expected_total} parameter(s) ({n_inputs} input(s) + {n_outputs} output(s)) to match fn run"
                ),
            )
            .to_compile_error());
        }
        for (i, arg) in dims_args.iter().enumerate() {
            let pat_ty = match arg {
                syn::FnArg::Typed(pt) => pt,
                syn::FnArg::Receiver(r) => {
                    return Err(
                        syn::Error::new_spanned(r, "dims must not have a self parameter")
                            .to_compile_error(),
                    );
                }
            };
            let is_input = i < n_inputs;
            match pat_ty.ty.as_ref() {
                syn::Type::Reference(r) => {
                    if is_input && r.mutability.is_some() {
                        return Err(syn::Error::new_spanned(
                            pat_ty,
                            "dims input parameters must be immutable references (&OrcDims)",
                        )
                        .to_compile_error());
                    }
                    if !is_input && r.mutability.is_none() {
                        return Err(syn::Error::new_spanned(
                            pat_ty,
                            "dims output parameters must be mutable references (&mut OrcDims)",
                        )
                        .to_compile_error());
                    }
                    if !matches!(r.elem.as_ref(), syn::Type::Path(p) if p.path.segments.last().map_or(false, |s| s.ident == "OrcDims"))
                    {
                        return Err(syn::Error::new_spanned(
                            r.elem.as_ref(),
                            "dims parameters must reference OrcDims",
                        )
                        .to_compile_error());
                    }
                }
                other => {
                    return Err(syn::Error::new_spanned(
                        other,
                        "dims parameters must be references to OrcDims",
                    )
                    .to_compile_error());
                }
            }
        }
    }
    Ok(ValidatedParams {
        input_params: input_params.into_boxed_slice(),
        output_params: output_params.into_boxed_slice(),
    })
}

/// Expands `orc_fn! name { ... }` into a full FFI function + OrcFuncInfo const.
#[proc_macro]
pub fn orc_fn(input: TokenStream) -> TokenStream {
    use proc_macro2::TokenTree;
    let mut iter = proc_macro2::TokenStream::from(input).into_iter();
    let name = match iter.next() {
        Some(TokenTree::Ident(i)) => i,
        other => {
            return syn::Error::new(
                other
                    .map(|t| t.span())
                    .unwrap_or(proc_macro2::Span::call_site()),
                "expected function name",
            )
            .to_compile_error()
            .into();
        }
    };
    let body = match iter.next() {
        Some(TokenTree::Group(g)) if g.delimiter() == proc_macro2::Delimiter::Brace => g,
        other => {
            return syn::Error::new(
                other
                    .map(|t| t.span())
                    .unwrap_or(proc_macro2::Span::call_site()),
                "expected `{` after function name",
            )
            .to_compile_error()
            .into();
        }
    };
    // Parse the body as a sequence of Rust statements and items.
    let stmts = syn::parse::Parser::parse2(
        |input: syn::parse::ParseStream| {
            let mut v = Vec::new();
            while !input.is_empty() {
                v.push(input.parse::<syn::Stmt>()?);
            }
            Ok(v)
        },
        body.stream(),
    );
    let stmts = match stmts {
        Ok(v) => v,
        Err(e) => return e.to_compile_error().into(),
    };
    let mut docs = String::new();
    // fn run(ctx: u64, <inputs...>, <outputs...>) { ... }  — required.
    let mut run_fn: Option<syn::ItemFn> = None;
    // fn dims(<&OrcDims inputs...>, <&mut OrcDims outputs...>) { ... }  — optional.
    let mut dims_fn: Option<syn::ItemFn> = None;
    // const INPUT_DEPTHS: [u8; N] = [...];
    let mut input_depths: Option<syn::ExprArray> = None;
    // const OUTPUT_DEPTHS: [u8; N] = [...];
    let mut output_depths: Option<syn::ExprArray> = None;
    // type Types = ((f32, f64), (f32, f32), ...);  — optional, one row per monomorphization.
    let mut types: Option<syn::Type> = None;
    // let registry: &ObjectRegistry = &MY_REGISTRY;
    let mut registry: Option<syn::Expr> = None;
    // Anything unrecognized is collected here and pasted verbatim into the function body.
    let mut user_items: Vec<proc_macro2::TokenStream> = Vec::new();
    for stmt in &stmts {
        // Collect doc attributes from any item in the body into the description string.
        let attrs: &[syn::Attribute] = match stmt {
            syn::Stmt::Item(item) => match item {
                syn::Item::Const(c) => &c.attrs,
                syn::Item::Fn(f) => &f.attrs,
                syn::Item::Type(t) => &t.attrs,
                _ => &[],
            },
            syn::Stmt::Local(l) => &l.attrs,
            _ => &[],
        };
        for attr in attrs {
            if attr.path().is_ident("doc") {
                if let syn::Meta::NameValue(nv) = &attr.meta {
                    if let syn::Expr::Lit(syn::ExprLit {
                        lit: syn::Lit::Str(s),
                        ..
                    }) = &nv.value
                    {
                        if !docs.is_empty() {
                            docs.push(' ');
                        }
                        docs.push_str(s.value().trim());
                    }
                }
            }
        }
        match stmt {
            syn::Stmt::Item(syn::Item::Const(c)) => match c.ident.to_string().as_str() {
                "INPUT_DEPTHS" | "OUTPUT_DEPTHS" => {
                    let is_u8_array = matches!(c.ty.as_ref(),
                        syn::Type::Array(syn::TypeArray { elem, .. })
                        if matches!(elem.as_ref(), syn::Type::Path(p) if p.path.is_ident("u8"))
                    );
                    if !is_u8_array {
                        return syn::Error::new_spanned(
                            &c.ty,
                            "INPUT_DEPTHS and OUTPUT_DEPTHS must be of type [u8; N]",
                        )
                        .to_compile_error()
                        .into();
                    }
                    if let syn::Expr::Array(arr) = c.expr.as_ref() {
                        if c.ident == "INPUT_DEPTHS" {
                            input_depths = Some(arr.clone());
                        } else {
                            output_depths = Some(arr.clone());
                        }
                    }
                }
                _ => user_items.push(quote::quote!(#c)),
            },
            syn::Stmt::Item(syn::Item::Type(t)) if t.ident == "Types" => {
                let outer = match t.ty.as_ref() {
                    syn::Type::Tuple(tup) => tup,
                    _ => {
                        return syn::Error::new_spanned(&t.ty, "Types must be a tuple")
                            .to_compile_error()
                            .into();
                    }
                };
                // Shape is determined by the first element: if it's a tuple of N types,
                // all others must also be N-tuples. If it's a plain type, all must be plain.
                let first_inner_len: Option<usize> = match outer.elems.first() {
                    None => {
                        return syn::Error::new_spanned(&t.ty, "Types cannot be an empty tuple")
                            .to_compile_error()
                            .into();
                    }
                    Some(syn::Type::Tuple(inner)) => Some(inner.elems.len()),
                    Some(_) => None,
                };
                for elem in outer.elems.iter().skip(1) {
                    match (first_inner_len, elem) {
                        (Some(n), syn::Type::Tuple(inner)) if inner.elems.len() == n => {}
                        (Some(n), syn::Type::Tuple(inner)) => {
                            return syn::Error::new_spanned(
                                elem,
                                format!(
                                    "expected a {n}-tuple; all rows of Types must have the same length, found {}",
                                    inner.elems.len()
                                ),
                            )
                            .to_compile_error()
                            .into();
                        }
                        (Some(n), _) => {
                            return syn::Error::new_spanned(elem, format!("expected a {n}-tuple"))
                                .to_compile_error()
                                .into();
                        }
                        (None, syn::Type::Tuple(_)) => {
                            return syn::Error::new_spanned(
                                elem,
                                "all elements of Types must match the shape of the first element",
                            )
                            .to_compile_error()
                            .into();
                        }
                        (None, _) => {} // Both plain types — valid.
                    }
                }
                types = Some(*t.ty.clone());
            }
            syn::Stmt::Item(syn::Item::Fn(f)) => match f.sig.ident.to_string().as_str() {
                "run" => run_fn = Some(f.clone()),
                "dims" => dims_fn = Some(f.clone()),
                _ => user_items.push(quote::quote!(#f)),
            },
            syn::Stmt::Local(local) => {
                let mut recognized = false;
                if let syn::Pat::Type(pt) = &local.pat {
                    if let syn::Pat::Ident(pi) = pt.pat.as_ref() {
                        if pi.ident == "registry" {
                            if let Some(init) = &local.init {
                                registry = Some(*init.expr.clone());
                                recognized = true;
                            }
                        }
                    }
                }
                if !recognized {
                    user_items.push(quote::quote!(#local));
                }
            }
            _ => user_items.push(quote::quote!(#stmt)),
        }
    }

    // run_fn is required.
    let run_fn = match run_fn {
        Some(f) => f,
        None => {
            return syn::Error::new(
                proc_macro2::Span::call_site(),
                "orc_fn! requires a `fn run(...)` body",
            )
            .to_compile_error()
            .into();
        }
    };

    let _validated = match validate_orc_fn(
        &run_fn,
        dims_fn.as_ref(),
        types.as_ref(),
        registry.as_ref(),
        input_depths.as_ref(),
        output_depths.as_ref(),
    ) {
        Ok(v) => v,
        Err(e) => return e.into(),
    };

    let _ = (name, docs, user_items);
    todo!()
}
