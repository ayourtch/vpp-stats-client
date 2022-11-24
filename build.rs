extern crate bindgen;

use bindgen::builder;
use std::env;
use std::path::PathBuf;
use std::process::Command;

fn find_vpp_lib_dir() -> String {
    /*
     * In the future there's more cleverness possibly to be added.
     * For now this will do.
     */
    let path = "/usr/lib/x86_64-linux-gnu/".to_string();
    path
}

fn main() {
    // Tell cargo to tell rustc to link the system bzip2
    // shared library.
    //println!("cargo:rustc-link-lib=bz2");

    // The bindgen::Builder is the main entry point
    // to bindgen, and lets you build up options for
    // the resulting bindings.
    // let mut bindings = bindgen::Builder::default()
    let mut bindings = bindgen::Builder::default()
        // The input header we would like to generate
        // bindings for.
        .header("wrapper.h")
        .clang_arg("-I/home/ubuntu/vpp/build-root/install-vpp_debug-native/vpp/include")
        .no_convert_floats()
        .allowlist_type("stat_.*")
        .allowlist_type("vec_header_t")
        .allowlist_function("stat_.*")
        .allowlist_function("clib_mem_init")
        .derive_debug(true)
        .derive_default(true)
        .generate()
        // Finish the builder and generate the bindings.
        // Unwrap the Result and panic on failure.
        .expect("Unable to generate bindings");

    /*
    The manipulations with types above are because bindgen does not handle this
    type of C code:
    struct blah_t;
    typedef void *(blah_callback_t)(struct blah_t *param);
    typedef struct {
      blah_callback_t *blah_callback;
    } blah_t;
    */

    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    let out_file_name = out_path.join("bindings.rs");
    bindings
        .write_to_file(out_file_name.clone())
        .expect("Couldn't write bindings!");

    Command::new("rustup")
        .args(&["run", "nightly", "rustfmt", out_file_name.to_str().unwrap()])
        .status(); // .unwrap();

    let vpp_lib_dir = match env::var("VPP_LIB_DIR") {
        Ok(val) => val,
        Err(_e) => find_vpp_lib_dir(),
    };
    if !std::path::Path::new(&format!("{}/libvppapiclient.so", &vpp_lib_dir)).exists() {
        panic!("Can not find libvppapiclient.so at {}, please install python3-vpp-api or define VPP_LIB_DIR accordingly", vpp_lib_dir)
    }
    let flags = format!(
        "cargo:rustc-flags=-L{} -lvppapiclient -lvppinfra",
        &vpp_lib_dir
    );

    // Tell cargo to tell rustc to link the VPP client library
    println!("{}", flags);
}
