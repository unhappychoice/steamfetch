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
