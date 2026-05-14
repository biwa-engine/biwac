use ariadne::{Color, Label, Report, ReportKind, Source};

use biwac_base::BiwacError;
use biwac_span::Span;

#[derive(Debug, Clone)]
pub enum TokenizeError {
    DoubleQuoteCloseNotFound { span: Span },
}

impl BiwacError for TokenizeError {
    fn print_error_message(
        &self,
        _metadata: &biwac_base::MetadataHolder,
        srcs: &biwac_base::SourceHolder,
    ) {
        match self {
            Self::DoubleQuoteCloseNotFound { span } => {
                let modsrc = srcs.mods.get(&span.module()).unwrap();

                let file_name = modsrc.modu.file_name();

                Report::build(
                    ReportKind::Error,
                    (file_name.as_str(), span.begin()..span.begin() + 1),
                )
                .with_message("Closing double quotation ( `\"` ) expected, but not found.")
                .with_label(
                    Label::new((file_name.as_str(), span.begin()..span.begin() + 1))
                        .with_message("`\"` expected")
                        .with_color(Color::Red),
                )
                .finish()
                .print((file_name.as_str(), Source::from(&modsrc.src)))
                .unwrap();
            }
        }
    }
}
