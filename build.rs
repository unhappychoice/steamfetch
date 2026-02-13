fn main() {
    // Set rpath so the binary looks for Steam API library in the same directory
    // and in ../lib/steamfetch/ (for Homebrew: bin/ -> lib/steamfetch/)
    #[cfg(target_os = "linux")]
    {
        println!("cargo:rustc-link-arg=-Wl,-rpath,$ORIGIN");
        println!("cargo:rustc-link-arg=-Wl,-rpath,$ORIGIN/../lib/steamfetch");
    }

    #[cfg(target_os = "macos")]
    {
        println!("cargo:rustc-link-arg=-Wl,-rpath,@executable_path");
        println!("cargo:rustc-link-arg=-Wl,-rpath,@executable_path/../lib/steamfetch");
    }
}
