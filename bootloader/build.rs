use std::env;
use std::path::PathBuf;

fn main() {
    let kernel = PathBuf::from(
        env::var_os("CARGO_BIN_FILE_KERNEL_kernel")
            .expect("kernel artifact not found"),
    );

    println!("NOVA: kernel artifact = {}", kernel.display());

    let out_dir = PathBuf::from(
        env::var_os("OUT_DIR")
            .expect("OUT_DIR not found"),
    );

    let bios_path = out_dir.join("nova-os-bios.img");

    bootloader::DiskImageBuilder::new(kernel)
        .create_bios_image(&bios_path)
        .expect("failed to create NOVA BIOS image");

    println!(
        "NOVA: BIOS image created = {}",
        bios_path.display()
    );

    println!(
        "cargo:rustc-env=NOVA_BIOS_IMAGE={}",
        bios_path.display()
    );
}
