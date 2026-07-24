#![allow(dead_code)]

// When this project builds, the build.rs file will be run, which uses bindgen to consume orc_ffi.h
// to generate a bindings.rs file. That file will be written into the output directory where the
// build process writes the binaries to. Cargo exposes that as an environment variable. So we use
// that environment variable, and include the contents of that file in this file, effectively
// compiling the ABI compatible Rust bindings.
//
// LSP (Rust analyzer) seems to be working just fine with these bindings. Autocomplete, and even
// go-to-definition work too.

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
