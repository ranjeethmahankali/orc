fn main() {
    let args: Vec<String> = std::env::args().collect();
    let header = match args.get(1) {
        Some(h) if !h.starts_with("--") => h.clone(),
        _ => {
            eprintln!(
                "Usage: bindings_generator <header.h> [--rust-output <path>] [--rust-check <path>] [--python-output <path>] [--python-check <path>]"
            );
            std::process::exit(1);
        }
    };
    let mut rust_output: Option<String> = None;
    let mut rust_check: Option<String> = None;
    let mut python_output: Option<String> = None;
    let mut python_check: Option<String> = None;
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
            "--python-output" => {
                python_output = Some(next());
                i += 2;
            }
            "--python-check" => {
                python_check = Some(next());
                i += 2;
            }
            other => {
                eprintln!("Unknown argument: {other}");
                std::process::exit(1);
            }
        }
    }
    if rust_output.is_none()
        && rust_check.is_none()
        && python_output.is_none()
        && python_check.is_none()
    {
        eprintln!(
            "At least one of --rust-output, --rust-check, --python-output, --python-check must be provided"
        );
        std::process::exit(1);
    }
    let needs_rust = rust_output.is_some() || rust_check.is_some();
    let needs_python = python_output.is_some() || python_check.is_some();
    if needs_rust {
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
            if generated != existing {
                eprintln!("Rust bindings are out of sync with {path}");
                std::process::exit(1);
            }
            println!("Rust bindings are up to date with {path}");
        }
    }
    if needs_python {
        let python_content = {
            let mut buf = Vec::new();
            generate_python_bindings(&header, &mut buf);
            buf
        };
        if let Some(path) = &python_output {
            std::fs::write(path, &python_content).unwrap_or_else(|e| {
                eprintln!("Couldn't write {path}: {e}");
                std::process::exit(1);
            });
            println!("Python bindings written to {path}");
        }
        if let Some(path) = &python_check {
            let generated =
                String::from_utf8(python_content).expect("Python bindings are not valid UTF-8");
            let existing = std::fs::read_to_string(path).unwrap_or_else(|e| {
                eprintln!("Couldn't read {path}: {e}");
                std::process::exit(1);
            });
            if generated != existing {
                eprintln!("Python bindings are out of sync with {path}");
                std::process::exit(1);
            }
            println!("Python bindings are up to date with {path}");
        }
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

// ---------------------------------------------------------------------------
// Python ctypes codegen via libclang
// ---------------------------------------------------------------------------

fn generate_python_bindings(header_path: &str, out: &mut impl std::io::Write) {
    let header_content = std::fs::read_to_string(header_path).expect("Failed to read orc_abi.h");
    writeln!(
        out,
        "\"\"\"Auto-generated ctypes bindings for orc_abi.h. Do not edit.\"\"\""
    )
    .unwrap();
    writeln!(out).unwrap();
    writeln!(out, "import ctypes").unwrap();
    writeln!(out).unwrap();
    emit_constants(&header_content, out);
    let cl = clang::Clang::new().expect("Failed to initialize libclang");
    let index = clang::Index::new(&cl, false, false);
    let tu = index
        .parser(header_path)
        .arguments(&["-std=c11"])
        .parse()
        .expect("Failed to parse orc_abi.h");
    emit_types(&tu, out);
}

/// Extract `#define ORC_*` integer constants and `static const` values.
/// Also passes through `// comment` lines that appear between #define groups.
fn emit_constants(header: &str, out: &mut impl std::io::Write) {
    let mut pending_comment: Option<&str> = None;
    let mut emitted_any = false;
    for line in header.lines() {
        let line = line.trim();
        if let Some(comment) = line.strip_prefix("// ") {
            pending_comment = Some(comment);
            continue;
        }
        if line.is_empty() {
            if emitted_any {
                writeln!(out).unwrap();
                emitted_any = false;
            }
            pending_comment = None;
            continue;
        }
        if let Some(rest) = line.strip_prefix("#define ") {
            let mut parts = rest.splitn(2, ' ');
            let name = match parts.next() {
                Some(n) => n,
                None => continue,
            };
            let value = match parts.next() {
                Some(v) => v.trim(),
                None => continue,
            };
            if name.contains('(') || !name.starts_with("ORC_") {
                continue;
            }
            if value.starts_with("__") {
                continue;
            }
            if let Some(val) = parse_c_integer(value) {
                if let Some(comment) = pending_comment.take() {
                    writeln!(out, "# {comment}").unwrap();
                }
                writeln!(out, "{name} = {val}").unwrap();
                emitted_any = true;
            }
        }
        if line.starts_with("static const uint64_t ORC_ABI_VERSION")
            && let Some(start) = line.find("ORC_VERSION_PACK(")
        {
            let args_start = start + "ORC_VERSION_PACK(".len();
            if let Some(end) = line[args_start..].find(')') {
                let mut parts = line[args_start..args_start + end].split(',');
                if let (Some(a), Some(b), Some(c)) = (parts.next(), parts.next(), parts.next())
                    && parts.next().is_none()
                {
                    let major: u64 = a.trim().parse().unwrap_or(0);
                    let minor: u64 = b.trim().parse().unwrap_or(0);
                    let patch: u64 = c.trim().parse().unwrap_or(0);
                    let version = (major << 42) | (minor << 21) | patch;
                    writeln!(out, "ORC_ABI_VERSION = {version}").unwrap();
                    emitted_any = true;
                }
            }
        }
    }
    writeln!(out).unwrap();
}

fn parse_c_integer(s: &str) -> Option<&str> {
    let s = s.trim();
    let s = s.trim_end_matches('u').trim_end_matches('U');
    let s = s
        .trim_end_matches("LL")
        .trim_end_matches("ll")
        .trim_end_matches('L')
        .trim_end_matches('l');
    if s.starts_with("0x") || s.starts_with("0X") {
        u64::from_str_radix(&s[2..], 16).ok().map(|_| s)
    } else {
        s.parse::<u64>().ok().map(|_| s)
    }
}

use std::borrow::Cow;

// ---------------------------------------------------------------------------
// AST-driven type emission
// ---------------------------------------------------------------------------

/// A collected struct definition.
struct StructDef {
    name: String,
    comment: Option<String>,
    fields: Vec<(String, Cow<'static, str>, Option<String>)>, // (field_name, ctypes_expr, comment)
}

/// A named function-pointer type (for anonymous fn-ptr fields in structs).
struct FnPtrDef {
    name: String,
    cfunctype: String,
}

fn emit_types(tu: &clang::TranslationUnit, out: &mut impl std::io::Write) {
    use clang::EntityKind;
    use std::collections::HashSet;
    let root = tu.get_entity();
    let mut simple_typedefs: Vec<(String, Cow<'static, str>)> = Vec::new();
    let mut array_typedefs: Vec<(String, Cow<'static, str>, usize)> = Vec::new();
    let mut fn_ptr_typedefs: Vec<(String, String)> = Vec::new();
    let mut struct_defs: Vec<StructDef> = Vec::new();
    let mut struct_field_fn_types: Vec<FnPtrDef> = Vec::new();
    let mut seen_structs: HashSet<String> = HashSet::new();
    let mut seen_fn_ptrs: HashSet<String> = HashSet::new();
    for child in root.get_children() {
        if let Some(loc) = child.get_location() {
            if !loc.is_in_main_file() {
                continue;
            }
        } else {
            continue;
        }
        match child.get_kind() {
            EntityKind::TypedefDecl => {
                let name = match child.get_name() {
                    Some(n) => n,
                    None => continue,
                };
                let underlying = match child.get_typedef_underlying_type() {
                    Some(t) => t,
                    None => continue,
                };
                let canon = underlying.get_canonical_type();
                match canon.get_kind() {
                    clang::TypeKind::UChar
                    | clang::TypeKind::UShort
                    | clang::TypeKind::UInt
                    | clang::TypeKind::ULong
                    | clang::TypeKind::ULongLong
                    | clang::TypeKind::SChar
                    | clang::TypeKind::CharS
                    | clang::TypeKind::Short
                    | clang::TypeKind::Int
                    | clang::TypeKind::Long
                    | clang::TypeKind::LongLong
                    | clang::TypeKind::Float
                    | clang::TypeKind::Double
                    | clang::TypeKind::Bool => {
                        simple_typedefs.push((name, primitive_to_ctypes(&canon)));
                    }
                    clang::TypeKind::Record => {
                        if seen_structs.contains(&name) {
                            continue;
                        }
                        let decl = match canon.get_declaration() {
                            Some(d) => d,
                            None => continue,
                        };
                        if decl.is_definition() {
                            seen_structs.insert(name.clone());
                            let comment = entity_comment(&child).or_else(|| entity_comment(&decl));
                            let fields = collect_struct_fields(
                                &decl,
                                &mut struct_field_fn_types,
                                &mut seen_fn_ptrs,
                            );
                            struct_defs.push(StructDef {
                                name,
                                comment,
                                fields,
                            });
                        }
                    }
                    clang::TypeKind::Pointer => {
                        if let Some(pointee) = canon.get_pointee_type()
                            && matches!(
                                pointee.get_kind(),
                                clang::TypeKind::FunctionPrototype
                                    | clang::TypeKind::FunctionNoPrototype
                            )
                        {
                            fn_ptr_typedefs.push((name, generate_cfunctype(&pointee)));
                        }
                    }
                    clang::TypeKind::ConstantArray => {
                        if let (Some(elem), Some(count)) =
                            (canon.get_element_type(), get_array_element_count(&canon))
                        {
                            array_typedefs.push((name, type_to_ctypes(&elem), count));
                        }
                    }
                    _ => {}
                }
            }
            EntityKind::StructDecl => {
                let name = match child.get_name() {
                    Some(n) => n,
                    None => continue,
                };
                if !child.is_definition() || seen_structs.contains(&name) {
                    continue;
                }
                seen_structs.insert(name.clone());
                let comment = entity_comment(&child);
                let fields =
                    collect_struct_fields(&child, &mut struct_field_fn_types, &mut seen_fn_ptrs);
                struct_defs.push(StructDef {
                    name,
                    comment,
                    fields,
                });
            }
            _ => {}
        }
    }
    for (name, expr) in &simple_typedefs {
        writeln!(out, "{name} = {expr}").unwrap();
    }
    writeln!(out).unwrap();
    for sd in &struct_defs {
        writeln!(out, "class {}(ctypes.Structure):", sd.name).unwrap();
        if let Some(comment) = &sd.comment {
            writeln!(out, "    \"\"\"{}\"\"\"", comment).unwrap();
        } else {
            writeln!(out, "    pass").unwrap();
        }
        writeln!(out).unwrap();
    }
    for (name, elem, count) in &array_typedefs {
        writeln!(out, "{name} = {elem} * {count}").unwrap();
    }
    if !array_typedefs.is_empty() {
        writeln!(out).unwrap();
    }
    for (name, cfunc) in &fn_ptr_typedefs {
        writeln!(out, "{name} = {cfunc}").unwrap();
    }
    if !fn_ptr_typedefs.is_empty() {
        writeln!(out).unwrap();
    }
    for fp in &struct_field_fn_types {
        writeln!(out, "{} = {}", fp.name, fp.cfunctype).unwrap();
    }
    if !struct_field_fn_types.is_empty() {
        writeln!(out).unwrap();
    }
    for sd in &struct_defs {
        writeln!(out, "{}._fields_ = [", sd.name).unwrap();
        for (fname, ftype, comment) in &sd.fields {
            if let Some(c) = comment {
                writeln!(out, "    (\"{fname}\", {ftype}),  # {c}").unwrap();
            } else {
                writeln!(out, "    (\"{fname}\", {ftype}),").unwrap();
            }
        }
        writeln!(out, "]").unwrap();
        writeln!(out).unwrap();
    }
}

fn collect_struct_fields(
    decl: &clang::Entity,
    fn_type_names: &mut Vec<FnPtrDef>,
    seen_fn_ptrs: &mut std::collections::HashSet<String>,
) -> Vec<(String, Cow<'static, str>, Option<String>)> {
    use clang::EntityKind;
    let mut fields = Vec::new();
    for child in decl.get_children() {
        if child.get_kind() != EntityKind::FieldDecl {
            continue;
        }
        let fname = match child.get_name() {
            Some(n) => n,
            None => continue,
        };
        let ftype = match child.get_type() {
            Some(t) => t,
            None => continue,
        };
        let comment = entity_comment(&child);
        if matches!(
            ftype.get_kind(),
            clang::TypeKind::Typedef | clang::TypeKind::Elaborated
        ) {
            let display = ftype.get_display_name();
            let clean = strip_const_struct(&display);
            if clean.starts_with("Orc") {
                let cow: Cow<'static, str> = if clean.len() == display.len() {
                    display.into()
                } else {
                    clean.to_string().into()
                };
                fields.push((fname, cow, comment));
                continue;
            }
        }
        let canon = ftype.get_canonical_type();
        if canon.get_kind() == clang::TypeKind::Pointer
            && let Some(pointee) = canon.get_pointee_type()
            && matches!(
                pointee.get_kind(),
                clang::TypeKind::FunctionPrototype | clang::TypeKind::FunctionNoPrototype
            )
        {
            let type_name = fn_field_type_name(&fname);
            if seen_fn_ptrs.insert(type_name.clone()) {
                let cfunctype = generate_cfunctype(&pointee);
                fn_type_names.push(FnPtrDef {
                    name: type_name.clone(),
                    cfunctype,
                });
            }
            fields.push((fname, type_name.into(), comment));
            continue;
        }
        fields.push((fname, type_to_ctypes(&ftype), comment));
    }
    fields
}

/// Convert a snake_case field name to a CamelCase function-pointer type name.
/// e.g. "report_message" → "OrcReportMessageFn"
fn fn_field_type_name(field_name: &str) -> String {
    let mut out = String::from("Orc");
    for part in field_name.split('_') {
        let mut chars = part.chars();
        if let Some(c) = chars.next() {
            for upper in c.to_uppercase() {
                out.push(upper);
            }
            out.push_str(chars.as_str());
        }
    }
    out.push_str("Fn");
    out
}

fn generate_cfunctype(func_ty: &clang::Type) -> String {
    use std::fmt::Write;
    let ret = func_ty
        .get_result_type()
        .map(|r| type_to_ctypes(&r))
        .unwrap_or_else(|| "None".into());
    let args = func_ty.get_argument_types().unwrap_or_default();
    let mut buf = format!("ctypes.CFUNCTYPE({ret}");
    for a in &args {
        write!(buf, ", {}", type_to_ctypes(a)).unwrap();
    }
    buf.push(')');
    buf
}

/// Extract and clean a comment from a clang entity.
fn entity_comment(entity: &clang::Entity) -> Option<String> {
    let raw = entity.get_comment()?;
    let mut cleaned = String::new();
    for l in raw.lines() {
        let l = l.trim();
        if l == "/*" || l == "*/" || l == "/**" {
            continue;
        }
        let l = l
            .strip_prefix("///")
            .or_else(|| l.strip_prefix("//"))
            .or_else(|| l.strip_prefix("/*"))
            .or_else(|| l.strip_prefix("* "))
            .or_else(|| l.strip_prefix("*"))
            .unwrap_or(l);
        let l = l.strip_suffix("*/").unwrap_or(l).trim();
        if l.is_empty() {
            continue;
        }
        if !cleaned.is_empty() {
            cleaned.push(' ');
        }
        cleaned.push_str(l);
    }
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned)
    }
}

/// Strip `const ` and `struct ` prefixes from a clang type display name.
fn strip_const_struct(name: &str) -> &str {
    let s = name.strip_prefix("const ").unwrap_or(name);
    s.strip_prefix("struct ").unwrap_or(s)
}

fn type_to_ctypes(ty: &clang::Type) -> Cow<'static, str> {
    use clang::TypeKind;
    match ty.get_kind() {
        TypeKind::Typedef | TypeKind::Elaborated | TypeKind::Record => {
            let display = ty.get_display_name();
            let name = strip_const_struct(&display);
            if name.starts_with("Orc") || ty.get_kind() == TypeKind::Record {
                return if name.len() == display.len() {
                    display.into()
                } else {
                    name.to_string().into()
                };
            }
            type_to_ctypes(&ty.get_canonical_type())
        }
        TypeKind::Pointer => {
            let pointee = match ty.get_pointee_type() {
                Some(p) => p,
                None => return "ctypes.c_void_p".into(),
            };
            let canon_pointee = pointee.get_canonical_type();
            match canon_pointee.get_kind() {
                TypeKind::Void => "ctypes.c_void_p".into(),
                TypeKind::CharS | TypeKind::SChar | TypeKind::CharU | TypeKind::UChar
                    if looks_like_char(&pointee) =>
                {
                    "ctypes.c_char_p".into()
                }
                TypeKind::FunctionPrototype | TypeKind::FunctionNoPrototype => {
                    generate_cfunctype(&canon_pointee).into()
                }
                _ => {
                    let inner = type_to_ctypes(&pointee);
                    format!("ctypes.POINTER({inner})").into()
                }
            }
        }
        TypeKind::ConstantArray => {
            let elem = ty
                .get_element_type()
                .map(|e| type_to_ctypes(&e))
                .unwrap_or_else(|| "ctypes.c_uint8".into());
            let count = get_array_element_count(ty).unwrap_or(0);
            format!("{elem} * {count}").into()
        }
        _ => primitive_to_ctypes(ty),
    }
}

/// Check if a type is `char` (as opposed to `uint8_t` which is also `unsigned char`).
fn looks_like_char(ty: &clang::Type) -> bool {
    let display = ty.get_display_name();
    display.contains("char") && !display.contains("uint") && !display.contains("int")
}

fn primitive_to_ctypes(ty: &clang::Type) -> Cow<'static, str> {
    use clang::TypeKind;
    match ty.get_kind() {
        TypeKind::Void => "None".into(),
        TypeKind::Bool => "ctypes.c_bool".into(),
        TypeKind::UChar => "ctypes.c_uint8".into(),
        TypeKind::UShort => "ctypes.c_uint16".into(),
        TypeKind::UInt => "ctypes.c_uint32".into(),
        TypeKind::ULong | TypeKind::ULongLong => "ctypes.c_uint64".into(),
        TypeKind::SChar | TypeKind::CharS => "ctypes.c_int8".into(),
        TypeKind::Short => "ctypes.c_int16".into(),
        TypeKind::Int => "ctypes.c_int32".into(),
        TypeKind::Long | TypeKind::LongLong => "ctypes.c_int64".into(),
        TypeKind::Float => "ctypes.c_float".into(),
        TypeKind::Double => "ctypes.c_double".into(),
        _ => format!("ctypes.c_void_p  # FIXME: {:?}", ty.get_kind()).into(),
    }
}

/// Get the element count of a ConstantArray type.
fn get_array_element_count(ty: &clang::Type) -> Option<usize> {
    let total = ty.get_sizeof().ok()?;
    let elem = ty.get_element_type()?;
    let elem_size = elem.get_sizeof().ok()?;
    if elem_size == 0 {
        return None;
    }
    Some(total / elem_size)
}
