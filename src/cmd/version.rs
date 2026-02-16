pub fn run() {
    println!("kmd {}", env!("CARGO_PKG_VERSION"));
    println!("kmd-core {}", kmd_core::Index::current_version());
    println!("target {}", std::env::consts::ARCH);
    println!("os {}", std::env::consts::OS);
}
