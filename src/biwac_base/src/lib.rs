mod error;
mod ident;
mod module;
mod package;
mod src;

pub use error::{BiwacError, ErrorContext, ErrorHolder, print_error_finish_message};
pub use ident::{IdentInterner, InternedIdent};
pub use module::ModPath;
pub use package::{
    DependedPackage, MetadataHolder, PackageId, PackageKind, PackageMetadata, PackageName,
    PackageNameError, PackageVersion, PackageVersionError,
};
pub use src::{ModId, ModSource, SourceHolder};

pub const BIWA_BINARY_PACKAGE_ROOT_MODULE_NAME: &str = "main";
pub const BIWA_LIBRARY_PACKAGE_ROOT_MODULE_NAME: &str = "lib";
pub const BIWA_EXTENSION: &str = "biwa";

pub const BIWA_BUILD_DIRECTORY_NAME: &str = ".biwa_build";

pub const METADATA_FILE_NAME: &str = "biwa-package.json";
