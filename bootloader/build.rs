use std::env;
use std::path::PathBuf;

fn main() {
    let kernel = PathBuf::from(
        env::var_os("CARGO_BIN_FILE_KERNEL_kernel")
            .expect("kernel artifact not found"),
    );

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let bios_path = out_dir.join("nova-os-bios.img");

    bootloader::DiskImageBuilder::new(kernel)
        .create_bios_image(&bios_path)
        .expect("failed to create NOVA BIOS image");

    println!("cargo:rustc-env=NOVA_BIOS_IMAGE={}", bios_path.display());
}

