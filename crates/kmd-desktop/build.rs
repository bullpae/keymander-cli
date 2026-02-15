fn main() {
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set("OriginalFilename", "kmd-desktop.exe");
        res.set("FileDescription", "keymander Desktop - keyboard-driven launcher");
        res.set("ProductName", "keymander Desktop");
        // icon.ico will be added once available
        if let Err(e) = res.compile() {
            eprintln!("winresource: {e}");
        }
    }
}
