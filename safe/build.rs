use std::env;
use std::fs;
use std::io;
use std::process::Command;
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
    println!(
        "cargo:rerun-if-changed={}",
        manifest_dir.join("src/pack.rs").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        manifest_dir.join("src/unpack.rs").display()
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
    println!("cargo:rerun-if-env-changed=CC");
    println!("cargo:rerun-if-env-changed=AR");
    compile_shims(&manifest_dir, &out_dir)?;

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

fn compile_shims(manifest_dir: &Path, out_dir: &Path) -> io::Result<()> {
    let cc = env::var("CC").unwrap_or_else(|_| {
        if Path::new("/usr/bin/cc").exists() {
            String::from("/usr/bin/cc")
        } else {
            String::from("cc")
        }
    });
    let ar = env::var("AR").unwrap_or_else(|_| {
        if Path::new("/usr/bin/ar").exists() {
            String::from("/usr/bin/ar")
        } else {
            String::from("ar")
        }
    });
    let include_dir = manifest_dir.join("include");
    let sources = [
        manifest_dir.join("csrc/pack_unpack_shim.c"),
        manifest_dir.join("csrc/sprintf_shim.c"),
    ];
    let archive_path = out_dir.join("libjansson_shims.a");
    let mut objects = Vec::with_capacity(sources.len());

    for source in sources {
        let stem = source
            .file_stem()
            .expect("shim source should have a file stem");
        let object_path = out_dir.join(Path::new(stem)).with_extension("o");
        let status = Command::new(&cc)
            .arg("-I")
            .arg(&include_dir)
            .arg("-fPIC")
            .arg("-Wall")
            .arg("-Wextra")
            .arg("-Wno-unused-parameter")
            .arg("-c")
            .arg(&source)
            .arg("-o")
            .arg(&object_path)
            .status()?;

        if !status.success() {
            return Err(io::Error::other(format!(
                "failed to compile {} with {}",
                source.display(),
                cc
            )));
        }

        objects.push(object_path);
    }

    let status = Command::new(&ar)
        .arg("crs")
        .arg(&archive_path)
        .args(&objects)
        .status()?;
    if !status.success() {
        return Err(io::Error::other(format!(
            "failed to archive shim objects with {}",
            ar
        )));
    }

    Ok(())
}
