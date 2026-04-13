mod error;
mod package;
mod span;
mod src;

pub use error::BiwacError;
pub use package::{
    DependedPackage, PackageMetadata, PackageName, PackageNameError, PackageVersion,
    PackageVersionError,
};
pub use span::{ModPath, Pos, SSpan, Span};
pub use src::{FileId, ModSource, SourceHolder};

pub const BIWA_BINARY_PACKAGE_ROOT_MODULE_NAME: &str = "main";
pub const BIWA_LIBRARY_PACKAGE_ROOT_MODULE_NAME: &str = "lib";
pub const BIWA_EXTENSION: &str = "biwa";

pub const BIWA_BUILD_DIRECTORY_NAME: &str = ".biwa_build";
