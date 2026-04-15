mod error;
mod package;
mod span;
mod src;

pub use error::{BiwacError, ErrorHolder, print_error_finish_message};
pub use package::{
    DependedPackage, MetadataHolder, PackageMetadata, PackageName, PackageNameError,
    PackageVersion, PackageVersionError,
};
pub use span::{ModPath, SSpan, Span};
pub use src::{ModId, ModSource, SourceHolder};

pub const BIWA_BINARY_PACKAGE_ROOT_MODULE_NAME: &str = "main";
pub const BIWA_LIBRARY_PACKAGE_ROOT_MODULE_NAME: &str = "lib";
pub const BIWA_EXTENSION: &str = "biwa";

pub const BIWA_BUILD_DIRECTORY_NAME: &str = ".biwa_build";

pub const METADATA_FILE_NAME: &str = "biwa-package.json";
