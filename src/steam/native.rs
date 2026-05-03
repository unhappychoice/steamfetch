use anyhow::{Context, Result};
use libloading::{Library, Symbol};
use std::ffi::CString;
use std::os::raw::{c_char, c_void};
use steamworks::sys;

type CreateInterfaceFn = unsafe extern "C" fn(*const c_char, *mut i32) -> *mut c_void;

#[cfg(target_os = "linux")]
fn get_steam_client_path() -> Option<String> {
    let home = std::env::var("HOME").ok()?;
    let paths = [
        format!("{}/.steam/sdk64/steamclient.so", home),
        format!("{}/.steam/steam/linux64/steamclient.so", home),
        format!("{}/.local/share/Steam/linux64/steamclient.so", home),
    ];
    paths.into_iter().find(|p| std::path::Path::new(p).exists())
}

#[cfg(target_os = "windows")]
fn get_steam_client_path() -> Option<String> {
    let paths = [
        "C:\\Program Files (x86)\\Steam\\steamclient64.dll".to_string(),
        "C:\\Program Files\\Steam\\steamclient64.dll".to_string(),
    ];

    // Try registry first
    if let Some(path) = get_steam_path_from_registry() {
        let client_path = format!("{}\\steamclient64.dll", path);
        if std::path::Path::new(&client_path).exists() {
            return Some(client_path);
        }
    }

    paths.into_iter().find(|p| std::path::Path::new(p).exists())
}

#[cfg(target_os = "windows")]
fn get_steam_path_from_registry() -> Option<String> {
    use std::process::Command;

    let output = Command::new("reg")
        .args(["query", "HKCU\\Software\\Valve\\Steam", "/v", "SteamPath"])
        .output()
        .ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if line.contains("SteamPath") && line.contains("REG_SZ") {
            // Format: "    SteamPath    REG_SZ    C:/Program Files (x86)/Steam"
            if let Some(idx) = line.find("REG_SZ") {
                let path = line[idx + 6..].trim();
                // Convert forward slashes to backslashes
                return Some(path.replace('/', "\\"));
            }
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn get_steam_client_path() -> Option<String> {
    let home = std::env::var("HOME").ok()?;
    let path = format!(
        "{}/Library/Application Support/Steam/Steam.AppBundle/Steam/Contents/MacOS/steamclient.dylib",
        home
    );
    if std::path::Path::new(&path).exists() {
        Some(path)
    } else {
        None
    }
}

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
fn get_steam_client_path() -> Option<String> {
    None
}

/// Sets the DLL search directory on Windows so that steamclient64.dll's dependencies can be found
#[cfg(target_os = "windows")]
fn set_dll_directory(path: &str) {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "kernel32")]
    extern "system" {
        fn SetDllDirectoryW(path: *const u16) -> i32;
    }

    let wide: Vec<u16> = OsStr::new(path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        SetDllDirectoryW(wide.as_ptr());
    }
}

pub struct NativeSteamClient {
    _lib: Library, // Keep library loaded
    client: *mut sys::ISteamClient,
    pipe: sys::HSteamPipe,
    user: sys::HSteamUser,
    apps: *mut sys::ISteamApps,
    friends: *mut sys::ISteamFriends,
    steam_user: *mut sys::ISteamUser,
}

impl NativeSteamClient {
    pub fn try_new(verbose: bool) -> Option<Self> {
        let steam_path = match get_steam_client_path() {
            Some(p) => {
                if verbose {
                    eprintln!("[verbose] Found Steam client at: {}", p);
                }
                p
            }
            None => {
                if verbose {
                    eprintln!("[verbose] Steam client not found");
                }
                return None;
            }
        };

        unsafe {
            // On Windows, set DLL directory to Steam's folder so dependencies can be found
            #[cfg(target_os = "windows")]
            {
                if let Some(parent) = std::path::Path::new(&steam_path).parent() {
                    if let Some(dir_str) = parent.to_str() {
                        if verbose {
                            eprintln!("[verbose] Setting DLL directory to: {}", dir_str);
                        }
                        set_dll_directory(dir_str);
                    }
                }
            }

            let lib = match Library::new(&steam_path) {
                Ok(l) => l,
                Err(e) => {
                    if verbose {
                        eprintln!("[verbose] Failed to load Steam client: {}", e);
                    }
                    return None;
                }
            };
            let create_interface: Symbol<CreateInterfaceFn> = match lib.get(b"CreateInterface") {
                Ok(f) => f,
                Err(e) => {
                    if verbose {
                        eprintln!("[verbose] Failed to get CreateInterface: {}", e);
                    }
                    return None;
                }
            };

            // Get SteamClient interface
            let version = CString::new("SteamClient021").ok()?;
            let mut return_code: i32 = 0;
            let client_ptr = create_interface(version.as_ptr(), &mut return_code);
            if client_ptr.is_null() {
                if verbose {
                    eprintln!(
                        "[verbose] CreateInterface returned null (code: {})",
                        return_code
                    );
                }
                return None;
            }
            let client = client_ptr as *mut sys::ISteamClient;

            // Create pipe
            let pipe = sys::SteamAPI_ISteamClient_CreateSteamPipe(client);
            if pipe == 0 {
                if verbose {
                    eprintln!("[verbose] CreateSteamPipe failed");
                }
                return None;
            }

            // Connect to global user
            let user = sys::SteamAPI_ISteamClient_ConnectToGlobalUser(client, pipe);
            if user == 0 {
                if verbose {
                    eprintln!("[verbose] ConnectToGlobalUser failed");
                }
                sys::SteamAPI_ISteamClient_BReleaseSteamPipe(client, pipe);
                return None;
            }
            if verbose {
                eprintln!("[verbose] Successfully connected to Steam");
            }

            // Get ISteamApps
            let apps_version = CString::new("STEAMAPPS_INTERFACE_VERSION008").ok()?;
            let apps =
                sys::SteamAPI_ISteamClient_GetISteamApps(client, user, pipe, apps_version.as_ptr());
            if apps.is_null() {
                sys::SteamAPI_ISteamClient_ReleaseUser(client, pipe, user);
                sys::SteamAPI_ISteamClient_BReleaseSteamPipe(client, pipe);
                return None;
            }

            // Get ISteamFriends for username
            let friends_version = CString::new("SteamFriends017").ok()?;
            let friends = sys::SteamAPI_ISteamClient_GetISteamFriends(
                client,
                user,
                pipe,
                friends_version.as_ptr(),
            );

            // Get ISteamUser for steam ID
            let user_version = CString::new("SteamUser023").ok()?;
            let steam_user =
                sys::SteamAPI_ISteamClient_GetISteamUser(client, user, pipe, user_version.as_ptr());

            Some(Self {
                _lib: lib,
                client,
                pipe,
                user,
                apps,
                friends,
                steam_user,
            })
        }
    }

    pub fn steam_id(&self) -> u64 {
        unsafe {
            if self.steam_user.is_null() {
                return 0;
            }
            sys::SteamAPI_ISteamUser_GetSteamID(self.steam_user)
        }
    }

    pub fn username(&self) -> String {
        unsafe {
            if self.friends.is_null() {
                return String::from("Unknown");
            }
            let name_ptr = sys::SteamAPI_ISteamFriends_GetPersonaName(self.friends);
            if name_ptr.is_null() {
                return String::from("Unknown");
            }
            std::ffi::CStr::from_ptr(name_ptr)
                .to_string_lossy()
                .into_owned()
        }
    }

    pub fn get_owned_appids(&self, appids_to_check: &[u32]) -> Vec<u32> {
        appids_to_check
            .iter()
            .filter(|&&appid| self.is_subscribed(appid))
            .copied()
            .collect()
    }

    pub fn is_subscribed(&self, appid: u32) -> bool {
        unsafe { sys::SteamAPI_ISteamApps_BIsSubscribedApp(self.apps, appid) }
    }
}

impl Drop for NativeSteamClient {
    fn drop(&mut self) {
        unsafe {
            sys::SteamAPI_ISteamClient_ReleaseUser(self.client, self.pipe, self.user);
            sys::SteamAPI_ISteamClient_BReleaseSteamPipe(self.client, self.pipe);
        }
    }
}

/// Fetches all known Steam game AppIDs from gib.me/sam/games.xml
pub async fn fetch_all_game_appids() -> Result<Vec<u32>> {
    let url = "https://gib.me/sam/games.xml";
    let response = reqwest::get(url)
        .await
        .context("Failed to fetch games.xml")?
        .text()
        .await
        .context("Failed to read games.xml")?;

    Ok(parse_games_xml(&response))
}

fn parse_games_xml(xml: &str) -> Vec<u32> {
    let mut appids = Vec::new();
    let mut current_pos = 0;

    while let Some(start) = xml[current_pos..].find("<game") {
        let abs_start = current_pos + start;
        let tag_end = abs_start + 5; // len("<game")

        // Skip <games> tag - check if next char is 's' or '>'
        if xml[tag_end..].starts_with('s') {
            current_pos = tag_end;
            continue;
        }

        if let Some(end_tag) = xml[abs_start..].find('>') {
            let content_start = abs_start + end_tag + 1;
            if let Some(close) = xml[content_start..].find("</game>") {
                let content = &xml[content_start..content_start + close];
                if let Ok(appid) = content.trim().parse::<u32>() {
                    appids.push(appid);
                }
                current_pos = content_start + close + 7;
                continue;
            }
        }
        current_pos = abs_start + 1;
    }

    appids
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_games_xml() {
        let xml = r#"<?xml version="1.0"?><games><game>220</game><game>240</game><game type="junk">480</game></games>"#;
        let appids = parse_games_xml(xml);
        assert_eq!(appids, vec![220, 240, 480]);
    }

    #[test]
    fn test_parse_games_xml_empty_string_returns_empty() {
        assert!(parse_games_xml("").is_empty());
    }

    #[test]
    fn test_parse_games_xml_no_game_tags_returns_empty() {
        let xml = r#"<?xml version="1.0"?><root><other>123</other></root>"#;
        assert!(parse_games_xml(xml).is_empty());
    }

    #[test]
    fn test_parse_games_xml_skips_invalid_appid_content() {
        let xml = r#"<games><game>not_a_number</game><game>440</game></games>"#;
        assert_eq!(parse_games_xml(xml), vec![440]);
    }

    #[test]
    fn test_parse_games_xml_handles_negative_number_as_invalid() {
        // u32 cannot parse negative; the entry is dropped.
        let xml = r#"<games><game>-7</game><game>730</game></games>"#;
        assert_eq!(parse_games_xml(xml), vec![730]);
    }

    #[test]
    fn test_parse_games_xml_trims_whitespace_in_content() {
        let xml = "<games><game>  570  </game></games>";
        assert_eq!(parse_games_xml(xml), vec![570]);
    }

    #[test]
    fn test_parse_games_xml_handles_attribute_only_tag() {
        let xml = r#"<games><game id="1">10</game></games>"#;
        assert_eq!(parse_games_xml(xml), vec![10]);
    }

    #[test]
    fn test_parse_games_xml_unclosed_game_tag_is_skipped() {
        // No closing </game> after the opening tag — falls through to
        // the `current_pos = abs_start + 1` recovery branch and finds nothing else.
        let xml = "<games><game>123";
        assert!(parse_games_xml(xml).is_empty());
    }

    #[test]
    fn test_parse_games_xml_open_tag_without_closing_bracket_is_skipped() {
        // "<game" appears but there's no '>' anywhere — the inner
        // `xml[abs_start..].find('>')` returns None, recovery advances by 1.
        let xml = "prefix <game and then nothing";
        assert!(parse_games_xml(xml).is_empty());
    }

    #[test]
    fn test_parse_games_xml_multiple_games_wrappers_handled() {
        // Two `<games>` wrappers in sequence should both be skipped
        // without producing spurious entries.
        let xml = "<games></games><games><game>100</game></games>";
        assert_eq!(parse_games_xml(xml), vec![100]);
    }

    #[test]
    fn test_parse_games_xml_overflow_u32_is_skipped() {
        // Larger than u32::MAX — parse fails, entry skipped.
        let xml = "<games><game>9999999999999</game><game>20</game></games>";
        assert_eq!(parse_games_xml(xml), vec![20]);
    }

    #[cfg(target_os = "linux")]
    mod steam_client_path_tests {
        use super::super::*;
        use std::env;
        use std::fs;
        use std::path::{Path, PathBuf};
        use std::sync::Mutex;
        use std::time::{SystemTime, UNIX_EPOCH};

        // $HOME mutation is process-global; serialize across tests in this submodule.
        static ENV_LOCK: Mutex<()> = Mutex::new(());

        struct HomeScope {
            prev: Option<String>,
        }

        impl HomeScope {
            fn set(home: &Path) -> Self {
                let prev = env::var("HOME").ok();
                env::set_var("HOME", home);
                Self { prev }
            }

            fn unset() -> Self {
                let prev = env::var("HOME").ok();
                env::remove_var("HOME");
                Self { prev }
            }
        }

        impl Drop for HomeScope {
            fn drop(&mut self) {
                match &self.prev {
                    Some(v) => env::set_var("HOME", v),
                    None => env::remove_var("HOME"),
                }
            }
        }

        fn unique_root(label: &str) -> PathBuf {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            env::temp_dir().join(format!(
                "steamfetch-native-test-{}-{}-{}",
                label,
                std::process::id(),
                nanos
            ))
        }

        fn touch(path: &Path) {
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, b"stub").unwrap();
        }

        #[test]
        fn test_get_steam_client_path_returns_none_when_home_unset() {
            let _guard = ENV_LOCK.lock().unwrap();
            let _scope = HomeScope::unset();
            assert!(get_steam_client_path().is_none());
        }

        #[test]
        fn test_get_steam_client_path_returns_none_when_no_files_exist() {
            let _guard = ENV_LOCK.lock().unwrap();
            let root = unique_root("none");
            fs::create_dir_all(&root).unwrap();
            let _scope = HomeScope::set(&root);
            assert!(get_steam_client_path().is_none());
            let _ = fs::remove_dir_all(&root);
        }

        #[test]
        fn test_get_steam_client_path_prefers_sdk64() {
            let _guard = ENV_LOCK.lock().unwrap();
            let root = unique_root("sdk64");
            let _scope = HomeScope::set(&root);
            let sdk = root.join(".steam/sdk64/steamclient.so");
            let local = root.join(".local/share/Steam/linux64/steamclient.so");
            touch(&sdk);
            touch(&local);

            let found = get_steam_client_path().expect("sdk64 path should be returned");
            assert_eq!(found, sdk.to_string_lossy());

            let _ = fs::remove_dir_all(&root);
        }

        #[test]
        fn test_get_steam_client_path_falls_back_to_steam_linux64() {
            let _guard = ENV_LOCK.lock().unwrap();
            let root = unique_root("middle");
            let _scope = HomeScope::set(&root);
            let middle = root.join(".steam/steam/linux64/steamclient.so");
            touch(&middle);

            let found = get_steam_client_path().expect("steam/linux64 path should be returned");
            assert_eq!(found, middle.to_string_lossy());

            let _ = fs::remove_dir_all(&root);
        }

        #[test]
        fn test_get_steam_client_path_falls_back_to_local_share() {
            let _guard = ENV_LOCK.lock().unwrap();
            let root = unique_root("local");
            let _scope = HomeScope::set(&root);
            let local = root.join(".local/share/Steam/linux64/steamclient.so");
            touch(&local);

            let found = get_steam_client_path().expect("local/share/Steam path should be returned");
            assert_eq!(found, local.to_string_lossy());

            let _ = fs::remove_dir_all(&root);
        }

        #[test]
        fn test_try_new_returns_none_when_no_steam_client_found() {
            // Empty $HOME → no candidate path exists → matches the `None`
            // arm of `match get_steam_client_path()` and returns None.
            // Exercises the early-return branch with verbose=false (no log).
            let _guard = ENV_LOCK.lock().unwrap();
            let root = unique_root("none-no-verbose");
            fs::create_dir_all(&root).unwrap();
            let _scope = HomeScope::set(&root);

            assert!(NativeSteamClient::try_new(false).is_none());

            let _ = fs::remove_dir_all(&root);
        }

        #[test]
        fn test_try_new_returns_none_when_no_steam_client_found_verbose() {
            // Same early-return path with verbose=true so the
            // "[verbose] Steam client not found" eprintln branch runs.
            let _guard = ENV_LOCK.lock().unwrap();
            let root = unique_root("none-verbose");
            fs::create_dir_all(&root).unwrap();
            let _scope = HomeScope::set(&root);

            assert!(NativeSteamClient::try_new(true).is_none());

            let _ = fs::remove_dir_all(&root);
        }

        #[test]
        fn test_try_new_returns_none_when_steam_client_fails_to_load() {
            // A stub file at the sdk64 path satisfies `get_steam_client_path`,
            // but `Library::new` fails to dlopen non-ELF bytes — the function
            // hits the `Err(e) => return None` arm of the load match.
            // verbose=false skips the eprintln branch.
            let _guard = ENV_LOCK.lock().unwrap();
            let root = unique_root("load-fail");
            let _scope = HomeScope::set(&root);
            let stub = root.join(".steam/sdk64/steamclient.so");
            touch(&stub);

            assert!(NativeSteamClient::try_new(false).is_none());

            let _ = fs::remove_dir_all(&root);
        }

        #[test]
        fn test_try_new_returns_none_when_steam_client_fails_to_load_verbose() {
            // Same load-failure path, verbose=true so both verbose branches
            // ("Found Steam client at" and "Failed to load Steam client")
            // execute.
            let _guard = ENV_LOCK.lock().unwrap();
            let root = unique_root("load-fail-verbose");
            let _scope = HomeScope::set(&root);
            let stub = root.join(".steam/sdk64/steamclient.so");
            touch(&stub);

            assert!(NativeSteamClient::try_new(true).is_none());

            let _ = fs::remove_dir_all(&root);
        }

        // Pick the first existing system shared library from a portable list
        // of common candidates. Returns None when none are present (the test
        // that uses this will then no-op rather than fail spuriously on
        // platforms where these libraries live elsewhere).
        fn find_loadable_system_lib() -> Option<PathBuf> {
            let candidates = [
                "/lib/x86_64-linux-gnu/libdl.so.2",
                "/lib/x86_64-linux-gnu/libpthread.so.0",
                "/lib/x86_64-linux-gnu/libc.so.6",
                "/usr/lib/x86_64-linux-gnu/libdl.so.2",
                "/usr/lib/x86_64-linux-gnu/libpthread.so.0",
                "/usr/lib/x86_64-linux-gnu/libc.so.6",
                "/lib64/libdl.so.2",
                "/lib64/libpthread.so.0",
                "/lib64/libc.so.6",
            ];
            candidates
                .into_iter()
                .map(PathBuf::from)
                .find(|p| p.exists())
        }

        #[test]
        fn test_try_new_returns_none_when_create_interface_symbol_missing() {
            // `Library::new` succeeds (we symlink to a real, loadable system
            // library), but `lib.get(b"CreateInterface")` then fails because
            // the loaded library does not export that symbol — exercises the
            // `Err(e) => return None` arm of the CreateInterface match
            // (lines ~152–158), a path the existing load-failure tests can't
            // reach because their stub bytes never get past `Library::new`.
            // verbose=false skips the eprintln.
            let _guard = ENV_LOCK.lock().unwrap();

            // Skip when no candidate system library is available — this
            // platform isn't suitable for this test, treat it as a no-op
            // rather than a failure.
            let Some(real_lib) = find_loadable_system_lib() else {
                return;
            };

            let root = unique_root("create-iface-missing");
            let _scope = HomeScope::set(&root);
            let target = root.join(".steam/sdk64/steamclient.so");
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            std::os::unix::fs::symlink(&real_lib, &target)
                .expect("symlink to system lib should succeed in tmp dir");

            assert!(NativeSteamClient::try_new(false).is_none());

            let _ = fs::remove_dir_all(&root);
        }

        #[test]
        fn test_try_new_returns_none_when_create_interface_symbol_missing_verbose() {
            // Same CreateInterface-missing path with verbose=true — exercises
            // both the "Found Steam client at" log and the "Failed to get
            // CreateInterface" verbose log inside the symbol-lookup error
            // arm.
            let _guard = ENV_LOCK.lock().unwrap();

            let Some(real_lib) = find_loadable_system_lib() else {
                return;
            };

            let root = unique_root("create-iface-missing-verbose");
            let _scope = HomeScope::set(&root);
            let target = root.join(".steam/sdk64/steamclient.so");
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            std::os::unix::fs::symlink(&real_lib, &target)
                .expect("symlink to system lib should succeed in tmp dir");

            assert!(NativeSteamClient::try_new(true).is_none());

            let _ = fs::remove_dir_all(&root);
        }

        #[test]
        fn test_homescope_drop_removes_home_when_prev_was_none() {
            // The other tests in this module run with $HOME already set, so
            // HomeScope::Drop's `Some(v)` arm is the only one ever hit. Force
            // $HOME to be unset before HomeScope::set so prev = None,
            // exercising the `None => env::remove_var("HOME")` branch on Drop.
            let _guard = ENV_LOCK.lock().unwrap();
            let outer_prev = env::var("HOME").ok();
            env::remove_var("HOME");

            let root = unique_root("homescope-none-arm");
            fs::create_dir_all(&root).unwrap();
            {
                let _scope = HomeScope::set(&root);
                assert_eq!(env::var("HOME").unwrap(), root.to_string_lossy());
            }

            // Drop ran the `None => env::remove_var("HOME")` branch.
            assert!(env::var("HOME").is_err());

            match outer_prev {
                Some(v) => env::set_var("HOME", v),
                None => env::remove_var("HOME"),
            }

            let _ = fs::remove_dir_all(&root);
        }
    }
}
