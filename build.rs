fn main() -> std::io::Result<()> {
    if !cfg!(target_arch = "wasm32") && cfg!(target_os = "linux") {
        // println!("cargo:rustc-link-lib=vulkan");
    }

    Ok(())
}
