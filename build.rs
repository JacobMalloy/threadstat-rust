use std::env;
use std::path::PathBuf;

fn main() {
    let bindings = bindgen::Builder::default()
        .header("wrapper/shared_def.h")
        .derive_default(true)
        .impl_debug(true)
        .generate()
        .expect("Unable to generate shared_def bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("shared_def_bindings.rs"))
        .expect("Couldn't write bindings");
}
