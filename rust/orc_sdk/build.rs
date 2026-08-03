fn main() {
    let bindings = bindgen::Builder::default()
        .header("../../c/orc_abi.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .parse_callbacks(Box::new(TypeAliasCallback))
        .allowlist_type("Orc.*")
        .allowlist_var("ORC_.*")
        .allowlist_function("orc_.*")
        .derive_partialeq(true)
        .derive_eq(true)
        .derive_default(true)
        .no_partialeq("OrcFuncInfo") // This contains a function pointer and doesn't require comparison.
        .no_partialeq("OrcHost.*") // This contains function pointers, and doesn't require comparison.
        .no_partialeq("OrcHandle") // This contains function pointers, and doesn't require comparison.
        .no_copy("OrcHandle") // OrcHandle owns heap memory; implicit copies would alias it.
        .generate()
        .expect("Unable to generate bindings");
    // Write bindings to the output directory.
    let out_path = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    // println!(
    //     "cargo:warning=bindgen output: {}",
    //     out_path.join("bindings.rs").display()
    // );
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings");
}

#[derive(Debug)]
struct TypeAliasCallback;

impl bindgen::callbacks::ParseCallbacks for TypeAliasCallback {
    fn int_macro(&self, name: &str, _value: i64) -> Option<bindgen::callbacks::IntKind> {
        match name {
            n if n.starts_with("ORC_TYPE_") => Some(bindgen::callbacks::IntKind::U64),
            n if n.starts_with("ORC_MSG_LEVEL_") => Some(bindgen::callbacks::IntKind::U8),
            n if n.starts_with("ORC_DECK_PROXY_") => Some(bindgen::callbacks::IntKind::U8),
            n if n.starts_with("ORC_ERROR_") => Some(bindgen::callbacks::IntKind::U32),
            _ => None,
        }
    }
}
