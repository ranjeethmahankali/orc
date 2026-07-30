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
    let name_lit = proc_macro2::Literal::c_string(
        &std::ffi::CString::new(display_name).expect("function name contains a null byte"),
    );
    let desc_lit = proc_macro2::Literal::c_string(
        &std::ffi::CString::new(desc).expect("doc comment contains a null byte"),
    );
    quote! {
        const #const_name: orc_sdk::OrcFuncInfo = orc_sdk::OrcFuncInfo {
            name: #name_lit.as_ptr(),
            desc: #desc_lit.as_ptr(),
            func: Some(#fn_name),
        };
        #func
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
    input_depths: Box<[u8]>,
    output_depths: Box<[u8]>,
}

fn is_deck_type(ty: &syn::Type) -> bool {
    matches!(ty, syn::Type::Path(p)
        if p.path.segments.last().map_or(false, |s| s.ident == "DeckView" || s.ident == "DeckWriter"))
}

fn infer_depth(ty: &syn::Type) -> u8 {
    match ty {
        syn::Type::Reference(r) => match r.elem.as_ref() {
            syn::Type::Slice(_) => 1,
            _ => 0,
        },
        _ => 0,
    }
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

/// Returns `Ok` if `generic` doesn't appear in `ty`, or appears only in an allowed position.
///
/// Allowed forms (where `T` is the generic):
/// ```text
/// T              — value input
/// &T             — shared-reference input
/// &mut T         — mutable-reference output
/// &[T]           — slice input (depth-1 list)
/// DeckView<T>    — higher-depth list input
/// DeckWriter<T>  — higher-depth list output
/// ```
///
/// Anything else that contains `T` is rejected:
/// ```text
/// &mut [T]       — error (use DeckWriter<T> for list outputs)
/// Box<T>         — error
/// Vec<T>         — error
/// Option<T>      — error
/// (T, T)         — error
/// ```
fn check_generic_in_param(
    ty: &syn::Type,
    generic: &syn::Ident,
) -> Result<(), proc_macro2::TokenStream> {
    match ty {
        syn::Type::Path(p) if p.path.is_ident(generic) => return Ok(()),
        syn::Type::Reference(r) => match r.elem.as_ref() {
            syn::Type::Path(p) if p.path.is_ident(generic) => return Ok(()),
            // &[T] is allowed (immutable slice input). &mut [T] is not — use DeckWriter<T> instead.
            syn::Type::Slice(s)
                if r.mutability.is_none()
                    && matches!(s.elem.as_ref(), syn::Type::Path(p) if p.path.is_ident(generic)) =>
            {
                return Ok(());
            }
            _ => {}
        },
        syn::Type::Path(p) => {
            if let Some(seg) = p.path.segments.last() {
                if seg.ident == "DeckView" || seg.ident == "DeckWriter" {
                    if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
                        if args.args.len() == 1 {
                            if let Some(syn::GenericArgument::Type(inner)) = args.args.first() {
                                if matches!(inner, syn::Type::Path(p) if p.path.is_ident(generic)) {
                                    return Ok(());
                                }
                            }
                        }
                    }
                }
            }
        }
        _ => {}
    }
    // Type didn't match any allowed pattern. Error only if the generic appears inside it.
    if generic_appears_in(ty, generic) {
        Err(syn::Error::new_spanned(
            ty,
            format!(
                "generic `{generic}` may only appear as `{generic}`, `&{generic}`, `&mut {generic}`, \
                 `&[{generic}]`, `DeckView<{generic}>`, or `DeckWriter<{generic}>`"
            ),
        )
        .to_compile_error())
    } else {
        Ok(())
    }
}

fn generic_appears_in(ty: &syn::Type, generic: &syn::Ident) -> bool {
    match ty {
        syn::Type::Path(p) => {
            p.path.is_ident(generic)
                || p.path.segments.iter().any(|seg| {
                    if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
                        args.args.iter().any(|arg| {
                            if let syn::GenericArgument::Type(t) = arg {
                                generic_appears_in(t, generic)
                            } else {
                                false
                            }
                        })
                    } else {
                        false
                    }
                })
        }
        syn::Type::Reference(r) => generic_appears_in(r.elem.as_ref(), generic),
        syn::Type::Slice(s) => generic_appears_in(s.elem.as_ref(), generic),
        syn::Type::Tuple(t) => t.elems.iter().any(|e| generic_appears_in(e, generic)),
        _ => false,
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
    // First parameter must be `u64` (the context handle).
    let ctx_ok = matches!(
        run_fn.sig.inputs.first(),
        Some(syn::FnArg::Typed(pt))
            if matches!(pt.ty.as_ref(), syn::Type::Path(p) if p.path.is_ident("u64"))
    );
    if !ctx_ok {
        return Err(syn::Error::new_spanned(
            &run_fn.sig,
            "first parameter of fn run must be of type `u64` (the context handle)",
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
    // Const generics are not supported.
    if let Some(const_param) = run_fn
        .sig
        .generics
        .params
        .iter()
        .find(|p| matches!(p, syn::GenericParam::Const(_)))
    {
        return Err(syn::Error::new_spanned(
            const_param,
            "const generics are not supported in fn run",
        )
        .to_compile_error());
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
        // Each generic must only appear in allowed positions across all params.
        let all_params = input_params.iter().chain(output_params.iter());
        for param in all_params {
            for generic_param in run_fn.sig.generics.params.iter() {
                if let syn::GenericParam::Type(tp) = generic_param {
                    check_generic_in_param(param.ty.as_ref(), &tp.ident)?;
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
    // Resolve depth arrays: use explicitly provided values, or auto-infer from parameter types.
    // Auto-inference is not possible for DeckView/DeckWriter params — those require explicit depths.
    let computed_input_depths: Box<[u8]> = match input_depths {
        Some(arr) => {
            if arr.elems.len() != input_params.len() {
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
            arr.elems
                .iter()
                .map(|e| match e {
                    syn::Expr::Lit(syn::ExprLit {
                        lit: syn::Lit::Int(i),
                        ..
                    }) => i.base10_parse::<u8>().unwrap_or(0),
                    _ => 0,
                })
                .collect()
        }
        None => {
            if let Some(p) = input_params.iter().find(|p| is_deck_type(p.ty.as_ref())) {
                return Err(syn::Error::new_spanned(
                    p,
                    "INPUT_DEPTHS must be defined when any input uses DeckView or DeckWriter",
                )
                .to_compile_error());
            }
            input_params
                .iter()
                .map(|p| infer_depth(p.ty.as_ref()))
                .collect()
        }
    };
    let computed_output_depths: Box<[u8]> = match output_depths {
        Some(arr) => {
            if arr.elems.len() != output_params.len() {
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
            arr.elems
                .iter()
                .map(|e| match e {
                    syn::Expr::Lit(syn::ExprLit {
                        lit: syn::Lit::Int(i),
                        ..
                    }) => i.base10_parse::<u8>().unwrap_or(0),
                    _ => 0,
                })
                .collect()
        }
        None => {
            if let Some(p) = output_params.iter().find(|p| is_deck_type(p.ty.as_ref())) {
                return Err(syn::Error::new_spanned(
                    p,
                    "OUTPUT_DEPTHS must be defined when any output uses DeckView or DeckWriter",
                )
                .to_compile_error());
            }
            output_params
                .iter()
                .map(|p| infer_depth(p.ty.as_ref()))
                .collect()
        }
    };
    // Validate dims_fn if present.
    if let Some(dims) = dims_fn {
        validate_dims_fn(dims, input_params.len(), output_params.len())?;
    }
    Ok(ValidatedParams {
        input_params: input_params.into_boxed_slice(),
        output_params: output_params.into_boxed_slice(),
        input_depths: computed_input_depths,
        output_depths: computed_output_depths,
    })
}

fn validate_dims_fn(
    dims: &ItemFn,
    n_inputs: usize,
    n_outputs: usize,
) -> Result<(), proc_macro2::TokenStream> {
    // First parameter must be `u64` (the context handle), matching fn run.
    let ctx_ok = matches!(
        dims.sig.inputs.first(),
        Some(syn::FnArg::Typed(pt))
            if matches!(pt.ty.as_ref(), syn::Type::Path(p) if p.path.is_ident("u64"))
    );
    if !ctx_ok {
        return Err(syn::Error::new_spanned(
            &dims.sig,
            "first parameter of fn dims must be of type `u64` (the context handle)",
        )
        .to_compile_error());
    }
    let expected_total = 1 + n_inputs + n_outputs;
    let dims_args: Vec<_> = dims.sig.inputs.iter().collect();
    if dims_args.len() != expected_total {
        return Err(syn::Error::new_spanned(
            &dims.sig,
            format!(
                "fn dims must have {expected_total} parameter(s) (ctx + {n_inputs} input(s) + {n_outputs} output(s)) to match fn run"
            ),
        )
        .to_compile_error());
    }
    for (i, arg) in dims_args.iter().skip(1).enumerate() {
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
    Ok(())
}

fn generate_orc_fn(
    name: &proc_macro2::Ident,
    docs: &str,
    run_fn: &syn::ItemFn,
    dims_fn: Option<&syn::ItemFn>,
    _types: Option<&syn::Type>,
    _registry: Option<&syn::Expr>,
    host_expr: &syn::Expr,
    _input_depths: Option<&syn::ExprArray>,
    _output_depths: Option<&syn::ExprArray>,
    user_items: &[proc_macro2::TokenStream],
    params: &ValidatedParams,
) -> proc_macro2::TokenStream {
    let info_name = info_const_name(&name.to_string());
    let n_inputs = params.input_params.len();
    let n_outputs = params.output_params.len();
    let name_lit = proc_macro2::Literal::c_string(
        &std::ffi::CString::new(name.to_string()).expect("name contains null byte"),
    );
    let desc_lit = proc_macro2::Literal::c_string(
        &std::ffi::CString::new(docs).expect("docs contains null byte"),
    );
    let dims_fn_tokens = dims_fn.map(|d| quote! { #d }).unwrap_or_default();
    quote! {
        const #info_name: orc_sdk::OrcFuncInfo = orc_sdk::OrcFuncInfo {
            name: #name_lit.as_ptr(),
            desc: #desc_lit.as_ptr(),
            func: Some(#name),
        };
        unsafe extern "C" fn #name(
            ctx: u64,
            inputs: *const orc_sdk::OrcHandle,
            n_inputs: u64,
            outputs: *mut orc_sdk::OrcHandle,
            n_outputs: u64,
        ) {
            #(#user_items)*
            #run_fn
            #dims_fn_tokens
            // Check the number of inputs.
            orc_sdk::orc_assert_return!(
                #host_expr,
                ctx,
                n_inputs == #n_inputs as u64,
                "Expected {} inputs, got {}",
                #n_inputs,
                n_inputs
            );
            // Check the number of outputs.
            orc_sdk::orc_assert_return!(
                #host_expr,
                ctx,
                n_outputs == #n_outputs as u64,
                "Expected {} outputs, got {}",
                #n_outputs,
                n_outputs
            );
            let inputs = unsafe { orc_sdk::slice_from_ptr(inputs, #n_inputs) };
            let outputs = unsafe { orc_sdk::slice_from_ptr_mut(outputs, #n_outputs) };
            todo!("dispatch to run")
        }
    }
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
    // let host: &HostCallbacks = host();
    let mut host: Option<syn::Expr> = None;
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
                        } else if pi.ident == "host" {
                            if let Some(init) = &local.init {
                                host = Some(*init.expr.clone());
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

    // host is required.
    let host = match host {
        Some(e) => e,
        None => {
            return syn::Error::new(
                proc_macro2::Span::call_site(),
                "orc_fn! requires `let host: &HostCallbacks = ...`",
            )
            .to_compile_error()
            .into();
        }
    };
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
    // Validate the rest.
    let validated_params = match validate_orc_fn(
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
    // Now that we validated everything, we can generate the code for this orc_fn.
    generate_orc_fn(
        &name,
        &docs,
        &run_fn,
        dims_fn.as_ref(),
        types.as_ref(),
        registry.as_ref(),
        &host,
        input_depths.as_ref(),
        output_depths.as_ref(),
        &user_items,
        &validated_params,
    )
    .into()
}
