use std::env;
use std::path::PathBuf;

fn main() {
    let http_get_bin = PathBuf::from(
        env::var_os("CARGO_BIN_FILE_HTTP_GET")
            .expect("http-get artifact not found"),
    );
    println!("cargo:rustc-env=ELF_HTTP_GET_BIN_PATH={}", http_get_bin.display());
}
