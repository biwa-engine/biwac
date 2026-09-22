use biwac_base::{BiwacError, ModPath};

use colored::Colorize;

#[derive(Debug)]
pub enum PkgLoadError<'src> {
    RootModuleDuplicated,
    RootModuleNotFound,
    /// 字句解析・構文解析いずれかの失敗。`SourceParser::parse` はどちらの
    /// 段で失敗したかを区別せず `Box<dyn BiwacError>` に包んで返すので、
    /// ここでも 1 種類にまとめている (`print_error_message` に委譲するだけで
    /// 種別を見る場所はどこにも無い)。
    ParseError {
        modpath: ModPath,
        err: Box<dyn BiwacError + 'src>,
    },
}

impl BiwacError for PkgLoadError<'_> {
    fn print_error_message(&self, ctx: &biwac_base::ErrorContext) {
        match self {
            Self::RootModuleDuplicated => {
                println!(
                    r#"{} Root module duplicated.
Both of `{}.{}` or `{}.{}` exist in a package.
Only one of them can exist."#,
                    "Error:".red(),
                    biwac_base::BIWA_LIBRARY_PACKAGE_ROOT_MODULE_NAME,
                    biwac_base::BIWA_EXTENSION,
                    biwac_base::BIWA_BINARY_PACKAGE_ROOT_MODULE_NAME,
                    biwac_base::BIWA_EXTENSION,
                )
            }
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
            Self::ParseError { err, .. } => {
                err.print_error_message(ctx);
            }
        }
    }
}
