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
