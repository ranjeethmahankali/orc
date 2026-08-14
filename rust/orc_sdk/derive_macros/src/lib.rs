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
    has_host: bool,
}

/// Parses `(run::<T1, T2>, run::<U1, U2>, ...)` into a list of generic arg lists,
/// one per monomorphization.
fn parse_types_expr(expr: &syn::Expr) -> syn::Result<Vec<Vec<syn::GenericArgument>>> {
    let tuple = match expr {
        syn::Expr::Tuple(t) => t,
        _ => {
            return Err(syn::Error::new_spanned(
                expr,
                "`let types` must be a tuple of `run::<...>` expressions",
            ));
        }
    };
    if tuple.elems.is_empty() {
        return Err(syn::Error::new_spanned(
            expr,
            "`let types` cannot be an empty tuple",
        ));
    }
    let mut result = Vec::new();
    for elem in &tuple.elems {
        let path_expr = match elem {
            syn::Expr::Path(p) => p,
            _ => {
                return Err(syn::Error::new_spanned(
                    elem,
                    "each element of `let types` must be a `run::<...>` expression",
                ));
            }
        };
        let seg = path_expr.path.segments.last().ok_or_else(|| {
            syn::Error::new_spanned(elem, "each element of `let types` must be `run::<...>`")
        })?;
        if seg.ident != "run" {
            return Err(syn::Error::new_spanned(
                &seg.ident,
                "each element of `let types` must call `run` (e.g. `run::<f32, f64>`)",
            ));
        }
        let args: Vec<syn::GenericArgument> = match &seg.arguments {
            syn::PathArguments::AngleBracketed(ab) => ab.args.iter().cloned().collect(),
            syn::PathArguments::None => vec![],
            _ => {
                return Err(syn::Error::new_spanned(
                    elem,
                    "each element of `let types` must be `run::<...>`",
                ));
            }
        };
        result.push(args);
    }
    Ok(result)
}

fn is_deck_type(ty: &syn::Type) -> bool {
    matches!(ty, syn::Type::Path(p)
        if p.path.segments.last().is_some_and(|s| s.ident == "DeckView" || s.ident == "DeckWriter"))
}

fn is_deck_writer_param(ty: &syn::Type) -> bool {
    matches!(ty, syn::Type::Reference(r)
        if r.mutability.is_some()
        && matches!(r.elem.as_ref(), syn::Type::Path(p)
            if p.path.segments.last().is_some_and(|s| s.ident == "DeckWriter")))
}

fn infer_depth(ty: &syn::Type) -> Option<u8> {
    match ty {
        syn::Type::Reference(r) => match r.elem.as_ref() {
            syn::Type::Slice(_) => Some(1),
            inner if is_deck_type(inner) => None,
            _ => Some(0),
        },
        _ if is_deck_type(ty) => None,
        _ => Some(0),
    }
}

fn is_output_param(ty: &syn::Type) -> syn::Result<bool> {
    match ty {
        syn::Type::Reference(r) if r.mutability.is_some() => Ok(true),
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
            "run parameter must be `&T`, `&[T]`, `&mut T`, `&mut DeckWriter<T>`, or `DeckView<T>`",
        )),
    }
}

/// Strips depth wrappers and returns the inner element type:
/// `&[T]` → `T`, `&T` → `T`, `&mut T` → `T`,
/// `&mut DeckWriter<T>` → `T`, `DeckView<T>` → `T`, anything else → itself.
fn inner_type(ty: &syn::Type) -> &syn::Type {
    match ty {
        syn::Type::Reference(r) => match r.elem.as_ref() {
            syn::Type::Slice(s) => s.elem.as_ref(),
            inner if is_deck_type(inner) => inner_type(inner),
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

fn is_host_param(ty: &syn::Type) -> bool {
    matches!(ty, syn::Type::Reference(r)
        if r.mutability.is_none()
        && matches!(r.elem.as_ref(), syn::Type::Path(p)
            if p.path.segments.last().is_some_and(|s| s.ident == "HostCallbacks")))
}

fn validate_orc_fn(
    run_fn: &syn::ItemFn,
    dims_fn: Option<&syn::ItemFn>,
    types: Option<&[Vec<syn::GenericArgument>]>,
    registry: Option<&syn::Expr>,
    input_depths: Option<&syn::ExprArray>,
    output_depths: Option<&syn::ExprArray>,
) -> syn::Result<ValidatedParams> {
    // Detect whether the first parameter is &HostCallbacks.
    let has_host = matches!(
        run_fn.sig.inputs.first(),
        Some(syn::FnArg::Typed(pt)) if is_host_param(pt.ty.as_ref())
    );
    // fn run must return Result<_, _> or nothing.
    if let syn::ReturnType::Type(_, ty) = &run_fn.sig.output
        && !matches!(ty.as_ref(), syn::Type::Path(p)
            if p.path.segments.last().is_some_and(|s| s.ident == "Result"))
    {
        return Err(syn::Error::new_spanned(
            &run_fn.sig,
            "fn run must return `Result<(), Error>` or nothing",
        ));
    }
    // Classify params (skip host if present).
    let mut input_params: Vec<syn::PatType> = Vec::new();
    let mut output_params: Vec<syn::PatType> = Vec::new();
    let mut saw_output = false;
    for arg in run_fn.sig.inputs.iter().skip(if has_host { 1 } else { 0 }) {
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
    // If run_fn has generics (type or const), `let types` must be defined with matching arity.
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
                    "fn run is generic; `let types = (run::<...>, ...)` must be provided",
                ));
            }
            Some(cases) => {
                for case_args in cases {
                    if case_args.len() != generic_count {
                        return Err(syn::Error::new_spanned(
                            &run_fn.sig,
                            format!(
                                "each entry in `let types` must have {generic_count} type argument(s) to match fn run's generics"
                            ),
                        ));
                    }
                }
            }
        }
    }
    // Outputs require a registry.
    if !output_params.is_empty() && registry.is_none() {
        return Err(syn::Error::new_spanned(
            &run_fn.sig,
            "fn run has output parameters; `let registry: &DeckRegistry = ...` must be defined",
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
        has_host,
    })
}

fn dims_returns_result(dims: &ItemFn) -> bool {
    match &dims.sig.output {
        syn::ReturnType::Type(_, ty) => matches!(
            ty.as_ref(),
            syn::Type::Path(p)
                if p.path.segments.last().is_some_and(|s| s.ident == "Result")
        ),
        syn::ReturnType::Default => false,
    }
}

fn validate_dims_fn(dims: &ItemFn, n_inputs: usize, n_outputs: usize) -> syn::Result<()> {
    // fn dims must return Result<_, _> or nothing.
    if let syn::ReturnType::Type(_, ty) = &dims.sig.output
        && !matches!(ty.as_ref(), syn::Type::Path(p)
            if p.path.segments.last().is_some_and(|s| s.ident == "Result"))
    {
        return Err(syn::Error::new_spanned(
            &dims.sig,
            "fn dims must return `Result<(), Error>` or nothing",
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

/// Returns true if `ty` is a bare identifier matching one of the generic type parameters.
fn is_generic_type(ty: &syn::Type, generics: &[&syn::GenericParam]) -> bool {
    if let syn::Type::Path(p) = ty
        && let Some(ident) = p.path.get_ident()
    {
        return generics
            .iter()
            .any(|gp| matches!(gp, syn::GenericParam::Type(tp) if tp.ident == *ident));
    }
    false
}

/// If `ty` is a bare ident matching a type generic, returns the substituted concrete type.
/// Otherwise returns a clone of `ty` unchanged.
fn substitute_type(
    ty: &syn::Type,
    generics: &[&syn::GenericParam],
    case_args: &[&syn::GenericArgument],
) -> syn::Type {
    if is_generic_type(ty, generics) {
        let ident = match ty {
            syn::Type::Path(p) => p.path.get_ident().unwrap(),
            _ => unreachable!(),
        };
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
    types: Option<&[Vec<syn::GenericArgument>]>,
    params: &ValidatedParams,
    registry_expr: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let n_inputs = params.inputs.len();
    let scrutinee_elems: Vec<proc_macro2::TokenStream> = (0..n_inputs)
        .map(|i| quote! { inputs_[#i].type_id })
        .collect();
    let scrutinee = quote! { (#(#scrutinee_elems),*) };

    let all_generics: Vec<&syn::GenericParam> = run_fn.sig.generics.params.iter().collect();

    // Non-generic / no types → one empty case. Otherwise one case per monomorphization.
    let owned_cases: Vec<Vec<syn::GenericArgument>> = match types {
        Some(cases) if !cases.is_empty() => cases.to_vec(),
        _ => vec![vec![]],
    };
    let cases: Vec<Vec<&syn::GenericArgument>> = owned_cases
        .iter()
        .map(|args| args.iter().collect())
        .collect();

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
        let ident = format_ident!("ORC_TYPE_ID_{mangled}_");
        type_to_const.insert(key, ident.clone());
        type_consts.push((ident.clone(), ty.clone()));
        ident
    };

    let mut seen_patterns: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut errors: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut arms: Vec<proc_macro2::TokenStream> = Vec::new();

    for case_args in &cases {
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
            errors.push(quote! {
                compile_error!("duplicate dispatch pattern in `let types`: this monomorphization produces the same input type signature as a previous entry");
            });
            continue;
        }

        let turbofish = if all_generics.is_empty() || case_args.is_empty() {
            quote! {}
        } else {
            quote! { ::<#(#case_args),*> }
        };

        arms.push(quote! {
            (#(#pattern_idents),*) => dispatch_ #turbofish (&host_, #registry_expr, inputs_, outputs_),
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
        let result_ = match #scrutinee {
            #(#arms)*
            _ => Err(orc_sdk::Error::DeckTypeMismatch),
        };
        if let Err(e) = result_ {
            host_.error(&::std::format!("Failed to run: {e:?}"));
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
        .map(|i| format_ident!("in_slice_{i}_"))
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
                    orc_sdk::slice_from_ptr(inputs_[#i].items.cast::<#inner_ty>(), inputs_[#i].n_items as usize)
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
                    syn::Type::Slice(_) => quote! { comb_.get_input(#ident, #i).as_slice() },
                    _ => quote! { comb_.get_input(#ident, #i).as_ref() },
                },
                _ => quote! { comb_.get_input(#ident, #i) }, // DeckView<T>
            }
        })
        .collect();
    // Per-output: deck/view/item idents, inner type.
    let out_deck_idents: Vec<proc_macro2::Ident> = (0..n_outputs)
        .map(|j| format_ident!("out_deck_{j}_"))
        .collect();
    let out_writer_idents: Vec<proc_macro2::Ident> = (0..n_outputs)
        .map(|j| format_ident!("out_writer_{j}_"))
        .collect();
    let out_item_idents: Vec<proc_macro2::Ident> = (0..n_outputs)
        .map(|j| format_ident!("out_item_{j}_"))
        .collect();
    let ensure_output_allocations: Vec<proc_macro2::TokenStream> = params
        .outputs
        .iter()
        .enumerate()
        .map(|(j, p)| {
            let inner_ty = &p.inner_type;
            quote! {
                registry_.alloc::<#inner_ty>(&mut outputs_[#j])?;
            }
        })
        .collect();
    // Destructure out_decks_ with a slice pattern so each element is an
    // independent &mut borrow. Sequential indexing (out_decks_[0], out_decks_[1])
    // would leave the whole slice mutably borrowed across statements, which the
    // borrow checker rejects when n_outputs > 1.
    let out_elem_idents: Vec<proc_macro2::Ident> = (0..n_outputs)
        .map(|j| format_ident!("out_decks_elem_{j}_"))
        .collect();
    let out_decks_destructure = {
        let elems = &out_elem_idents;
        quote! {
            let [#(#elems),*] = out_decks_ else {
                return Err(orc_sdk::Error::DeckTypeMismatch);
            };
        }
    };
    let out_downcasts: Vec<proc_macro2::TokenStream> = params
        .outputs
        .iter()
        .enumerate()
        .map(|(j, p)| {
            let deck_ident = &out_deck_idents[j];
            let elem_ident = &out_elem_idents[j];
            let inner_ty = &p.inner_type;
            quote! {
                let #deck_ident: &mut orc_sdk::Deck<#inner_ty> = #elem_ident
                    .downcast_mut()
                    .ok_or(orc_sdk::Error::DeckTypeMismatch)?;
            }
        })
        .collect();
    let out_writer_setup: Vec<proc_macro2::TokenStream> = (0..n_outputs)
        .map(|j| {
            let writer_ident = &out_writer_idents[j];
            let deck_ident = &out_deck_idents[j];
            if is_deck_writer_param(params.outputs[j].param.ty.as_ref()) {
                quote! {
                    let mut #writer_ident = comb_.get_output(#deck_ident, #j);
                }
            } else {
                let item_ident = &out_item_idents[j];
                quote! {
                    let mut #writer_ident = comb_.get_output(#deck_ident, #j);
                    let #item_ident = #writer_ident.push_default_mut();
                }
            }
        })
        .collect();
    let out_call_args: Vec<proc_macro2::TokenStream> = (0..n_outputs)
        .map(|j| {
            if is_deck_writer_param(params.outputs[j].param.ty.as_ref()) {
                let writer_ident = &out_writer_idents[j];
                quote! { &mut #writer_ident }
            } else {
                let item_ident = &out_item_idents[j];
                quote! { #item_ident }
            }
        })
        .collect();
    let out_handle_refs: Vec<proc_macro2::TokenStream> = (0..n_outputs)
        .map(|j| quote! { outputs_[#j].handle })
        .collect();
    let out_handle_updates: Vec<proc_macro2::TokenStream> = (0..n_outputs)
        .map(|j| {
            let deck_ident = &out_deck_idents[j];
            quote! {
                // update_handle_from_deck preserves handle.handle; set free_fn so the host
                // can call back into this plugin to free the data.
                unsafe { orc_sdk::update_handle_from_deck(#deck_ident, &mut outputs_[#j]) };
                outputs_[#j].free_fn = Some(crate::orc_deck_free);
            }
        })
        .collect();
    let input_depths_vals: Vec<u8> = params.inputs.iter().map(|p| p.depth).collect();
    let output_depths_vals: Vec<u8> = params.outputs.iter().map(|p| p.depth).collect();
    let run_returns_result = matches!(
        &run_fn.sig.output,
        syn::ReturnType::Type(_, ty) if matches!(ty.as_ref(), syn::Type::Path(p)
            if p.path.segments.last().is_some_and(|s| s.ident == "Result"))
    );
    let host_arg = if params.has_host {
        quote! { host_, }
    } else {
        quote! {}
    };
    let run_turbofish = {
        let args: Vec<proc_macro2::TokenStream> = run_fn
            .sig
            .generics
            .params
            .iter()
            .map(|p| match p {
                syn::GenericParam::Type(tp) => {
                    let ident = &tp.ident;
                    quote! { #ident }
                }
                syn::GenericParam::Const(cp) => {
                    let ident = &cp.ident;
                    quote! { #ident }
                }
                syn::GenericParam::Lifetime(lp) => {
                    let lifetime = &lp.lifetime;
                    quote! { #lifetime }
                }
            })
            .collect();
        if args.is_empty() {
            quote! {}
        } else {
            quote! { ::<#(#args),*> }
        }
    };
    let run_call = if run_returns_result {
        quote! { run #run_turbofish (#host_arg #(#in_call_args,)* #(#out_call_args),*)?; }
    } else {
        quote! { run #run_turbofish (#host_arg #(#in_call_args,)* #(#out_call_args),*); }
    };
    quote! {
        fn dispatch_ #run_generics (
            host_: &orc_sdk::HostCallbacks,
            registry_: &orc_sdk::DeckRegistry,
            inputs_: &[orc_sdk::OrcHandle],
            outputs_: &mut [orc_sdk::OrcHandle],
        ) -> Result<(), orc_sdk::Error> #where_clause {
            const INPUT_DEPTHS_: &[u8] = &[#(#input_depths_vals),*];
            const OUTPUT_DEPTHS_: &[u8] = &[#(#output_depths_vals),*];
            let mut comb_ = orc_sdk::Combinations::from_handles(inputs_, INPUT_DEPTHS_, OUTPUT_DEPTHS_)?;
            #(#ensure_output_allocations)*
            #(#input_item_slice_setup)*
            let result_ = registry_.with_mut(
                &[#(#out_handle_refs),*],
                |out_decks_| -> Result<(), orc_sdk::Error> {
                    #out_decks_destructure
                    #(#out_downcasts)*
                    loop {
                        #(#out_writer_setup)*
                        #run_call
                        if !comb_.advance() { break; }
                    }
                    #(#out_handle_updates)*
                    Ok(())
                },
            )
            .flatten();
            result_
        }
    }
}

struct FnConfig<'a> {
    name: &'a proc_macro2::Ident,
    docs: &'a str,
    run_fn: &'a syn::ItemFn,
    dims_fn: Option<&'a syn::ItemFn>,
    types: Option<&'a [Vec<syn::GenericArgument>]>,
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
                .map(|i| quote! { &inputs_[#i].dims })
                .collect();
            let out_args: Vec<proc_macro2::TokenStream> = (0..n_outputs)
                .map(|j| quote! { &mut outputs_[#j].dims })
                .collect();
            let call = if dims_returns_result(d) {
                quote! {
                    orc_sdk::orc_check_return!(
                        host_,
                        dims(#(#in_args,)* #(#out_args),*).is_ok(),
                        "dims computation failed"
                    );
                }
            } else {
                quote! { dims(#(#in_args,)* #(#out_args),*); }
            };
            (quote! { #d }, call)
        }
        None => (quote! {}, quote! {}),
    };
    let dispatch_fn = generate_dispatch_fn(run_fn, params);
    let registry_expr = registry_expr.map(|r| quote! { #r }).unwrap_or_default();
    let type_dispatch = generate_type_dispatch(run_fn, types, params, &registry_expr);
    let all_generics: Vec<&syn::GenericParam> = run_fn.sig.generics.params.iter().collect();
    let fn_type_id_expr = |p: &ParamInfo| -> proc_macro2::TokenStream {
        if is_generic_type(&p.inner_type, &all_generics) {
            quote! { orc_sdk::ORC_TYPE_ANY }
        } else {
            let ty = &p.inner_type;
            quote! { <#ty as orc_sdk::TOrcData>::TYPE_INFO.type_id }
        }
    };
    let input_types_ptr = if n_inputs == 0
        || params
            .inputs
            .iter()
            .all(|p| is_generic_type(&p.inner_type, &all_generics))
    {
        quote! { ::std::ptr::null_mut() }
    } else {
        let exprs: Vec<_> = params.inputs.iter().map(&fn_type_id_expr).collect();
        quote! {
            (&[#(#exprs),*] as *const [orc_sdk::OrcTypeId; #n_inputs])
                .cast::<orc_sdk::OrcTypeId>()
                .cast_mut()
        }
    };
    let output_types_ptr = if n_outputs == 0
        || params
            .outputs
            .iter()
            .all(|p| is_generic_type(&p.inner_type, &all_generics))
    {
        quote! { ::std::ptr::null_mut() }
    } else {
        let exprs: Vec<_> = params.outputs.iter().map(&fn_type_id_expr).collect();
        quote! {
            (&[#(#exprs),*] as *const [orc_sdk::OrcTypeId; #n_outputs])
                .cast::<orc_sdk::OrcTypeId>()
                .cast_mut()
        }
    };
    quote! {
        pub const #info_name: orc_sdk::OrcFuncInfo = orc_sdk::OrcFuncInfo {
            name: #name_lit.as_ptr(),
            desc: #desc_lit.as_ptr(),
            n_inputs: #n_inputs as u64,
            n_outputs: #n_outputs as u64,
            input_types: #input_types_ptr,
            output_types: #output_types_ptr,
            func: Some(#name),
        };
        unsafe extern "C" fn #name(
            ctx_: u64,
            inputs_ptr_: *const orc_sdk::OrcHandle,
            n_inputs_: u64,
            outputs_ptr_: *mut orc_sdk::OrcHandle,
            n_outputs_: u64,
        ) {
            let orc_hc_ref_: &orc_sdk::OrcHostCallbackAPI = #host_callbacks_expr;
            let host_ = orc_sdk::HostCallbacks {
                inner: *orc_hc_ref_,
                context: ctx_,
            };
            #(#user_items)*
            #run_fn
            #dims_fn_tokens
            // Check the number of inputs.
            orc_sdk::orc_check_return!(
                host_,
                n_inputs_ == #n_inputs as u64,
                "Expected {} inputs, got {}",
                #n_inputs,
                n_inputs_
            );
            // Check the number of outputs.
            orc_sdk::orc_check_return!(
                host_,
                n_outputs_ == #n_outputs as u64,
                "Expected {} outputs, got {}",
                #n_outputs,
                n_outputs_
            );
            let inputs_ = unsafe { orc_sdk::slice_from_ptr(inputs_ptr_, #n_inputs) };
            let outputs_ = unsafe { orc_sdk::slice_from_ptr_mut(outputs_ptr_, #n_outputs) };
            #dims_call
            #dispatch_fn
            #type_dispatch
        }
    }
}

fn parse_fn_body(item: &syn::ItemFn, has_input_output_depths: bool) -> syn::Result<ParsedBody> {
    let mut docs = docs_from_attrs(&item.attrs);
    let mut run_fn: Option<syn::ItemFn> = None;
    let mut dims_fn: Option<syn::ItemFn> = None;
    let mut input_depths: Option<syn::ExprArray> = None;
    let mut output_depths: Option<syn::ExprArray> = None;
    let mut types: Option<Vec<Vec<syn::GenericArgument>>> = None;
    let mut registry: Option<syn::Expr> = None;
    let mut host_callbacks_expr: Option<syn::Expr> = None;
    let mut user_items: Vec<proc_macro2::TokenStream> = Vec::new();
    for stmt in &item.block.stmts {
        let attrs: &[syn::Attribute] = match stmt {
            syn::Stmt::Item(item) => match item {
                syn::Item::Const(c) => &c.attrs,
                syn::Item::Fn(f) => &f.attrs,
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
            syn::Stmt::Item(syn::Item::Const(c))
                if has_input_output_depths
                    && matches!(
                        c.ident.to_string().as_str(),
                        "INPUT_DEPTHS" | "OUTPUT_DEPTHS"
                    ) =>
            {
                let is_u8_array = matches!(c.ty.as_ref(),
                    syn::Type::Array(syn::TypeArray { elem, .. })
                    if matches!(elem.as_ref(), syn::Type::Path(p) if p.path.is_ident("u8"))
                );
                if !is_u8_array {
                    return Err(syn::Error::new_spanned(
                        &c.ty,
                        "INPUT_DEPTHS and OUTPUT_DEPTHS must be of type [u8; N]",
                    ));
                }
                if let syn::Expr::Array(arr) = c.expr.as_ref() {
                    if c.ident == "INPUT_DEPTHS" {
                        input_depths = Some(arr.clone());
                    } else {
                        output_depths = Some(arr.clone());
                    }
                }
            }
            syn::Stmt::Item(syn::Item::Const(c)) => user_items.push(quote::quote!(#c)),
            syn::Stmt::Item(syn::Item::Fn(f)) => match f.sig.ident.to_string().as_str() {
                "run" => {
                    let mut f = f.clone();
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
                    } else if ident == "host_callbacks" {
                        if let Some(init) = &local.init {
                            host_callbacks_expr = Some(*init.expr.clone());
                            recognized = true;
                        }
                    } else if ident == "types"
                        && let Some(init) = &local.init
                    {
                        types = Some(parse_types_expr(init.expr.as_ref())?);
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
    Ok(ParsedBody {
        docs,
        run_fn,
        dims_fn,
        input_depths,
        output_depths,
        types,
        registry,
        host_callbacks_expr,
        user_items,
    })
}

struct ParsedBody {
    docs: String,
    run_fn: Option<syn::ItemFn>,
    dims_fn: Option<syn::ItemFn>,
    input_depths: Option<syn::ExprArray>,
    output_depths: Option<syn::ExprArray>,
    types: Option<Vec<Vec<syn::GenericArgument>>>,
    registry: Option<syn::Expr>,
    host_callbacks_expr: Option<syn::Expr>,
    user_items: Vec<proc_macro2::TokenStream>,
}

/// Expands `#[orc_fn] fn name() { ... }` into a full FFI function + OrcFuncInfo const.
#[proc_macro_attribute]
pub fn orc_fn(_attrs: TokenStream, input: TokenStream) -> TokenStream {
    let item = parse_macro_input!(input as syn::ItemFn);
    let name = item.sig.ident.clone();
    let body = match parse_fn_body(&item, true) {
        Ok(b) => b,
        Err(e) => return e.to_compile_error().into(),
    };
    let ParsedBody {
        docs,
        run_fn,
        dims_fn,
        input_depths,
        output_depths,
        types,
        registry,
        host_callbacks_expr,
        user_items,
    } = body;
    let host_callbacks_expr = match host_callbacks_expr {
        Some(e) => e,
        None => {
            return syn::Error::new_spanned(
                &item.sig,
                "#[orc_fn] requires `let host_callbacks = <expr returning &OrcHostCallbackAPI>`",
            )
            .to_compile_error()
            .into();
        }
    };
    let run_fn = match run_fn {
        Some(f) => f,
        None => {
            return syn::Error::new_spanned(&item.sig, "#[orc_fn] requires a `fn run(...)` body")
                .to_compile_error()
                .into();
        }
    };
    let validated_params = match validate_orc_fn(
        &run_fn,
        dims_fn.as_ref(),
        types.as_deref(),
        registry.as_ref(),
        input_depths.as_ref(),
        output_depths.as_ref(),
    ) {
        Ok(v) => v,
        Err(e) => return e.to_compile_error().into(),
    };
    generate_orc_fn(FnConfig {
        name: &name,
        docs: &docs,
        run_fn: &run_fn,
        dims_fn: dims_fn.as_ref(),
        types: types.as_deref(),
        registry_expr: registry.as_ref(),
        host_callbacks_expr: &host_callbacks_expr,
        user_items: &user_items,
        params: &validated_params,
    })
    .into()
}

fn validate_orc_map_fn(
    run_fn: &syn::ItemFn,
    dims_fn: Option<&syn::ItemFn>,
    types: Option<&[Vec<syn::GenericArgument>]>,
    registry: Option<&syn::Expr>,
) -> syn::Result<ValidatedParams> {
    let has_host = matches!(
        run_fn.sig.inputs.first(),
        Some(syn::FnArg::Typed(pt)) if is_host_param(pt.ty.as_ref())
    );
    // fn run must return Result<_, _> or nothing.
    if let syn::ReturnType::Type(_, ty) = &run_fn.sig.output
        && !matches!(ty.as_ref(), syn::Type::Path(p)
            if p.path.segments.last().is_some_and(|s| s.ident == "Result"))
    {
        return Err(syn::Error::new_spanned(
            &run_fn.sig,
            "fn run must return `Result<(), Error>` or nothing",
        ));
    }
    let mut input_param: Option<syn::PatType> = None;
    let mut output_param: Option<syn::PatType> = None;
    for arg in run_fn.sig.inputs.iter().skip(if has_host { 1 } else { 0 }) {
        let pat_ty = match arg {
            syn::FnArg::Typed(pt) => pt,
            syn::FnArg::Receiver(r) => {
                return Err(syn::Error::new_spanned(
                    r,
                    "`run` must not have a self parameter",
                ));
            }
        };
        let is_out = is_output_param(pat_ty.ty.as_ref())?;
        if is_out {
            if output_param.is_some() {
                return Err(syn::Error::new_spanned(
                    pat_ty,
                    "orc_map_fn! run must have exactly one output parameter",
                ));
            }
            if is_deck_writer_param(pat_ty.ty.as_ref()) {
                return Err(syn::Error::new_spanned(
                    pat_ty,
                    "orc_map_fn! run output must be `&mut T`, not `&mut DeckWriter<T>`",
                ));
            }
            output_param = Some(pat_ty.clone());
        } else {
            if input_param.is_some() {
                return Err(syn::Error::new_spanned(
                    pat_ty,
                    "orc_map_fn! run must have exactly one input parameter",
                ));
            }
            if is_deck_type(pat_ty.ty.as_ref()) {
                return Err(syn::Error::new_spanned(
                    pat_ty,
                    "orc_map_fn! run input must be `&T`, not `DeckView<T>`",
                ));
            }
            if let syn::Type::Reference(r) = pat_ty.ty.as_ref()
                && matches!(r.elem.as_ref(), syn::Type::Slice(_))
            {
                return Err(syn::Error::new_spanned(
                    pat_ty,
                    "orc_map_fn! run input must be `&T`, not `&[T]`",
                ));
            }
            input_param = Some(pat_ty.clone());
        }
    }
    let input_param = input_param.ok_or_else(|| {
        syn::Error::new_spanned(
            &run_fn.sig,
            "orc_map_fn! run must have exactly one input parameter",
        )
    })?;
    let output_param = output_param.ok_or_else(|| {
        syn::Error::new_spanned(
            &run_fn.sig,
            "orc_map_fn! run must have exactly one output parameter",
        )
    })?;
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
                    "fn run is generic; `let types = (run::<...>, ...)` must be provided",
                ));
            }
            Some(cases) => {
                for case_args in cases {
                    if case_args.len() != generic_count {
                        return Err(syn::Error::new_spanned(
                            &run_fn.sig,
                            format!(
                                "each entry in `let types` must have {generic_count} type argument(s) to match fn run's generics"
                            ),
                        ));
                    }
                }
            }
        }
    }
    if registry.is_none() {
        return Err(syn::Error::new_spanned(
            &run_fn.sig,
            "orc_map_fn requires `let registry: &DeckRegistry = ...`",
        ));
    }
    if let Some(dims) = dims_fn {
        validate_dims_fn(dims, 1, 1)?;
    }
    let in_inner = inner_type(input_param.ty.as_ref()).clone();
    let out_inner = inner_type(output_param.ty.as_ref()).clone();
    Ok(ValidatedParams {
        inputs: Box::new([ParamInfo {
            param: input_param,
            depth: 1,
            inner_type: in_inner,
        }]),
        outputs: Box::new([ParamInfo {
            param: output_param,
            depth: 1,
            inner_type: out_inner,
        }]),
        has_host,
    })
}

fn generate_map_dispatch_fn(
    run_fn: &syn::ItemFn,
    params: &ValidatedParams,
) -> proc_macro2::TokenStream {
    let run_generics = &run_fn.sig.generics;
    let where_clause = &run_fn.sig.generics.where_clause;
    let in_inner_ty = &params.inputs[0].inner_type;
    let out_inner_ty = &params.outputs[0].inner_type;
    let run_returns_result = matches!(
        &run_fn.sig.output,
        syn::ReturnType::Type(_, ty) if matches!(ty.as_ref(), syn::Type::Path(p)
            if p.path.segments.last().is_some_and(|s| s.ident == "Result"))
    );
    let host_arg = if params.has_host {
        quote! { host_, }
    } else {
        quote! {}
    };
    let run_call = if run_returns_result {
        quote! { run(#host_arg in_item_, out_item_)?; }
    } else {
        quote! { run(#host_arg in_item_, out_item_); }
    };
    quote! {
        fn dispatch_ #run_generics (
            host_: &orc_sdk::HostCallbacks,
            registry_: &orc_sdk::DeckRegistry,
            inputs_: &[orc_sdk::OrcHandle],
            outputs_: &mut [orc_sdk::OrcHandle],
        ) -> Result<(), orc_sdk::Error> #where_clause {
            let in_items_ = unsafe {
                orc_sdk::slice_from_ptr(
                    inputs_[0].items.cast::<#in_inner_ty>(),
                    inputs_[0].n_items as usize,
                )
            };
            let in_marks_ = unsafe {
                orc_sdk::slice_from_ptr(inputs_[0].marks, inputs_[0].n_marks as usize)
            };
            registry_.alloc::<#out_inner_ty>(&mut outputs_[0])?;
            let result_ = registry_.with_mut(
                &[outputs_[0].handle],
                |out_decks_| -> Result<(), orc_sdk::Error> {
                    let [out_elem_] = out_decks_ else {
                        return Err(orc_sdk::Error::DeckTypeMismatch);
                    };
                    let out_deck_: &mut orc_sdk::Deck<#out_inner_ty> = out_elem_
                        .downcast_mut()
                        .ok_or(orc_sdk::Error::DeckTypeMismatch)?;
                    out_deck_.reshape_with_marks(in_items_.len(), in_marks_);
                    for (in_item_, out_item_) in
                        in_items_.iter().zip(out_deck_.items_mut().iter_mut())
                    {
                        #run_call
                    }
                    unsafe { orc_sdk::update_handle_from_deck(out_deck_, &mut outputs_[0]) };
                    outputs_[0].free_fn = Some(crate::orc_deck_free);
                    Ok(())
                },
            )
            .flatten();
            result_
        }
    }
}

fn generate_orc_map_fn(cfg: FnConfig<'_>) -> proc_macro2::TokenStream {
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
    let name_lit = proc_macro2::Literal::c_string(
        &std::ffi::CString::new(name.to_string()).expect("name contains null byte"),
    );
    let desc_lit = proc_macro2::Literal::c_string(
        &std::ffi::CString::new(docs).expect("docs contains null byte"),
    );
    let dispatch_fn = generate_map_dispatch_fn(run_fn, params);
    let registry_expr_ts = registry_expr.map(|r| quote! { #r }).unwrap_or_default();
    let type_dispatch = generate_type_dispatch(run_fn, types, params, &registry_expr_ts);
    let (dims_fn_tokens, dims_call) = match dims_fn {
        Some(d) => {
            let call = if dims_returns_result(d) {
                quote! {
                    orc_sdk::orc_check_return!(
                        host_,
                        dims(&inputs_[0].dims, &mut outputs_[0].dims).is_ok(),
                        "dims computation failed"
                    );
                }
            } else {
                quote! { dims(&inputs_[0].dims, &mut outputs_[0].dims); }
            };
            (quote! { #d }, call)
        }
        None => (quote! {}, quote! {}),
    };
    let all_generics: Vec<&syn::GenericParam> = run_fn.sig.generics.params.iter().collect();
    let in_ty = &params.inputs[0].inner_type;
    let out_ty = &params.outputs[0].inner_type;
    let input_types_ptr = if is_generic_type(in_ty, &all_generics) {
        quote! { ::std::ptr::null_mut() }
    } else {
        quote! {
            (&[<#in_ty as orc_sdk::TOrcData>::TYPE_INFO.type_id] as *const [orc_sdk::OrcTypeId; 1])
                .cast::<orc_sdk::OrcTypeId>()
                .cast_mut()
        }
    };
    let output_types_ptr = if is_generic_type(out_ty, &all_generics) {
        quote! { ::std::ptr::null_mut() }
    } else {
        quote! {
            (&[<#out_ty as orc_sdk::TOrcData>::TYPE_INFO.type_id] as *const [orc_sdk::OrcTypeId; 1])
                .cast::<orc_sdk::OrcTypeId>()
                .cast_mut()
        }
    };
    quote! {
        pub const #info_name: orc_sdk::OrcFuncInfo = orc_sdk::OrcFuncInfo {
            name: #name_lit.as_ptr(),
            desc: #desc_lit.as_ptr(),
            n_inputs: 1u64,
            n_outputs: 1u64,
            input_types: #input_types_ptr,
            output_types: #output_types_ptr,
            func: Some(#name),
        };
        unsafe extern "C" fn #name(
            ctx_: u64,
            inputs_ptr_: *const orc_sdk::OrcHandle,
            n_inputs_: u64,
            outputs_ptr_: *mut orc_sdk::OrcHandle,
            n_outputs_: u64,
        ) {
            let orc_hc_ref_: &orc_sdk::OrcHostCallbackAPI = #host_callbacks_expr;
            let host_ = orc_sdk::HostCallbacks {
                inner: *orc_hc_ref_,
                context: ctx_,
            };
            #(#user_items)*
            #run_fn
            #dims_fn_tokens
            orc_sdk::orc_check_return!(
                host_,
                n_inputs_ == 1u64,
                "Expected 1 input, got {}",
                n_inputs_
            );
            orc_sdk::orc_check_return!(
                host_,
                n_outputs_ == 1u64,
                "Expected 1 output, got {}",
                n_outputs_
            );
            let inputs_ = unsafe { orc_sdk::slice_from_ptr(inputs_ptr_, 1) };
            let outputs_ = unsafe { orc_sdk::slice_from_ptr_mut(outputs_ptr_, 1) };
            #dims_call
            #dispatch_fn
            #type_dispatch
        }
    }
}

/// Expands `#[orc_map_fn] fn name() { ... }` into a full FFI function + OrcFuncInfo const,
/// specialized for functions with exactly one input and one output deck.
/// Unlike `#[orc_fn]`, this avoids the overhead of `Combinations` and instead
/// reshapes the output to match the input structure, then maps items pairwise.
#[proc_macro_attribute]
pub fn orc_map_fn(_attrs: TokenStream, input: TokenStream) -> TokenStream {
    let item = parse_macro_input!(input as syn::ItemFn);
    let name = item.sig.ident.clone();
    let body = match parse_fn_body(&item, false) {
        Ok(b) => b,
        Err(e) => return e.to_compile_error().into(),
    };
    let ParsedBody {
        docs,
        run_fn,
        dims_fn,
        types,
        registry,
        host_callbacks_expr,
        user_items,
        ..
    } = body;
    let host_callbacks_expr = match host_callbacks_expr {
        Some(e) => e,
        None => {
            return syn::Error::new_spanned(
                &item.sig,
                "#[orc_map_fn] requires `let host_callbacks = <expr returning &OrcHostCallbackAPI>`",
            )
            .to_compile_error()
            .into();
        }
    };
    let run_fn = match run_fn {
        Some(f) => f,
        None => {
            return syn::Error::new_spanned(
                &item.sig,
                "#[orc_map_fn] requires a `fn run(...)` body",
            )
            .to_compile_error()
            .into();
        }
    };
    let validated_params = match validate_orc_map_fn(
        &run_fn,
        dims_fn.as_ref(),
        types.as_deref(),
        registry.as_ref(),
    ) {
        Ok(v) => v,
        Err(e) => return e.to_compile_error().into(),
    };
    generate_orc_map_fn(FnConfig {
        name: &name,
        docs: &docs,
        run_fn: &run_fn,
        dims_fn: dims_fn.as_ref(),
        types: types.as_deref(),
        registry_expr: registry.as_ref(),
        host_callbacks_expr: &host_callbacks_expr,
        user_items: &user_items,
        params: &validated_params,
    })
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    fn test_infer_depth() {
        // Input: &T → depth 0
        assert_eq!(infer_depth(&parse_quote! { &T }), Some(0));
        // Input: &[T] → depth 1
        assert_eq!(infer_depth(&parse_quote! { &[T] }), Some(1));
        // Input: DeckView<T> → cannot infer
        assert_eq!(infer_depth(&parse_quote! { DeckView<T> }), None);
        // Output: &mut T → depth 0
        assert_eq!(infer_depth(&parse_quote! { &mut T }), Some(0));
        // Output: &mut DeckWriter<T> → cannot infer
        assert_eq!(infer_depth(&parse_quote! { &mut DeckWriter<T> }), None);
    }

    #[test]
    fn test_is_output_param() {
        // &T → input
        assert!(!is_output_param(&parse_quote! { &T }).unwrap());
        // &[T] → input
        assert!(!is_output_param(&parse_quote! { &[T] }).unwrap());
        // DeckView<T> → input
        assert!(!is_output_param(&parse_quote! { DeckView<T> }).unwrap());
        // &mut T → output
        assert!(is_output_param(&parse_quote! { &mut T }).unwrap());
        // &mut DeckWriter<T> → output
        assert!(is_output_param(&parse_quote! { &mut DeckWriter<T> }).unwrap());
        // bare DeckWriter<T> → error
        assert!(is_output_param(&parse_quote! { DeckWriter<T> }).is_err());
        // bare T → error
        assert!(is_output_param(&parse_quote! { T }).is_err());
    }
}
