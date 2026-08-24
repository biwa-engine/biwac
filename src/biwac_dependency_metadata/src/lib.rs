mod error;
mod external_package;
mod metadata;

pub use error::DepMetadataError;
pub use external_package::ExternalPackage;
pub use metadata::DepMetadata;
pub use metadata::module_view::{
    DepMetadataModuleView, ExternalChildKind, ExternalChildRef, PackageModuleView,
};
