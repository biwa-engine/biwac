use ariadne::{Color, Label, Report, ReportKind, Source};

use biwac_base::BiwacError;
use biwac_span::Span;

#[derive(Debug, Clone)]
pub enum TokenizeError {
    DoubleQuoteCloseNotFound {
        span: Span,
    },

    /// `{{` の後ろに、同じ行で何かが書かれている。
    ///
    /// `{{ ... }}` の中身は Biwa の文法ではないので、閉じの判定に中身は使えない
    /// (native code に `}}` が現れても不思議ではない)。
    /// そのため閉じは「空白文字を除いて `}}` で始まる行」だけと決めてあり、
    /// 1 行で書かれた `{{ ... }}` は永遠に閉じない。
    DslOpenNotAtLineEnd {
        span: Span,
    },

    /// `{{` に対応する閉じの行が見つからないままファイルが終わった。
    DslCloseNotFound {
        span: Span,
    },
}

impl BiwacError for TokenizeError {
    fn print_error_message(&self, ctx: &biwac_base::ErrorContext) {
        match self {
            Self::DoubleQuoteCloseNotFound { span } => {
                let modsrc = ctx.srcs.mods.get(&span.module()).unwrap();

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

            Self::DslOpenNotAtLineEnd { span } => {
                let modsrc = ctx.srcs.mods.get(&span.module()).unwrap();

                let file_name = modsrc.modu.file_name();

                Report::build(
                    ReportKind::Error,
                    (file_name.as_str(), span.begin()..span.end()),
                )
                .with_message("`{{` must be the last thing on its line.")
                .with_label(
                    Label::new((file_name.as_str(), span.begin()..span.end()))
                        .with_message("the body must start on the next line")
                        .with_color(Color::Red),
                )
                .with_note(
                    "a `{{ ... }}` block is closed by a line starting with `}}`, \
                         so it cannot be written on a single line",
                )
                .finish()
                .print((file_name.as_str(), Source::from(&modsrc.src)))
                .unwrap();
            }

            Self::DslCloseNotFound { span } => {
                let modsrc = ctx.srcs.mods.get(&span.module()).unwrap();

                let file_name = modsrc.modu.file_name();

                Report::build(
                    ReportKind::Error,
                    (file_name.as_str(), span.begin()..span.end()),
                )
                .with_message("Closing `}}` expected, but not found.")
                .with_label(
                    Label::new((file_name.as_str(), span.begin()..span.end()))
                        .with_message("this block is never closed")
                        .with_color(Color::Red),
                )
                .with_note("`}}` must be at the start of a line (leading whitespace is allowed)")
                .finish()
                .print((file_name.as_str(), Source::from(&modsrc.src)))
                .unwrap();
            }
        }
    }
}
