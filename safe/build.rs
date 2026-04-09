use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const ABI_VERSION_NODE: &str = "libjansson.so.4";
const ABI_RUNTIME_VERSION: &str = "4.14.0";

fn main() -> io::Result<()> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent()
        .expect("safe/ should live under the repository root");
    let def_path = workspace_root.join("original/jansson-2.14/src/jansson.def");
    let checked_map_path = manifest_dir.join("jansson.map");
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let generated_map_path = out_dir.join("jansson.generated.map");

    println!("cargo:rerun-if-changed={}", def_path.display());
    println!("cargo:rerun-if-changed={}", checked_map_path.display());
    println!(
        "cargo:rerun-if-changed={}",
        manifest_dir.join("build.rs").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        manifest_dir.join("csrc/pack_unpack_shim.c").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        manifest_dir.join("csrc/sprintf_shim.c").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        manifest_dir.join("include/jansson.h").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        manifest_dir.join("include/jansson_config.h").display()
    );

    let generated_map = generate_version_script(&def_path)?;
    fs::write(&generated_map_path, &generated_map)?;

    let checked_map = fs::read_to_string(&checked_map_path)?;
    if normalize_newlines(&checked_map) != generated_map {
        panic!(
            "{} is stale; regenerate it from {}",
            checked_map_path.display(),
            def_path.display()
        );
    }

    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux") {
        println!(
            "cargo:rustc-cdylib-link-arg=-Wl,--version-script={}",
            generated_map_path.display()
        );
        println!("cargo:rustc-cdylib-link-arg=-Wl,-soname,{ABI_VERSION_NODE}");
    }

    println!("cargo:rustc-env=JANSSON_RUNTIME_VERSION={ABI_RUNTIME_VERSION}");

    cc::Build::new()
        .include(manifest_dir.join("include"))
        .files([
            manifest_dir.join("csrc/pack_unpack_shim.c"),
            manifest_dir.join("csrc/sprintf_shim.c"),
        ])
        .cargo_metadata(false)
        .flag_if_supported("-Wno-unused-parameter")
        .warnings(true)
        .compile("jansson_shims");

    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static:+whole-archive=jansson_shims");

    Ok(())
}

fn generate_version_script(def_path: &Path) -> io::Result<String> {
    let def = fs::read_to_string(def_path)?;
    let mut script = String::from("libjansson.so.4 {\n    global:\n");

    for line in def.lines() {
        let symbol = line.trim();
        if symbol.is_empty() || symbol == "EXPORTS" {
            continue;
        }

        script.push_str("        ");
        script.push_str(symbol);
        script.push_str(";\n");
    }

    script.push_str("    local:\n        *;\n};\n");
    Ok(script)
}

fn normalize_newlines(input: &str) -> String {
    let mut normalized = input.replace("\r\n", "\n");
    if !normalized.ends_with('\n') {
        normalized.push('\n');
    }
    normalized
}
