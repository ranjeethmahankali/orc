fn main() {
    let bindings = bindgen::Builder::default()
        .header("../c/src/orc_ffi.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .allowlist_type("Orc.*")
        .allowlist_var("ORC_.*")
        .allowlist_function("orc_.*")
        .generate()
        .expect("Unable to generate bindings");
    // Write bindings to the output directory.
    bindings
        .write_to_file("src/bindings.rs")
        .expect("Couldn't write bindings");
}
