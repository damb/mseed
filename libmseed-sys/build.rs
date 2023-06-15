extern crate bindgen;

use std::env;
use std::path::PathBuf;
use std::process::Command;

use bindgen::CargoCallbacks;

const LIB_DIR: &str = "vendor";

fn main() {
    let package_dir_path = PathBuf::from(&env::var("CARGO_MANIFEST_DIR").unwrap())
        .canonicalize()
        .expect("cannot canonicalize path");

    let lib_dir_path = package_dir_path.join(LIB_DIR);
    let lib_dir_path_str = lib_dir_path.to_str().expect("path is not a valid string");

    // build library
    env::set_current_dir(&lib_dir_path).expect("failed to change directory");
    Command::new("make")
        .output()
        .expect("failed to build library");
    env::set_current_dir(&package_dir_path).expect("failed to change directory");

    let headers_path = lib_dir_path.join("libmseed.h");
    if !headers_path.try_exists().is_ok() {
        panic!("header file does not exist");
    }
    let headers_path_str = headers_path.to_str().expect("path is not a valid string");

    println!("cargo:rustc-link-lib=static=mseed");
    println!("cargo:rustc-link-search=native={}", lib_dir_path_str);
    println!("cargo:rerun-if-changed={}", headers_path_str);

    println!("Searching for libraries at: {}", lib_dir_path_str);
    println!("Generate bindings.rs");

    let bindings = bindgen::Builder::default()
        .header(headers_path_str)
        .parse_callbacks(Box::new(CargoCallbacks))
        .clang_arg(&format!("-I{}", lib_dir_path_str))
        .allowlist_type("MS.*")
        .allowlist_var("MS_.*")
        .allowlist_var("HPT.*")
        .allowlist_var("DE_.*")
        .allowlist_var("MSF_.*")
        .allowlist_var("NSTERROR")
        .allowlist_function("ms_.*")
        .allowlist_function("msr_.*")
        .allowlist_function("ms3_.*")
        .allowlist_function("mst_.*")
        .allowlist_function("mstl3_.*")
        .allowlist_function("msr3_.*")
        .generate()
        .expect("Unable to generate bindings");
    // println!("version: {:?}", bindgen::clang_version());

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap()).join("bindings.rs");
    bindings
        .write_to_file(out_path)
        .expect("Couldn't write bindings!");
}
