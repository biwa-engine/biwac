mod error;
mod metadata;

pub use error::DepMetadataError;
pub use metadata::DepMetadata;
pub use metadata::module_view::{
    DepMetadataModuleView, ExternalChildKind, ExternalChildRef, PackageModuleView,
};
