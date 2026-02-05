use anyhow::{Context, Result};
use libloading::{Library, Symbol};
use std::ffi::CString;
use std::os::raw::{c_char, c_void};
use steamworks::sys;

type CreateInterfaceFn = unsafe extern "C" fn(*const c_char, *mut i32) -> *mut c_void;

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
    pub fn try_new() -> Option<Self> {
        let steam_path = std::env::var("HOME").ok()? + "/.steam/sdk64/steamclient.so";

        unsafe {
            let lib = Library::new(&steam_path).ok()?;
            let create_interface: Symbol<CreateInterfaceFn> = lib.get(b"CreateInterface").ok()?;

            // Get SteamClient interface
            let version = CString::new("SteamClient021").ok()?;
            let mut return_code: i32 = 0;
            let client_ptr = create_interface(version.as_ptr(), &mut return_code);
            if client_ptr.is_null() {
                return None;
            }
            let client = client_ptr as *mut sys::ISteamClient;

            // Create pipe
            let pipe = sys::SteamAPI_ISteamClient_CreateSteamPipe(client);
            if pipe == 0 {
                return None;
            }

            // Connect to global user
            let user = sys::SteamAPI_ISteamClient_ConnectToGlobalUser(client, pipe);
            if user == 0 {
                sys::SteamAPI_ISteamClient_BReleaseSteamPipe(client, pipe);
                return None;
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
}
