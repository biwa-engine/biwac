use biwac_base::{BiwacError, PackageNameError, PackageVersionError};
use colored::Colorize;

#[derive(Debug)]
pub enum PkgMetadataLoadError {
    MetadataFileNotFound,
    InvalidFormat(String),
    PackageNameError(PackageNameError),
    PackageVersionError(PackageVersionError),
}

impl BiwacError for PkgMetadataLoadError {
    fn print_error_message(&self, srcs: &biwac_base::SourceHolder) {
        match self {
            Self::MetadataFileNotFound => {
                println!(
                    r#"{} Metadata file `{}` not found."#,
                    "Error:".red(),
                    crate::METADATA_FILE_NAME
                )
            }
            Self::InvalidFormat(e) => {
                println!(
                    r#"{} Invalid metadata file `{}` format.
    --> {e}"#,
                    "Error:".red(),
                    crate::METADATA_FILE_NAME
                )
            }
            Self::PackageNameError(e) => {
                e.print_error_message(srcs);
            }
            Self::PackageVersionError(e) => {
                e.print_error_message(srcs);
            }
        }
    }
}
