use biwac_base::{BiwacError, ModPath};
use biwac_lexer::TokenizeError;
use biwac_parser::ParseError;

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
            Self::LexError { err, .. } => err.print_error_message(metadata, srcs),
            _ => todo!(),
        }
    }
}
