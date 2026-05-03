use thiserror::Error;

#[derive(Debug, Error)]
pub enum SteamApiError {
    #[error(
        "This Steam profile is private.\n\
         To use steamfetch, set your profile to public:\n\
         \n\
         1. Open Steam -> Profile -> Edit Profile\n\
         2. Set 'My profile' to 'Public'\n\
         3. Set 'Game details' to 'Public'"
    )]
    PrivateProfile,

    #[error("Steam API rate limit reached. Please wait a moment and try again.")]
    RateLimited,

    #[error("Request timed out. Check your connection or increase timeout with --timeout.")]
    Timeout,

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Invalid Steam API key. Please check your API key configuration.")]
    InvalidApiKey,

    #[error("Player not found. Please check your Steam ID.")]
    PlayerNotFound,

    #[error("Steam API error (HTTP {status}): {message}")]
    ApiError { status: u16, message: String },
}

impl SteamApiError {
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::RateLimited
                | Self::Timeout
                | Self::NetworkError(_)
                | Self::ApiError {
                    status: 500..=599,
                    ..
                }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_retryable_rate_limited() {
        assert!(SteamApiError::RateLimited.is_retryable());
    }

    #[test]
    fn test_is_retryable_timeout() {
        assert!(SteamApiError::Timeout.is_retryable());
    }

    #[test]
    fn test_is_retryable_network_error() {
        assert!(SteamApiError::NetworkError("connection reset".to_string()).is_retryable());
    }

    #[test]
    fn test_is_retryable_api_error_5xx() {
        assert!(SteamApiError::ApiError {
            status: 500,
            message: "internal".to_string(),
        }
        .is_retryable());
        assert!(SteamApiError::ApiError {
            status: 503,
            message: "unavailable".to_string(),
        }
        .is_retryable());
        assert!(SteamApiError::ApiError {
            status: 599,
            message: "edge".to_string(),
        }
        .is_retryable());
    }

    #[test]
    fn test_is_retryable_api_error_4xx_not_retryable() {
        assert!(!SteamApiError::ApiError {
            status: 400,
            message: "bad request".to_string(),
        }
        .is_retryable());
        assert!(!SteamApiError::ApiError {
            status: 404,
            message: "not found".to_string(),
        }
        .is_retryable());
        assert!(!SteamApiError::ApiError {
            status: 499,
            message: "edge".to_string(),
        }
        .is_retryable());
    }

    #[test]
    fn test_is_retryable_api_error_outside_5xx_range() {
        assert!(!SteamApiError::ApiError {
            status: 600,
            message: "weird".to_string(),
        }
        .is_retryable());
    }

    #[test]
    fn test_is_retryable_private_profile_not_retryable() {
        assert!(!SteamApiError::PrivateProfile.is_retryable());
    }

    #[test]
    fn test_is_retryable_invalid_api_key_not_retryable() {
        assert!(!SteamApiError::InvalidApiKey.is_retryable());
    }

    #[test]
    fn test_is_retryable_player_not_found_not_retryable() {
        assert!(!SteamApiError::PlayerNotFound.is_retryable());
    }

    #[test]
    fn test_display_private_profile_mentions_public() {
        let msg = SteamApiError::PrivateProfile.to_string();
        assert!(msg.contains("private"));
        assert!(msg.contains("Public"));
    }

    #[test]
    fn test_display_rate_limited() {
        let msg = SteamApiError::RateLimited.to_string();
        assert!(msg.contains("rate limit"));
    }

    #[test]
    fn test_display_timeout() {
        let msg = SteamApiError::Timeout.to_string();
        assert!(msg.contains("timed out"));
    }

    #[test]
    fn test_display_network_error_includes_inner() {
        let msg = SteamApiError::NetworkError("dns failure".to_string()).to_string();
        assert!(msg.contains("Network error"));
        assert!(msg.contains("dns failure"));
    }

    #[test]
    fn test_display_invalid_api_key() {
        let msg = SteamApiError::InvalidApiKey.to_string();
        assert!(msg.contains("Invalid Steam API key"));
    }

    #[test]
    fn test_display_player_not_found() {
        let msg = SteamApiError::PlayerNotFound.to_string();
        assert!(msg.contains("Player not found"));
    }

    #[test]
    fn test_display_api_error_includes_status_and_message() {
        let msg = SteamApiError::ApiError {
            status: 502,
            message: "bad gateway".to_string(),
        }
        .to_string();
        assert!(msg.contains("502"));
        assert!(msg.contains("bad gateway"));
    }
}
