mod error;
mod span;
mod src;

pub use error::BiwacError;
pub use span::{ModPath, Pos, Span};

pub const BIWA_BINARY_PACKAGE_ROOT_MODULE_NAME: &str = "main";
pub const BIWA_LIBRARY_PACKAGE_ROOT_MODULE_NAME: &str = "lib";
pub const BIWA_EXTENSION: &str = "biwa";
