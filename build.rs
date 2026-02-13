fn main() {
    // Windows only: embed application icon and version metadata into the exe
    #[cfg(target_os = "windows")]
    {
        let icon_path = "assets/icon.ico";
        let mut res = winresource::WindowsResource::new();

        if std::path::Path::new(icon_path).exists() {
            res.set_icon(icon_path);
        } else {
            println!("cargo:warning=assets/icon.ico not found — run `cargo run --manifest-path tools/gen-icon/Cargo.toml` to generate it");
        }

        res.compile().expect("Failed to compile Windows resources");
    }
}
