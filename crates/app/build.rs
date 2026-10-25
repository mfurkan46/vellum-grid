use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=vellum-grid.rc");
    println!("cargo:rerun-if-changed=../../assets/vellum-grid.ico");
    let windows = std::env::var("CARGO_CFG_TARGET_OS").is_ok_and(|os| os == "windows");
    let gnu = std::env::var("CARGO_CFG_TARGET_ENV").is_ok_and(|env| env == "gnu");
    if !windows || !gnu {
        return;
    }
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("cargo sets OUT_DIR"));
    let object = out.join("vellum-grid-resources.o");
    let status = Command::new("windres")
        .args([
            "--codepage=65001",
            "-O",
            "coff",
            "-i",
            "vellum-grid.rc",
            "-o",
        ])
        .arg(&object)
        .status();
    match status {
        Ok(status) if status.success() => {
            println!("cargo:rustc-link-arg-bins={}", object.display());
        }
        _ => println!("cargo:warning=windres failed; the exe is built without an icon"),
    }
}
