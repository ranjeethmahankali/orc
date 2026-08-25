fn main() {
    let args: Vec<String> = std::env::args().collect();
    let header = match args.get(1) {
        Some(h) if !h.starts_with("--") => h.clone(),
        _ => {
            eprintln!(
                "Usage: bindings_generator <header.h> [--rust-output <path>] [--rust-check <path>]"
            );
            std::process::exit(1);
        }
    };
    let mut rust_output: Option<String> = None;
    let mut rust_check: Option<String> = None;
    let mut i = 2;
    while i < args.len() {
        let next = || {
            args.get(i + 1).cloned().unwrap_or_else(|| {
                eprintln!("Missing argument after {}", args[i]);
                std::process::exit(1);
            })
        };
        match args[i].as_str() {
            "--rust-output" => {
                rust_output = Some(next());
                i += 2;
            }
            "--rust-check" => {
                rust_check = Some(next());
                i += 2;
            }
            other => {
                eprintln!("Unknown argument: {other}");
                std::process::exit(1);
            }
        }
    }
    if rust_output.is_none() && rust_check.is_none() {
        eprintln!("At least one of --rust-output, --rust-check must be provided");
        std::process::exit(1);
    }
    let bindings = generate_rust_bindings(&header);
    if let Some(path) = &rust_output {
        bindings
            .write_to_file(path)
            .expect("Couldn't write Rust bindings");
        println!("Rust bindings written to {path}");
    }
    if let Some(path) = &rust_check {
        let mut buf = Vec::new();
        bindings
            .write(Box::new(&mut buf))
            .expect("Couldn't serialize Rust bindings");
        let generated = String::from_utf8(buf).expect("Rust bindings are not valid UTF-8");
        let existing = std::fs::read_to_string(path).unwrap_or_else(|e| {
            eprintln!("Couldn't read {path}: {e}");
            std::process::exit(1);
        });
        let existing = existing.replace("\r\n", "\n");
        let mut has_diff = false;
        for diff in diff::lines(&existing, &generated) {
            match diff {
                diff::Result::Left(l) => {
                    if !has_diff {
                        eprintln!("Rust bindings are out of sync with {path}:");
                        has_diff = true;
                    }
                    eprintln!("- {l}");
                }
                diff::Result::Right(r) => {
                    if !has_diff {
                        eprintln!("Rust bindings are out of sync with {path}:");
                        has_diff = true;
                    }
                    eprintln!("+ {r}");
                }
                diff::Result::Both(..) => {}
            }
        }
        if has_diff {
            std::process::exit(1);
        }
        println!("Rust bindings are up to date with {path}");
    }
}

fn generate_rust_bindings(header: &str) -> bindgen::Bindings {
    bindgen::Builder::default()
        .header(header)
        .parse_callbacks(Box::new(TypeAliasCallback))
        .allowlist_type("Orc.*")
        .allowlist_var("ORC_.*")
        .allowlist_function("orc_.*")
        .derive_partialeq(true)
        .derive_eq(true)
        .derive_default(true)
        .no_default("OrcHost")
        .no_partialeq("OrcFuncInfo")
        .no_partialeq("OrcHost.*")
        .no_partialeq("OrcHandle")
        .no_copy("OrcHandle")
        .generate()
        .expect("Unable to generate bindings")
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
            "ORC_ARGS_VARIADIC" => Some(bindgen::callbacks::IntKind::U64),
            _ => None,
        }
    }
}
