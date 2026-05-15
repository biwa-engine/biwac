use biwac_base::{BiwacError, PackageNameError, PackageVersionError};
use colored::Colorize;

#[derive(Debug)]
pub enum PkgMetadataLoadError {
    MetadataFileNotFound,
    InvalidFormat {
        err_msg: String,
        line: usize,
        column: usize,
    },
    PackageNameError(PackageNameError),
    PackageVersionError(PackageVersionError),
}

impl BiwacError for PkgMetadataLoadError {
    fn print_error_message(&self, ctx: &biwac_base::ErrorContext) {
        match self {
            Self::MetadataFileNotFound => {
                println!(
                    r#"{} Package profile `{}` not found."#,
                    "Error:".red(),
                    biwac_base::METADATA_FILE_NAME
                )
            }
            Self::InvalidFormat {
                err_msg,
                line,
                column,
            } => {
                println!(
                    r#"{} Invalid package profile format.
    --> {}:{}:{}
    --> {err_msg}"#,
                    "Error:".red(),
                    biwac_base::METADATA_FILE_NAME,
                    line,
                    column
                )
            }
            Self::PackageNameError(e) => {
                e.print_error_message(ctx);
            }
            Self::PackageVersionError(e) => {
                e.print_error_message(ctx);
            }
        }
    }
}
