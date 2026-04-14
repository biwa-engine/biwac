use biwac_base::{BiwacError, ModPath};
use biwac_lexer::TokenizeError;
use biwac_parser::ParseError;

#[derive(Debug, Clone)]
pub enum PkgLoadError {
    RootModuleNotFound,
    LexError {
        modpath: ModPath,
        err: Box<TokenizeError>,
    },
    ParseError {
        modpath: ModPath,
        err: Box<ParseError>,
    },
}

impl BiwacError for PkgLoadError {
    fn error_message(&self, srcs: &biwac_base::SourceHolder) -> String {
        match self {
            Self::LexError { err, .. } => err.error_message(srcs),
            _ => todo!(),
        }
    }
}
