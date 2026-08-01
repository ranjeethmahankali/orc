use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{ItemFn, parse_macro_input};

fn info_const_name(fn_name: &str) -> syn::Ident {
    format_ident!("ORC_FN_INFO_{}", fn_name.to_uppercase())
}

fn docs_from_attrs(attrs: &[syn::Attribute]) -> String {
    attrs
        .iter()
        .filter(|a| a.path().is_ident("doc"))
        .filter_map(|a| {
            if let syn::Meta::NameValue(nv) = &a.meta
                && let syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Str(s),
                    ..
                }) = &nv.value
            {
                return Some(s.value().trim().to_string());
            }
            None
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn extract_doc(func: &ItemFn) -> String {
    docs_from_attrs(&func.attrs)
}

/// `#[orc_generate_fn_info]` or `#[orc_generate_fn_info("add")]` — generates an `OrcFuncInfo` const alongside the function.
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
        pub const #const_name: orc_sdk::OrcFuncInfo = orc_sdk::OrcFuncInfo {
            name: #name_lit.as_ptr(),
            desc: #desc_lit.as_ptr(),
            func: Some(#fn_name),
        };
        #func
    }
    .into()
}

/// `orc_fn_info!(add)` expands to `ORC_FN_INFO_ADD`.
/// `orc_fn_info!(basic::add)` expands to `basic::ORC_FN_INFO_ADD`.
#[proc_macro]
pub fn orc_fn_info(input: TokenStream) -> TokenStream {
    let path = parse_macro_input!(input as syn::Path);
    let last = match path.segments.last() {
        Some(seg) => seg,
        None => {
            return syn::Error::new(proc_macro2::Span::call_site(), "expected a path")
                .to_compile_error()
                .into();
        }
    };
    let const_name = info_const_name(&last.ident.to_string());
    if path.segments.len() == 1 {
        quote! { #const_name }.into()
    } else {
        let prefix: syn::punctuated::Punctuated<&syn::PathSegment, syn::Token![::]> =
            path.segments.iter().take(path.segments.len() - 1).collect();
        quote! { #prefix::#const_name }.into()
    }
}

struct ParamInfo {
    param: syn::PatType,
    depth: u8,
    inner_type: syn::Type,
}

struct ValidatedParams {
    inputs: Box<[ParamInfo]>,
    outputs: Box<[ParamInfo]>,
}

/// Returns the number of args in `Case<...>`, or `None` if the type is not `Case<...>`.
fn sig_arg_count(ty: &syn::Type) -> Option<usize> {
    if let syn::Type::Path(p) = ty
        && let Some(seg) = p.path.segments.last()
        && seg.ident == "Case"
        && let syn::PathArguments::AngleBracketed(args) = &seg.arguments
    {
        return Some(args.args.len());
    }
    None
}

fn is_deck_type(ty: &syn::Type) -> bool {
    matches!(ty, syn::Type::Path(p)
        if p.path.segments.last().is_some_and(|s| s.ident == "DeckView" || s.ident == "DeckWriter"))
}

fn infer_depth(ty: &syn::Type) -> Option<u8> {
    match ty {
        syn::Type::Reference(r) => match r.elem.as_ref() {
            syn::Type::Slice(_) => Some(1),
            _ => Some(0),
        },
        _ if is_deck_type(ty) => None,
        _ => Some(0),
    }
}

fn is_output_param(ty: &syn::Type) -> syn::Result<bool> {
    match ty {
        syn::Type::Reference(r) if r.mutability.is_some() => Ok(true),
        syn::Type::Path(p)
            if p.path
                .segments
                .last()
                .is_some_and(|s| s.ident == "DeckWriter") =>
        {
            Ok(true)
        }
        syn::Type::Reference(r) if r.mutability.is_none() => Ok(false),
        syn::Type::Path(p)
            if p.path
                .segments
                .last()
                .is_some_and(|s| s.ident == "DeckView") =>
        {
            Ok(false)
        }
        other => Err(syn::Error::new_spanned(
            other,
            "run parameter must be `&T`, `&[T]`, `&mut T`, `DeckView<T>`, or `DeckWriter<T>`",
        )),
    }
}

/// Strips depth wrappers and returns the inner element type:
/// `&[T]` → `T`, `&T` → `T`, `&mut T` → `T`,
/// `DeckView<T>` → `T`, `DeckWriter<T>` → `T`, anything else → itself.
fn inner_type(ty: &syn::Type) -> &syn::Type {
    match ty {
        syn::Type::Reference(r) => match r.elem.as_ref() {
            syn::Type::Slice(s) => s.elem.as_ref(),
            inner => inner,
        },
        syn::Type::Path(p) => {
            if let Some(seg) = p.path.segments.last()
                && (seg.ident == "DeckView" || seg.ident == "DeckWriter")
                && let syn::PathArguments::AngleBracketed(args) = &seg.arguments
                && let Some(syn::GenericArgument::Type(t)) = args.args.first()
            {
                return t;
            }
            ty
        }
        _ => ty,
    }
}

fn resolve_depths(
    explicit: Option<&syn::ExprArray>,
    params: &[syn::PatType],
    array_name: &str,
    param_kind: &str,
) -> syn::Result<Box<[u8]>> {
    let inferred: Vec<Option<u8>> = params.iter().map(|p| infer_depth(p.ty.as_ref())).collect();
    match explicit {
        Some(arr) => {
            if arr.elems.len() != params.len() {
                return Err(syn::Error::new_spanned(
                    arr,
                    format!(
                        "{array_name} has {} element(s) but fn run has {} {param_kind}(s)",
                        arr.elems.len(),
                        params.len()
                    ),
                ));
            }
            let mut result = Vec::with_capacity(params.len());
            for (i, e) in arr.elems.iter().enumerate() {
                let provided = match e {
                    syn::Expr::Lit(syn::ExprLit {
                        lit: syn::Lit::Int(i),
                        ..
                    }) => match i.base10_parse::<u8>() {
                        Ok(v) => v,
                        Err(_) => {
                            return Err(syn::Error::new_spanned(
                                i,
                                format!("{array_name} values must be u8 literals"),
                            ));
                        }
                    },
                    _ => {
                        return Err(syn::Error::new_spanned(
                            e,
                            format!("{array_name} values must be u8 literals"),
                        ));
                    }
                };
                if let Some(inf) = inferred[i]
                    && provided != inf
                {
                    return Err(syn::Error::new_spanned(
                        e,
                        format!(
                            "{array_name}[{i}] is {provided} but the parameter type implies depth {inf}, which does not match the provided depth {provided}"
                        ),
                    ));
                }
                result.push(provided);
            }
            Ok(result.into_boxed_slice())
        }
        None => {
            if let Some(i) = inferred.iter().position(|d| d.is_none()) {
                return Err(syn::Error::new_spanned(
                    &params[i],
                    format!(
                        "{array_name} must be specified; depth cannot be inferred for this parameter"
                    ),
                ));
            }
            Ok(inferred.into_iter().map(|d| d.unwrap()).collect())
        }
    }
}

fn validate_orc_fn(
    run_fn: &syn::ItemFn,
    dims_fn: Option<&syn::ItemFn>,
    types: Option<&syn::Type>,
    registry: Option<&syn::Expr>,
    input_depths: Option<&syn::ExprArray>,
    output_depths: Option<&syn::ExprArray>,
) -> syn::Result<ValidatedParams> {
    // First parameter must be a reference to the host callbacks.
    let host_ok = matches!(
        run_fn.sig.inputs.first(),
        Some(syn::FnArg::Typed(pt))
            if matches!(pt.ty.as_ref(), syn::Type::Reference(_))
    );
    if !host_ok {
        return Err(syn::Error::new_spanned(
            &run_fn.sig,
            "first parameter of fn run must be a reference to the host callbacks",
        ));
    }
    // fn run must return Result<_, _>.
    let returns_result = match &run_fn.sig.output {
        syn::ReturnType::Type(_, ty) => matches!(
            ty.as_ref(),
            syn::Type::Path(p)
                if p.path.segments.last().is_some_and(|s| s.ident == "Result")
        ),
        syn::ReturnType::Default => false,
    };
    if !returns_result {
        return Err(syn::Error::new_spanned(
            &run_fn.sig,
            "fn run must return `Result<(), Error>`",
        ));
    }
    // Classify remaining params (skip host).
    let mut input_params: Vec<syn::PatType> = Vec::new();
    let mut output_params: Vec<syn::PatType> = Vec::new();
    let mut saw_output = false;
    for arg in run_fn.sig.inputs.iter().skip(1) {
        let pat_ty = match arg {
            syn::FnArg::Typed(pt) => pt,
            syn::FnArg::Receiver(r) => {
                return Err(syn::Error::new_spanned(
                    r,
                    "`run` function must not have a self parameter",
                ));
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
                    "All inputs must precede all outputs in `fn run`",
                ));
            }
            input_params.push(pat_ty.clone());
        }
    }
    // If run_fn has generics (type or const), Types must be defined with matching arity.
    let generic_count = run_fn
        .sig
        .generics
        .params
        .iter()
        .filter(|p| matches!(p, syn::GenericParam::Type(_) | syn::GenericParam::Const(_)))
        .count();
    if generic_count > 0 {
        match types {
            None => {
                return Err(syn::Error::new_spanned(
                    &run_fn.sig,
                    "fn run is generic; Types must be defined",
                ));
            }
            Some(ty) => {
                let arity = match ty {
                    syn::Type::Tuple(outer) => {
                        outer.elems.first().and_then(sig_arg_count).unwrap_or(0)
                    }
                    _ => 0,
                };
                if arity != generic_count {
                    return Err(syn::Error::new_spanned(
                        ty,
                        format!(
                            "Each Case<...> in Types must have {generic_count} argument(s) to match fn run's generics, found {arity}"
                        ),
                    ));
                }
            }
        }
    }
    // Outputs require a registry.
    if !output_params.is_empty() && registry.is_none() {
        return Err(syn::Error::new_spanned(
            &run_fn.sig,
            "fn run has output parameters; `let registry: &ObjectRegistry = ...` must be defined",
        ));
    }
    // Resolve depth arrays. First infer depths from parameter types (None = cannot infer).
    // If all depths are inferrable, the user may omit the explicit array.
    // If any are None (DeckView/DeckWriter), the user must provide explicit depths.
    // When explicit depths are provided alongside inferrable ones, they must agree.
    let computed_input_depths =
        resolve_depths(input_depths, &input_params, "INPUT_DEPTHS", "input")?;
    let computed_output_depths =
        resolve_depths(output_depths, &output_params, "OUTPUT_DEPTHS", "output")?;
    // Validate dims_fn if present.
    if let Some(dims) = dims_fn {
        validate_dims_fn(dims, input_params.len(), output_params.len())?;
    }
    Ok(ValidatedParams {
        inputs: input_params
            .into_iter()
            .zip(computed_input_depths)
            .map(|(param, depth)| {
                let inner_type = inner_type(param.ty.as_ref()).clone();
                ParamInfo {
                    param,
                    depth,
                    inner_type,
                }
            })
            .collect(),
        outputs: output_params
            .into_iter()
            .zip(computed_output_depths)
            .map(|(param, depth)| {
                let inner_type = inner_type(param.ty.as_ref()).clone();
                ParamInfo {
                    param,
                    depth,
                    inner_type,
                }
            })
            .collect(),
    })
}

fn validate_dims_fn(dims: &ItemFn, n_inputs: usize, n_outputs: usize) -> syn::Result<()> {
    // fn dims must return Result<_, _>.
    let returns_result = match &dims.sig.output {
        syn::ReturnType::Type(_, ty) => matches!(
            ty.as_ref(),
            syn::Type::Path(p)
                if p.path.segments.last().is_some_and(|s| s.ident == "Result")
        ),
        syn::ReturnType::Default => false,
    };
    if !returns_result {
        return Err(syn::Error::new_spanned(
            &dims.sig,
            "fn dims must return `Result<(), Error>`",
        ));
    }
    let expected_total = n_inputs + n_outputs;
    let dims_args: Vec<_> = dims.sig.inputs.iter().collect();
    if dims_args.len() != expected_total {
        return Err(syn::Error::new_spanned(
            &dims.sig,
            format!(
                "fn dims must have {expected_total} parameter(s) ({n_inputs} input(s) + {n_outputs} output(s)) to match fn run"
            ),
        ));
    }
    for (i, arg) in dims_args.iter().enumerate() {
        let pat_ty = match arg {
            syn::FnArg::Typed(pt) => pt,
            syn::FnArg::Receiver(r) => {
                return Err(syn::Error::new_spanned(
                    r,
                    "dims must not have a self parameter",
                ));
            }
        };
        let is_input = i < n_inputs;
        match pat_ty.ty.as_ref() {
            syn::Type::Reference(r) => {
                if is_input && r.mutability.is_some() {
                    return Err(syn::Error::new_spanned(
                        pat_ty,
                        "dims input parameters must be immutable references (&OrcDims)",
                    ));
                }
                if !is_input && r.mutability.is_none() {
                    return Err(syn::Error::new_spanned(
                        pat_ty,
                        "dims output parameters must be mutable references (&mut OrcDims)",
                    ));
                }
                if !matches!(r.elem.as_ref(), syn::Type::Path(p) if p.path.segments.last().is_some_and(|s| s.ident == "OrcDims"))
                {
                    return Err(syn::Error::new_spanned(
                        r.elem.as_ref(),
                        "dims parameters must reference OrcDims",
                    ));
                }
            }
            other => {
                return Err(syn::Error::new_spanned(
                    other,
                    "dims parameters must be references to OrcDims",
                ));
            }
        }
    }
    Ok(())
}

/// If `ty` is a bare ident matching a type generic, returns the substituted concrete type.
/// Otherwise returns a clone of `ty` unchanged.
fn substitute_type(
    ty: &syn::Type,
    generics: &[&syn::GenericParam],
    case_args: &[&syn::GenericArgument],
) -> syn::Type {
    if let syn::Type::Path(p) = ty
        && let Some(ident) = p.path.get_ident()
    {
        for (gp, arg) in generics.iter().zip(case_args.iter()) {
            if let (syn::GenericParam::Type(tp), syn::GenericArgument::Type(concrete)) = (gp, arg)
                && tp.ident == *ident
            {
                return concrete.clone();
            }
        }
    }
    ty.clone()
}

fn generate_type_dispatch(
    run_fn: &syn::ItemFn,
    types: Option<&syn::Type>,
    params: &ValidatedParams,
    registry_expr: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let n_inputs = params.inputs.len();
    let scrutinee_elems: Vec<proc_macro2::TokenStream> = (0..n_inputs)
        .map(|i| quote! { __inputs[#i].type_id })
        .collect();
    let scrutinee = quote! { (#(#scrutinee_elems),*) };

    let all_generics: Vec<&syn::GenericParam> = run_fn.sig.generics.params.iter().collect();

    // Each entry: (case args, original Case type for error spans).
    // Non-generic / no Types → one empty case.
    let cases: Vec<(Vec<&syn::GenericArgument>, Option<&syn::Type>)> = match types {
        Some(syn::Type::Tuple(outer)) => outer
            .elems
            .iter()
            .map(|elem| {
                let args = match elem {
                    syn::Type::Path(p) => match p.path.segments.last() {
                        Some(seg) => match &seg.arguments {
                            syn::PathArguments::AngleBracketed(ab) => ab.args.iter().collect(),
                            _ => vec![],
                        },
                        None => vec![],
                    },
                    _ => vec![],
                };
                (args, Some(elem))
            })
            .collect(),
        _ => vec![(vec![], None)],
    };

    // Track unique types → their const ident.
    let mut type_to_const: std::collections::HashMap<String, proc_macro2::Ident> =
        std::collections::HashMap::new();
    // Ordered for deterministic const emission.
    let mut type_consts: Vec<(proc_macro2::Ident, syn::Type)> = Vec::new();

    let mut get_or_insert = |ty: &syn::Type| -> proc_macro2::Ident {
        let key = quote! { #ty }.to_string();
        if let Some(ident) = type_to_const.get(&key) {
            return ident.clone();
        }
        let mangled = key
            .replace("::", "_")
            .replace(" ", "")
            .replace("<", "_")
            .replace(">", "")
            .replace(",", "_")
            .to_uppercase();
        let ident = format_ident!("__ORC_TYPE_ID_{mangled}");
        type_to_const.insert(key, ident.clone());
        type_consts.push((ident.clone(), ty.clone()));
        ident
    };

    let mut seen_patterns: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut errors: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut arms: Vec<proc_macro2::TokenStream> = Vec::new();

    for (case_args, case_ty) in &cases {
        let mono_input_types: Vec<syn::Type> = params
            .inputs
            .iter()
            .map(|p| substitute_type(&p.inner_type, &all_generics, case_args))
            .collect();

        let pattern_idents: Vec<proc_macro2::Ident> =
            mono_input_types.iter().map(&mut get_or_insert).collect();

        let pattern_key = pattern_idents
            .iter()
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join(",");

        if !seen_patterns.insert(pattern_key) {
            if let Some(ty) = case_ty {
                errors.push(
                    syn::Error::new_spanned(
                        ty,
                        r"This Case produces the same input type signature as a previous one when applied to `fn run`'s input parameters — likely because the generic type parameters substituted by this Case do not appear in any input parameter, making it indistinguishable from another Case at dispatch time. This can also happen if the Case itself is duplicated.",
                    )
                    .to_compile_error(),
                );
            }
            continue;
        }

        let turbofish = if all_generics.is_empty() || case_args.is_empty() {
            quote! {}
        } else {
            quote! { ::<#(#case_args),*> }
        };

        arms.push(quote! {
            (#(#pattern_idents),*) => __dispatch #turbofish (&__host, #registry_expr, __inputs, __outputs),
        });
    }

    let const_decls: Vec<proc_macro2::TokenStream> = type_consts
        .iter()
        .map(|(ident, ty)| {
            quote! {
                const #ident: orc_sdk::OrcTypeId =
                    <#ty as orc_sdk::TOrcData>::TYPE_INFO.type_id;
            }
        })
        .collect();

    quote! {
        #(#errors)*
        #(#const_decls)*
        let __result = match #scrutinee {
            #(#arms)*
            _ => Err(orc_sdk::Error::DeckTypeMismatch),
        };
        if let Err(e) = __result {
            __host.error(&::std::format!("Failed to run: {e:?}"));
        }
    }
}

fn generate_dispatch_fn(
    run_fn: &syn::ItemFn,
    params: &ValidatedParams,
) -> proc_macro2::TokenStream {
    let run_generics = &run_fn.sig.generics;
    let where_clause = &run_fn.sig.generics.where_clause;
    let n_inputs = params.inputs.len();
    let n_outputs = params.outputs.len();
    // Per-input: slice ident, inner type, call-arg expression.
    let in_slice_idents: Vec<proc_macro2::Ident> = (0..n_inputs)
        .map(|i| format_ident!("__in_slice_{i}"))
        .collect();
    let input_item_slice_setup: Vec<proc_macro2::TokenStream> = params
        .inputs
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let ident = &in_slice_idents[i];
            let inner_ty = &p.inner_type;
            quote! {
                let #ident = unsafe {
                    orc_sdk::slice_from_ptr(__inputs[#i].items.cast::<#inner_ty>(), __inputs[#i].n_items as usize)
                };
            }
        })
        .collect();
    let in_call_args: Vec<proc_macro2::TokenStream> = params
        .inputs
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let ident = &in_slice_idents[i];
            match p.param.ty.as_ref() {
                syn::Type::Reference(r) if r.mutability.is_none() => match r.elem.as_ref() {
                    syn::Type::Slice(_) => quote! { __comb.get_input(#ident, #i).as_slice() },
                    _ => quote! { __comb.get_input(#ident, #i).as_ref() },
                },
                _ => quote! { __comb.get_input(#ident, #i) }, // DeckView<T>
            }
        })
        .collect();
    // Per-output: deck/view/item idents, inner type.
    let out_deck_idents: Vec<proc_macro2::Ident> = (0..n_outputs)
        .map(|j| format_ident!("__out_deck_{j}"))
        .collect();
    let out_view_idents: Vec<proc_macro2::Ident> = (0..n_outputs)
        .map(|j| format_ident!("__out_view_{j}"))
        .collect();
    let out_item_idents: Vec<proc_macro2::Ident> = (0..n_outputs)
        .map(|j| format_ident!("__out_item_{j}"))
        .collect();
    let ensure_output_allocations: Vec<proc_macro2::TokenStream> = params
        .outputs
        .iter()
        .enumerate()
        .map(|(j, p)| {
            let inner_ty = &p.inner_type;
            quote! {
                __registry.ensure_alloc_default::<orc_sdk::Deck<#inner_ty>>(&mut __outputs[#j].handle)?;
            }
        })
        .collect();
    let out_downcasts: Vec<proc_macro2::TokenStream> = params
        .outputs
        .iter()
        .enumerate()
        .map(|(j, p)| {
            let deck_ident = &out_deck_idents[j];
            let inner_ty = &p.inner_type;
            quote! {
                let #deck_ident: &mut orc_sdk::Deck<#inner_ty> = __out_decks[#j]
                    .downcast_mut()
                    .ok_or(orc_sdk::Error::DeckTypeMismatch)?;
            }
        })
        .collect();
    let out_view_setup: Vec<proc_macro2::TokenStream> = (0..n_outputs)
        .map(|j| {
            let view_ident = &out_view_idents[j];
            let item_ident = &out_item_idents[j];
            let deck_ident = &out_deck_idents[j];
            quote! {
                let mut #view_ident = __comb.get_output(#deck_ident, #j);
                let #item_ident = #view_ident.push_default_mut();
            }
        })
        .collect();
    let out_handle_refs: Vec<proc_macro2::TokenStream> = (0..n_outputs)
        .map(|j| quote! { __outputs[#j].handle })
        .collect();
    let out_handle_updates: Vec<proc_macro2::TokenStream> = (0..n_outputs)
        .map(|j| {
            let deck_ident = &out_deck_idents[j];
            let id_ident = format_ident!("__out_id_{j}");
            quote! {
                let #id_ident = __outputs[#j].handle;
                __outputs[#j] = orc_sdk::handle_from_deck(#deck_ident, #id_ident);
            }
        })
        .collect();
    let input_depths_vals: Vec<u8> = params.inputs.iter().map(|p| p.depth).collect();
    let output_depths_vals: Vec<u8> = params.outputs.iter().map(|p| p.depth).collect();
    quote! {
        fn __dispatch #run_generics (
            __host: &orc_sdk::HostCallbacks,
            __registry: &orc_sdk::ObjectRegistry,
            __inputs: &[orc_sdk::OrcHandle],
            __outputs: &mut [orc_sdk::OrcHandle],
        ) -> Result<(), orc_sdk::Error> #where_clause {
            const __INPUT_DEPTHS: &[u8] = &[#(#input_depths_vals),*];
            const __OUTPUT_DEPTHS: &[u8] = &[#(#output_depths_vals),*];
            let mut __comb = orc_sdk::Combinations::from_handles(__inputs, __INPUT_DEPTHS, __OUTPUT_DEPTHS)?;
            #(#ensure_output_allocations)*
            #(#input_item_slice_setup)*
            let __result = __registry.with_mut(
                &[#(#out_handle_refs),*],
                |__out_decks| -> Result<(), orc_sdk::Error> {
                    #(#out_downcasts)*
                    loop {
                        #(#out_view_setup)*
                        run(__host, #(#in_call_args,)* #(#out_item_idents),*)?;
                        if !__comb.advance() { break; }
                    }
                    #(#out_handle_updates)*
                    Ok(())
                },
            )
            .flatten();
            __result
        }
    }
}

struct FnConfig<'a> {
    name: &'a proc_macro2::Ident,
    docs: &'a str,
    run_fn: &'a syn::ItemFn,
    dims_fn: Option<&'a syn::ItemFn>,
    types: Option<&'a syn::Type>,
    registry_expr: Option<&'a syn::Expr>,
    host_callbacks_expr: &'a syn::Expr,
    user_items: &'a [proc_macro2::TokenStream],
    params: &'a ValidatedParams,
}

fn generate_orc_fn(cfg: FnConfig<'_>) -> proc_macro2::TokenStream {
    let FnConfig {
        name,
        docs,
        run_fn,
        dims_fn,
        types,
        registry_expr,
        host_callbacks_expr,
        user_items,
        params,
    } = cfg;
    let info_name = info_const_name(&name.to_string());
    let n_inputs = params.inputs.len();
    let n_outputs = params.outputs.len();
    let name_lit = proc_macro2::Literal::c_string(
        &std::ffi::CString::new(name.to_string()).expect("name contains null byte"),
    );
    let desc_lit = proc_macro2::Literal::c_string(
        &std::ffi::CString::new(docs).expect("docs contains null byte"),
    );
    let (dims_fn_tokens, dims_call) = match dims_fn {
        Some(d) => {
            let in_args: Vec<proc_macro2::TokenStream> = (0..n_inputs)
                .map(|i| quote! { &__inputs[#i].dims })
                .collect();
            let out_args: Vec<proc_macro2::TokenStream> = (0..n_outputs)
                .map(|j| quote! { &mut __outputs[#j].dims })
                .collect();
            (
                quote! { #d },
                quote! {
                    orc_sdk::orc_check_return!(
                        __host,
                        dims(#(#in_args,)* #(#out_args),*).is_ok(),
                        "dims computation failed"
                    );
                },
            )
        }
        None => (quote! {}, quote! {}),
    };
    let dispatch_fn = generate_dispatch_fn(run_fn, params);
    let registry_expr = registry_expr.map(|r| quote! { #r }).unwrap_or_default();
    let type_dispatch = generate_type_dispatch(run_fn, types, params, &registry_expr);
    quote! {
        pub const #info_name: orc_sdk::OrcFuncInfo = orc_sdk::OrcFuncInfo {
            name: #name_lit.as_ptr(),
            desc: #desc_lit.as_ptr(),
            func: Some(#name),
        };
        unsafe extern "C" fn #name(
            __ctx: u64,
            __inputs_ptr: *const orc_sdk::OrcHandle,
            __n_inputs: u64,
            __outputs_ptr: *mut orc_sdk::OrcHandle,
            __n_outputs: u64,
        ) {
            let __orc_hc_ref: &orc_sdk::OrcHostCallbackAPI = #host_callbacks_expr;
            let __host = orc_sdk::HostCallbacks {
                inner: *__orc_hc_ref,
                context: __ctx,
            };
            #(#user_items)*
            #run_fn
            #dims_fn_tokens
            // Check the number of inputs.
            orc_sdk::orc_check_return!(
                __host,
                __n_inputs == #n_inputs as u64,
                "Expected {} inputs, got {}",
                #n_inputs,
                __n_inputs
            );
            // Check the number of outputs.
            orc_sdk::orc_check_return!(
                __host,
                __n_outputs == #n_outputs as u64,
                "Expected {} outputs, got {}",
                #n_outputs,
                __n_outputs
            );
            let __inputs = unsafe { orc_sdk::slice_from_ptr(__inputs_ptr, #n_inputs) };
            let __outputs = unsafe { orc_sdk::slice_from_ptr_mut(__outputs_ptr, #n_outputs) };
            #dims_call
            #dispatch_fn
            #type_dispatch
        }
    }
}

struct FnMacroArgs {
    name: proc_macro2::Ident,
    stmts: Vec<syn::Stmt>,
}

impl syn::parse::Parse for FnMacroArgs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let name = input.parse()?;
        input.parse::<syn::Token![,]>()?;
        let content;
        syn::braced!(content in input);
        let mut stmts = Vec::new();
        while !content.is_empty() {
            stmts.push(content.parse()?);
        }
        Ok(FnMacroArgs { name, stmts })
    }
}

/// Expands `orc_fn!(name, { ... })` into a full FFI function + OrcFuncInfo const.
#[proc_macro]
pub fn orc_fn(input: TokenStream) -> TokenStream {
    let FnMacroArgs { name, stmts } = parse_macro_input!(input as FnMacroArgs);
    let mut docs = String::new();
    // fn run(<&HostCallbacks>, <inputs...>, <outputs...>) { ... }  — required.
    let mut run_fn: Option<syn::ItemFn> = None;
    // fn dims(<&OrcDims inputs...>, <&mut OrcDims outputs...>) { ... }  — optional.
    let mut dims_fn: Option<syn::ItemFn> = None;
    // const INPUT_DEPTHS: [u8; N] = [...];
    let mut input_depths: Option<syn::ExprArray> = None;
    // const OUTPUT_DEPTHS: [u8; N] = [...];
    let mut output_depths: Option<syn::ExprArray> = None;
    // type Types = (Case<f32>, Case<f64>, ...);  — optional, one case per monomorphization.
    let mut types: Option<syn::Type> = None;
    // let registry: &ObjectRegistry = &MY_REGISTRY;
    let mut registry: Option<syn::Expr> = None;
    // let host_callbacks = host_callbacks();
    let mut host_callbacks_expr: Option<syn::Expr> = None;
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
        let stmt_docs = docs_from_attrs(attrs);
        if !stmt_docs.is_empty() {
            if !docs.is_empty() {
                docs.push(' ');
            }
            docs.push_str(&stmt_docs);
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
                        return syn::Error::new_spanned(
                            &t.ty,
                            "Types must be a tuple of Case<...>",
                        )
                        .to_compile_error()
                        .into();
                    }
                };
                if outer.elems.is_empty() {
                    return syn::Error::new_spanned(&t.ty, "Types cannot be an empty tuple")
                        .to_compile_error()
                        .into();
                }
                // All elements must be Case<...> with the same number of args.
                let first_arity = match sig_arg_count(outer.elems.first().unwrap()) {
                    Some(n) => n,
                    None => {
                        return syn::Error::new_spanned(
                            outer.elems.first().unwrap(),
                            "each element of Types must be `Case<T, U, ...>`",
                        )
                        .to_compile_error()
                        .into();
                    }
                };
                for elem in outer.elems.iter().skip(1) {
                    match sig_arg_count(elem) {
                        Some(n) if n == first_arity => {}
                        Some(n) => {
                            return syn::Error::new_spanned(
                                elem,
                                format!("expected Case with {first_arity} argument(s), found {n}"),
                            )
                            .to_compile_error()
                            .into();
                        }
                        None => {
                            return syn::Error::new_spanned(
                                elem,
                                "each element of Types must be `Case<T, U, ...>`",
                            )
                            .to_compile_error()
                            .into();
                        }
                    }
                }
                types = Some(*t.ty.clone());
            }
            syn::Stmt::Item(syn::Item::Fn(f)) => match f.sig.ident.to_string().as_str() {
                "run" => {
                    let mut f = f.clone();
                    // Doc attrs were already collected into `docs`; strip them so they
                    // don't re-appear as #[doc = ...] on the emitted inner function.
                    f.attrs.retain(|a| !a.path().is_ident("doc"));
                    run_fn = Some(f);
                }
                "dims" => {
                    let mut f = f.clone();
                    f.attrs.retain(|a| !a.path().is_ident("doc"));
                    dims_fn = Some(f);
                }
                _ => user_items.push(quote::quote!(#f)),
            },
            syn::Stmt::Local(local) => {
                let mut recognized = false;
                let binding_ident = match &local.pat {
                    syn::Pat::Ident(pi) => Some(&pi.ident),
                    syn::Pat::Type(pt) => match pt.pat.as_ref() {
                        syn::Pat::Ident(pi) => Some(&pi.ident),
                        _ => None,
                    },
                    _ => None,
                };
                if let Some(ident) = binding_ident {
                    if ident == "registry" {
                        if let Some(init) = &local.init {
                            registry = Some(*init.expr.clone());
                            recognized = true;
                        }
                    } else if ident == "host_callbacks"
                        && let Some(init) = &local.init
                    {
                        host_callbacks_expr = Some(*init.expr.clone());
                        recognized = true;
                    }
                }
                if !recognized {
                    user_items.push(quote::quote!(#local));
                }
            }
            _ => user_items.push(quote::quote!(#stmt)),
        }
    }

    // host_callbacks is required.
    let host_callbacks_expr = match host_callbacks_expr {
        Some(e) => e,
        None => {
            return syn::Error::new(
                proc_macro2::Span::call_site(),
                "orc_fn! requires `let host_callbacks = <expr returning &OrcHostCallbackAPI>`",
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
        Err(e) => return e.to_compile_error().into(),
    };
    // Now that we validated everything, we can generate the code for this orc_fn.
    generate_orc_fn(FnConfig {
        name: &name,
        docs: &docs,
        run_fn: &run_fn,
        dims_fn: dims_fn.as_ref(),
        types: types.as_ref(),
        registry_expr: registry.as_ref(),
        host_callbacks_expr: &host_callbacks_expr,
        user_items: &user_items,
        params: &validated_params,
    })
    .into()
}
