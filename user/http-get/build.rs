use std::env;
use std::path::PathBuf;
fn main() {
    let dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let linker_script = PathBuf::from(dir).join("linker.ld");
    println!("cargo:rustc-link-arg=-T{}", linker_script.display());
}
