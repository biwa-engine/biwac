use std::fmt::Display;

#[derive(Debug)]
pub enum FetchError {
    Hub(biwa_hub_client::HubClientError),
    NoMatchingVersion {
        name: String,
        min: String,
        max: Option<String>,
    },
    Io(std::io::Error),
    /// `git` コマンドが非 0 で終了した。
    GitFailed { step: &'static str, status: String },
}

impl Display for FetchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Hub(e) => write!(f, "{e}"),
            Self::NoMatchingVersion { name, min, max } => write!(
                f,
                "no version of `{name}` satisfies {min}..{}",
                max.clone().unwrap_or_else(|| "*".to_string())
            ),
            Self::Io(e) => write!(f, "{e}"),
            Self::GitFailed { step, status } => {
                write!(f, "`git {step}` failed ({status})")
            }
        }
    }
}

impl std::error::Error for FetchError {}

impl From<biwa_hub_client::HubClientError> for FetchError {
    fn from(e: biwa_hub_client::HubClientError) -> Self {
        Self::Hub(e)
    }
}

impl From<std::io::Error> for FetchError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}
