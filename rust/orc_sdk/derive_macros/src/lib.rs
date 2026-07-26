use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{ItemFn, parse::Parse, parse::ParseStream, parse_macro_input};

fn info_const_name(fn_name: &str) -> syn::Ident {
    format_ident!("ORC_FN_INFO_{}", fn_name.to_uppercase())
}

/// Attribute macro that generates an `OrcFuncInfo` const alongside the function.
///
/// The const is named `ORC_FN_INFO_{FUNCTION_NAME_UPPER}`.
/// The description is extracted from the function's doc comments.
///
/// # Example
///
/// ```ignore
/// #[orc_fn]
/// /// Adds the inputs together.
/// unsafe extern "C" fn plugin_fn_add(
///     ctx: u64,
///     inputs: *const OrcHandle,
///     n_inputs: usize,
///     outputs: *mut OrcHandle,
///     n_outputs: usize,
/// ) {
///     // ...
/// }
/// ```
///
/// Expands to the original function plus:
/// ```ignore
/// const ORC_FN_INFO_PLUGIN_FN_ADD: OrcFuncInfo = OrcFuncInfo {
///     name: c"plugin_fn_add".as_ptr(),
///     desc: c"Adds the inputs together.".as_ptr(),
///     func: Some(plugin_fn_add),
/// };
/// ```
#[proc_macro_attribute]
pub fn orc_fn(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let func = parse_macro_input!(item as ItemFn);
    let fn_name = &func.sig.ident;
    let fn_name_str = fn_name.to_string();
    let const_name = info_const_name(&fn_name_str);
    // Extract doc comments from attributes.
    let doc_lines: Vec<String> = func
        .attrs
        .iter()
        .filter_map(|attr| {
            if attr.path().is_ident("doc") {
                if let syn::Meta::NameValue(nv) = &attr.meta {
                    if let syn::Expr::Lit(expr_lit) = &nv.value {
                        if let syn::Lit::Str(s) = &expr_lit.lit {
                            return Some(s.value());
                        }
                    }
                }
            }
            None
        })
        .collect();
    let desc = doc_lines
        .iter()
        .map(|l| l.trim())
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string();
    // Build C string literals. We use byte-string concat at the call site.
    let name_cstr = format!("{}\0", fn_name_str);
    let desc_cstr = format!("{}\0", desc);
    let name_bytes = proc_macro2::Literal::byte_string(name_cstr.as_bytes());
    let desc_bytes = proc_macro2::Literal::byte_string(desc_cstr.as_bytes());
    let expanded = quote! {
        #func
        const #const_name: orc_sdk::OrcFuncInfo = orc_sdk::OrcFuncInfo {
            name: #name_bytes.as_ptr().cast(),
            desc: #desc_bytes.as_ptr().cast(),
            func: Some(#fn_name),
        };
    };
    expanded.into()
}

struct FnInfoInput {
    ident: syn::Ident,
}

impl Parse for FnInfoInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let ident: syn::Ident = input.parse()?;
        Ok(FnInfoInput { ident })
    }
}

/// Function-like macro that expands to the generated const name for a given function.
///
/// # Example
///
/// ```ignore
/// const ORC_EXPORTED_FUNCTIONS: &[OrcFuncInfo] = &[
///     orc_fn_info!(plugin_fn_add),
///     orc_fn_info!(plugin_fn_mul),
/// ];
/// ```
///
/// Expands each invocation to `ORC_FN_INFO_PLUGIN_FN_ADD`, `ORC_FN_INFO_PLUGIN_FN_MUL`, etc.
#[proc_macro]
pub fn orc_fn_info(input: TokenStream) -> TokenStream {
    let FnInfoInput { ident } = parse_macro_input!(input as FnInfoInput);
    let const_name = info_const_name(&ident.to_string());
    quote! { #const_name }.into()
}
