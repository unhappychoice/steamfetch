use std::path::PathBuf;

fn main() {
    set_rpath();
    copy_steam_api_library();
}

/// Set rpath so the binary looks for Steam API library in the same directory
/// and in ../lib/steamfetch/ (for Homebrew: bin/ -> lib/steamfetch/)
fn set_rpath() {
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

/// Copy libsteam_api shared library next to the output binary so it can be found at runtime.
/// This handles `cargo build` and `cargo install` cases where the library would otherwise
/// only exist deep inside the build directory.
fn copy_steam_api_library() {
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");
    let out_path = PathBuf::from(&out_dir);

    let lib_name = steam_api_lib_name();

    // OUT_DIR is typically: target/<profile>/build/<pkg>-<hash>/out
    //   nth(0) = .../out
    //   nth(1) = .../steamfetch-<hash>
    //   nth(2) = .../build
    //   nth(3) = .../<profile>  (e.g., target/release or target/debug)
    let profile_dir = match out_path.ancestors().nth(3) {
        Some(dir) => dir.to_path_buf(),
        None => return,
    };

    let build_dir = profile_dir.join("build");
    let lib_src = find_steam_api_lib(&build_dir, lib_name);
    let lib_src = match lib_src {
        Some(path) => path,
        None => return,
    };

    let lib_dst = profile_dir.join(lib_name);
    if lib_src != lib_dst {
        let _ = std::fs::copy(&lib_src, &lib_dst);
    }
}

fn find_steam_api_lib(build_dir: &PathBuf, lib_name: &str) -> Option<PathBuf> {
    std::fs::read_dir(build_dir)
        .ok()?
        .filter_map(|e| e.ok())
        .find_map(|entry| {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if !name_str.starts_with("steamworks-sys-") {
                return None;
            }
            let candidate = entry.path().join("out").join(lib_name);
            candidate.exists().then_some(candidate)
        })
}

fn steam_api_lib_name() -> &'static str {
    #[cfg(target_os = "linux")]
    {
        "libsteam_api.so"
    }
    #[cfg(target_os = "macos")]
    {
        "libsteam_api.dylib"
    }
    #[cfg(target_os = "windows")]
    {
        "steam_api64.dll"
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        "libsteam_api.so"
    }
}
