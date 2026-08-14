use bootloader::DiskImageBuilder;

fn main() {
    let kernel = "../target/x86_64-unknown-none/debug/kernel";

    let bios_path = "nova-os.img";

    DiskImageBuilder::new(kernel)
        .create_bios_image(bios_path)
        .expect("failed to create NOVA OS image");

    println!("NOVA OS image created: {bios_path}");
}
