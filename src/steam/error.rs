use std::fmt;

#[derive(Debug)]
pub enum SteamApiError {
    PrivateProfile,
    RateLimited,
    Timeout,
    NetworkError(String),
    InvalidApiKey,
    PlayerNotFound,
    ApiError { status: u16, message: String },
}

impl fmt::Display for SteamApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PrivateProfile => write!(
                f,
                "This Steam profile is private.\n\
                 To use steamfetch, set your profile to public:\n\
                 \n\
                 1. Open Steam -> Profile -> Edit Profile\n\
                 2. Set 'My profile' to 'Public'\n\
                 3. Set 'Game details' to 'Public'"
            ),
            Self::RateLimited => write!(
                f,
                "Steam API rate limit reached. Please wait a moment and try again."
            ),
            Self::Timeout => write!(
                f,
                "Request timed out. Check your connection or increase timeout with --timeout."
            ),
            Self::NetworkError(msg) => write!(f, "Network error: {}", msg),
            Self::InvalidApiKey => write!(
                f,
                "Invalid Steam API key. Please check your API key configuration."
            ),
            Self::PlayerNotFound => write!(f, "Player not found. Please check your Steam ID."),
            Self::ApiError { status, message } => {
                write!(f, "Steam API error (HTTP {}): {}", status, message)
            }
        }
    }
}

impl std::error::Error for SteamApiError {}

impl SteamApiError {
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::RateLimited | Self::Timeout | Self::NetworkError(_)
        )
    }
}
