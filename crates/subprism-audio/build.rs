fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        // Required at runtime for screencapturekit's Swift concurrency bridge
        println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
    }
}
