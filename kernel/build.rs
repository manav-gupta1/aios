use std::env;
use std::path::PathBuf;

fn main() {
    let http_get_bin = PathBuf::from(
        env::var_os("CARGO_BIN_FILE_HTTP_GET")
            .expect("http-get artifact not found"),
    );
    println!("cargo:rustc-env=ELF_HTTP_GET_BIN_PATH={}", http_get_bin.display());

    let udp_test_bin = PathBuf::from(
        env::var_os("CARGO_BIN_FILE_UDP_TEST")
            .expect("udp-test artifact not found"),
    );
    println!("cargo:rustc-env=ELF_UDP_TEST_BIN_PATH={}", udp_test_bin.display());

    let ping_bin = PathBuf::from(
        env::var_os("CARGO_BIN_FILE_PING")
            .expect("ping artifact not found"),
    );
    println!("cargo:rustc-env=ELF_PING_BIN_PATH={}", ping_bin.display());
}
