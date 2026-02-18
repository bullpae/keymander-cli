fn main() {
    #[cfg(windows)]
    {
        let ico_path = "../../assets/icon.ico";
        let mut res = winresource::WindowsResource::new();
        res.set("OriginalFilename", "kmd-desktop.exe");
        res.set(
            "FileDescription",
            "keymander Desktop - keyboard-driven launcher",
        );
        res.set("ProductName", "keymander Desktop");

        if std::path::Path::new(ico_path).exists() {
            res.set_icon(ico_path);
        } else {
            println!("cargo:warning=assets/icon.ico not found — run `cargo run --manifest-path tools/gen-icon/Cargo.toml`");
        }

        if let Err(e) = res.compile() {
            eprintln!("winresource: {e}");
        }
    }
}
