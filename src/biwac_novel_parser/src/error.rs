use biwac_base::BiwacError;
use biwac_span::Span;

use crate::token::{CharKind, NCodeTkKindName, NCodeToken};

use ariadne::{Color, Label, Report, ReportKind, Source};

#[derive(Debug, Clone)]
pub enum NovelParseError {
    InvalidToken {
        expecteds: Vec<NCodeTkKindName>,
        found: Box<NCodeToken>,
    },
    InvalidChar {
        expecteds: Vec<CharKind>,
        found: CharKind,
        span: Span,
    },
    InvalidLineEnd {
        expecteds: Vec<NCodeTkKindName>,
        span: Span,
    },
    LineEndExpected {
        found: Box<NCodeToken>,
    },
    GeneralCommandLineOnlyPrefix {
        span: Span,
    },
    InvalidCloseLine {
        span: Span,
    },
    CloseLineExpected {
        span: Span,
    },
    StringLiteralNotClosed {
        span: Span,
    },
}

impl BiwacError for NovelParseError {
    fn print_error_message(&self, ctx: &biwac_base::ErrorContext) {
        match self {
            Self::InvalidToken { expecteds, found } => {
                let modsrc = ctx.srcs.mods.get(&found.span.module()).unwrap();

                let file_name = modsrc.modu.file_name();
                let begin = modsrc.char_offset(found.span.begin());
                let end = modsrc.char_offset(found.span.end());

                Report::build(ReportKind::Error, (file_name.as_str(), begin..end))
                    .with_message("Unexpected token found.")
                    .with_label(
                        Label::new((file_name.as_str(), begin..end))
                            .with_message(if expecteds.is_empty() {
                                format!(
                                    "Another token expected, but found {}.",
                                    found.kind.pattern()
                                )
                            } else if expecteds.len() == 1 {
                                format!(
                                    "Expected {}, but found {}.",
                                    format_token_kinds(expecteds),
                                    found.kind.pattern()
                                )
                            } else {
                                format!(
                                    "Expected one of {}, but found {}.",
                                    format_token_kinds(expecteds),
                                    found.kind.pattern()
                                )
                            })
                            .with_color(Color::Red),
                    )
                    .finish()
                    .print((file_name.as_str(), Source::from(&modsrc.src)))
                    .unwrap();
            }
            Self::InvalidChar {
                expecteds,
                found,
                span,
            } => {
                let modsrc = ctx.srcs.mods.get(&span.module()).unwrap();

                let file_name = modsrc.modu.file_name();
                let begin = modsrc.char_offset(span.begin());
                let end = modsrc.char_offset(span.end());

                Report::build(ReportKind::Error, (file_name.as_str(), begin..end))
                    .with_message("Unexpected character found.")
                    .with_label(
                        Label::new((file_name.as_str(), begin..end))
                            .with_message(if expecteds.is_empty() {
                                format!(
                                    "Another character expected, but found {}.",
                                    found.pattern()
                                )
                            } else if expecteds.len() == 1 {
                                format!(
                                    "Expected {}, but found {}.",
                                    format_char_kinds(expecteds),
                                    found.pattern()
                                )
                            } else {
                                format!(
                                    "Expected one of {}, but found {}.",
                                    format_char_kinds(expecteds),
                                    found.pattern()
                                )
                            })
                            .with_color(Color::Red),
                    )
                    .finish()
                    .print((file_name.as_str(), Source::from(&modsrc.src)))
                    .unwrap();
            }
            Self::StringLiteralNotClosed { span } => {
                let modsrc = ctx.srcs.mods.get(&span.module()).unwrap();

                let file_name = modsrc.modu.file_name();
                let begin = modsrc.char_offset(span.begin());
                let end = modsrc.char_offset(span.end());

                Report::build(ReportKind::Error, (file_name.as_str(), begin..end))
                    .with_message("Closing double quotation ( `\"` ) expected, but not found.")
                    .with_label(
                        Label::new((file_name.as_str(), begin..end))
                            .with_message("`\"` expected")
                            .with_color(Color::Red),
                    )
                    .with_note("a string literal in a scene block must be closed on the same line")
                    .finish()
                    .print((file_name.as_str(), Source::from(&modsrc.src)))
                    .unwrap();
            }
            _ => todo!(),
        }
    }
}

fn format_token_kinds(kinds: &[NCodeTkKindName]) -> String {
    if kinds.is_empty() {
        "".to_string()
    } else if kinds.len() == 1 {
        kinds[0].to_string()
    } else if kinds.len() == 2 {
        format!("{} or {}", kinds.first().unwrap(), kinds.last().unwrap())
    } else {
        format!(
            "{} or {}",
            kinds[..kinds.len() - 1]
                .iter()
                .map(|kind| kind.to_string())
                .collect::<Vec<_>>()
                .join(", "),
            kinds[kinds.len() - 1..][0]
        )
    }
}

fn format_char_kinds(kinds: &[CharKind]) -> String {
    if kinds.is_empty() {
        "".to_string()
    } else if kinds.len() == 1 {
        kinds[0].pattern().to_string()
    } else if kinds.len() == 2 {
        format!(
            "{} or {}",
            kinds.first().unwrap().pattern(),
            kinds.last().unwrap().pattern()
        )
    } else {
        format!(
            "{} or {}",
            kinds[..kinds.len() - 1]
                .iter()
                .map(|kind| kind.pattern().to_string())
                .collect::<Vec<_>>()
                .join(", "),
            kinds[kinds.len() - 1..][0].pattern()
        )
    }
}
