use std::collections::BTreeMap;

use ariadne::{Color, Label, Report, ReportKind};
use colored::Colorize;

use crate::{IdentInterner, MetadataHolder, ModId, SourceHolder};

pub trait BiwacError {
    fn print_error_message(&self, ctx: &ErrorContext);
}

#[derive(Debug)]
pub struct ErrorContext<'a> {
    pub metadata: &'a MetadataHolder,
    pub srcs: &'a SourceHolder,
    pub interner: &'a IdentInterner,
}

impl<'a> ErrorContext<'a> {
    /// ソース抜粋つきの診断を組み立てる。
    ///
    /// ラベルを 1 つも置けなかった場合 (span が外部パッケージや
    /// ダミーを指している場合) はメッセージだけを出す。
    pub fn diagnostic(&'a self, message: impl Into<String>) -> Diagnostic<'a> {
        Diagnostic {
            ctx: self,
            message: message.into(),
            labels: Vec::new(),
            notes: Vec::new(),
        }
    }
}

/// ラベルを置く位置。
///
/// [`biwac_span::Span`] は `biwac_base` から見えないので、
/// 分解した値で受ける。
#[derive(Debug, Clone, Copy)]
pub struct DiagSpan {
    pub mod_id: ModId,
    pub begin: usize,
    pub end: usize,
}

impl DiagSpan {
    pub fn new(mod_id: ModId, begin: usize, end: usize) -> Self {
        Self { mod_id, begin, end }
    }
}

/// ariadne でソース抜粋つきのエラーを出すための組み立て器。
///
/// 各エラー型が同じ定型を書かなくて済むようにまとめてある。
/// ラベルは複数のファイルに跨ってよい (別モジュールの重複定義など)。
pub struct Diagnostic<'a> {
    ctx: &'a ErrorContext<'a>,
    message: String,
    labels: Vec<(DiagSpan, String, Color)>,
    notes: Vec<String>,
}

impl<'a> Diagnostic<'a> {
    /// 主たるラベル。赤で表示する。最初に置いたものが報告位置になる。
    pub fn label(mut self, span: DiagSpan, text: impl Into<String>) -> Self {
        self.labels.push((span, text.into(), Color::Red));
        self
    }

    /// 補助のラベル。「先に定義された場所」などに使う。
    pub fn sub_label(mut self, span: DiagSpan, text: impl Into<String>) -> Self {
        self.labels.push((span, text.into(), Color::Blue));
        self
    }

    /// `span` が `None` のときは何も置かない。
    pub fn label_opt(self, span: Option<DiagSpan>, text: impl Into<String>) -> Self {
        match span {
            Some(span) => self.label(span, text),
            None => self,
        }
    }

    pub fn sub_label_opt(self, span: Option<DiagSpan>, text: impl Into<String>) -> Self {
        match span {
            Some(span) => self.sub_label(span, text),
            None => self,
        }
    }

    pub fn note(mut self, text: impl Into<String>) -> Self {
        self.notes.push(text.into());
        self
    }

    pub fn print(self) {
        // 自パッケージのソースを持っているものだけラベルにできる。
        // 外部パッケージのシンボルやダミー span はここで落ちる。
        let placed: Vec<((String, std::ops::Range<usize>), String, Color)> = self
            .labels
            .iter()
            .filter_map(|(span, text, color)| {
                let modsrc = self.ctx.srcs.mods.get(&span.mod_id)?;
                let begin = modsrc.char_offset(span.begin);
                let end = modsrc.char_offset(span.end).max(begin);
                Some(((modsrc.modu.file_name(), begin..end), text.clone(), *color))
            })
            .collect();

        let Some((primary_id, _, _)) = placed.first().cloned() else {
            // 位置を示せないなら、せめてメッセージと補足は出す。
            eprintln!("{} {}", "Error:".red().bold(), self.message);
            for note in &self.notes {
                eprintln!("  note: {note}");
            }
            return;
        };

        // ariadne はラベルを渡された順に並べるので、位置の昇順にしておく。
        // 並んでいないと、同じファイルの中でも抜粋が分割されて読みにくくなる。
        let mut placed = placed;
        placed.sort_by(|(a, _, _), (b, _, _)| a.0.cmp(&b.0).then(a.1.start.cmp(&b.1.start)));

        let mut report = Report::build(ReportKind::Error, primary_id).with_message(&self.message);

        for (id, text, color) in &placed {
            report =
                report.with_label(Label::new(id.clone()).with_message(text).with_color(*color));
        }
        for note in &self.notes {
            report = report.with_note(note);
        }

        // ラベルが跨るファイルの分だけソースを渡す。
        // 同じファイルを 2 度渡すと ariadne 側で取り違えるので畳んでおく。
        let mut sources: BTreeMap<String, &str> = BTreeMap::new();
        for (span, _, _) in &self.labels {
            if let Some(modsrc) = self.ctx.srcs.mods.get(&span.mod_id) {
                sources.insert(modsrc.modu.file_name(), modsrc.src.as_str());
            }
        }

        report.finish().print(ariadne::sources(sources)).unwrap();
    }
}

#[derive(Debug)]
pub struct ErrorHolder<'c, E: BiwacError> {
    pub errs: Vec<E>,
    pub ctx: ErrorContext<'c>,
}

impl<'a, E: BiwacError> ErrorHolder<'a, E> {
    pub fn print_error_messages(&self) {
        for e in &self.errs {
            e.print_error_message(&self.ctx);
        }

        print_error_finish_message(self.errs.len());
    }
}

pub fn print_error_finish_message(err_count: usize) {
    println!(
        "{} Compile failed because of {} previous error(s).",
        "Error!".red().bold(),
        err_count
    );
}
