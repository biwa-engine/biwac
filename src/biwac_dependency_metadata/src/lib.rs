mod error;
mod external_package;
mod metadata;

pub use error::DepMetadataError;
pub use external_package::ExternalPackage;
pub use metadata::module_view::{
    DepMetadataModuleView, ExternalChildKind, ExternalChildRef, PackageModuleView,
};
pub use metadata::{BIWAC_DEPENDENCY_METADATA_FORMAT_VERSION, DepMetadata};
