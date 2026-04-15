use biwac_base::{BiwacError, ModPath};
use biwac_lexer::TokenizeError;
use biwac_parser::ParseError;

use colored::Colorize;

#[derive(Debug, Clone)]
pub enum PkgLoadError<'src> {
    RootModuleNotFound,
    LexError {
        modpath: ModPath,
        err: Box<TokenizeError>,
    },
    ParseError {
        modpath: ModPath,
        err: Box<ParseError<'src>>,
    },
}

impl BiwacError for PkgLoadError<'_> {
    fn print_error_message(
        &self,
        metadata: &biwac_base::MetadataHolder,
        srcs: &biwac_base::SourceHolder,
    ) {
        match self {
            Self::RootModuleNotFound => {
                println!(
                    r#"{} Root module not found.
One of `{}.{}` or `{}.{}` needed in a package."#,
                    "Error:".red(),
                    biwac_base::BIWA_LIBRARY_PACKAGE_ROOT_MODULE_NAME,
                    biwac_base::BIWA_EXTENSION,
                    biwac_base::BIWA_BINARY_PACKAGE_ROOT_MODULE_NAME,
                    biwac_base::BIWA_EXTENSION,
                )
            }
            Self::LexError { err, .. } => err.print_error_message(metadata, srcs),
            Self::ParseError { err, .. } => {
                err.print_error_message(metadata, srcs);
            }
        }
    }
}
