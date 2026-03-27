use std::env;
use std::path::PathBuf;

fn main() {
    #[cfg(feature = "pfm")]
    println!("cargo:rustc-link-lib=pfm");

    let builder = bindgen::Builder::default()
        .header("wrapper.h")
        .derive_default(true)
        .impl_debug(true);

    #[cfg(feature = "pfm")]
    let builder = builder.clang_arg("-DUSE_PFM");

    let bindings = builder
        .generate()
        .expect("Unable to generate perf_event bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("perf_bindings.rs"))
        .expect("Couldn't write bindings");
}
