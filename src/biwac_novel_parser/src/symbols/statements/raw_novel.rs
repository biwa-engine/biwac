//! 生ノベルテキスト行の読み取り。
//!
//! 行は「生テキストの断片」と「`$` の埋め込み式」に切り分けられる。
//!
//! ```biwa
//! Hello! 私の名前は$blue(bold(italic("琵琶")))です。>>
//! ^^^^^^^^^^^^^^^^ ^^^^^^^^^^^^^^^^^^^^^^^^^^ ^^^^^ ^^
//! テキスト         埋め込み式                 テキスト 待ち
//! ```
//!
//! 設計は `docs/content-api.md` を参照。

use biwac_ast::{NovelContent, NovelFlush, NovelStmt};
use biwac_span::Span;

use crate::scan::{self, ScanError};
use crate::{NovelLineHandler, NovelParseError, NovelSourceStream, token::NCodeTokenOption};

use super::{ParsedNovelStmt, WAIT_COMMAND};

/// 埋め込み式の導入記号。
const EMBED: char = '$';

/// エスケープの導入記号。
///
/// いまは `\$` だけを解釈する。文字列リテラルのエスケープも
/// 将来 `\` で行うことにしてあるので記法を揃えてある。
const ESCAPE: char = '\\';

impl<'src> NovelSourceStream<'src> {
    pub(super) fn consume_raw_novel_line(
        &mut self,
        line_handler: NovelLineHandler,
    ) -> Result<ParsedNovelStmt, NovelParseError> {
        let base = line_handler.begin_idx();
        let end = line_handler.end_idx();
        let line = self.line_str(&line_handler).to_string();
        let span = self.line_span(&line_handler);

        // 待ちコマンド `>>`。
        //
        // 行がこれだけなら待つだけ、
        // 本文の後ろに付いていればその行を書いてから待つ。
        let (ranges, wait) = match line.trim_end().strip_suffix(WAIT_COMMAND) {
            Some(before) => {
                let cut = line.rfind(WAIT_COMMAND).expect("suffix was found");
                let wait = NovelStmt::ContentFlushAndWait(NovelFlush {
                    span: Span::new(span.module(), span.begin() + before.len(), span.end()),
                });

                // `>>` だけを取り除く。字下げと行末の改行は本文の一部として残す。
                let ranges = if before.trim().is_empty() {
                    // 行が `>>` だけなら本文は無い。
                    Vec::new()
                } else {
                    vec![(base, base + cut), (base + cut + WAIT_COMMAND.len(), end)]
                };

                (ranges, Some(wait))
            }
            None => (vec![(base, end)], None),
        };

        let mut stmts = Vec::new();
        self.consume_novel_content(&ranges, &mut stmts)?;
        stmts.extend(wait);

        Ok(ParsedNovelStmt::Stmts { stmts })
    }

    /// 与えられた範囲をテキストと埋め込み式に切り分ける。
    ///
    /// 範囲が複数になるのは `>>` を取り除いたときで、
    /// その前後は**地続きのテキストとして扱う**
    /// (`です。>>` と改行が別々の出力にならないようにする)。
    fn consume_novel_content(
        &mut self,
        ranges: &[(usize, usize)],
        out: &mut Vec<NovelStmt>,
    ) -> Result<(), NovelParseError> {
        // 生テキストは切り貼りするので所有権ごと持つ。
        // `\$` を `$` に畳むため、元の範囲をそのまま切り出せない。
        let mut text = String::new();
        let mut text_begin = ranges.first().map(|(b, _)| *b).unwrap_or(0);
        let mut text_end = text_begin;

        for &(begin, end) in ranges {
            if text.is_empty() {
                text_begin = begin;
            }
            self.consume_novel_content_range(begin, end, &mut text, &mut text_begin, out)?;
            text_end = end;
        }

        push_text(out, &mut text, self.span_of(text_begin, text_end));

        Ok(())
    }

    fn consume_novel_content_range(
        &mut self,
        begin: usize,
        end: usize,
        text: &mut String,
        text_begin: &mut usize,
        out: &mut Vec<NovelStmt>,
    ) -> Result<(), NovelParseError> {
        let mut i = begin;

        while i < end {
            let rest = &self.src()[i..end];
            let c = rest.chars().next().expect("in range");

            if c == ESCAPE
                && let Some(next) = rest[c.len_utf8()..].chars().next()
                && next == EMBED
            {
                // `\$` は `$` そのもの。
                text.push(EMBED);
                i += c.len_utf8() + next.len_utf8();
                continue;
            }

            if c != EMBED {
                text.push(c);
                i += c.len_utf8();
                continue;
            }

            // 埋め込み式。まず手前のテキストを出す。
            push_text(out, text, self.span_of(*text_begin, i));

            let expr_begin = i + c.len_utf8();
            let expr_end = self.embedded_expr_end(expr_begin, end)?;
            let expr = self.consume_embedded_expression(expr_begin, expr_end)?;

            out.push(NovelStmt::ContentPush(NovelContent::Expr {
                expr,
                span: self.span_of(i, expr_end),
            }));

            i = expr_end;
            *text_begin = i;
        }

        Ok(())
    }

    /// `$` の直後 `begin` から、埋め込み式の終端を返す。
    ///
    /// 走査そのものは [`crate::scan`] が持つ。行コメントの切り出しも
    /// 同じ走査を使うので、ここでは診断に直すだけである。
    fn embedded_expr_end(&self, begin: usize, limit: usize) -> Result<usize, NovelParseError> {
        scan::embedded_expr_end(self.src(), begin, limit).map_err(|e| match e {
            ScanError::NotAnExpression { at } => NovelParseError::EmbeddedExpressionExpected {
                span: self.span_of(at, (at + 1).min(limit)),
            },
            ScanError::UnclosedParen { at } => NovelParseError::EmbeddedExpressionNotClosed {
                span: self.span_of(at, limit),
            },
            ScanError::NotEndingWithCall { begin, end } => {
                NovelParseError::EmbeddedExpressionMustEndWithCall {
                    span: self.span_of(begin, end),
                }
            }
        })
    }

    /// `[begin, end)` を式として読む。
    ///
    /// 読む範囲を先に測ってから行の範囲を狭めるのは、
    /// 式パーサに「どこで止まるか」を教える手段が無いからである。
    /// `$blue("琵琶")です。` をそのまま食わせると、
    /// `です。` をコードとして字句解析しようとして落ちる。
    fn consume_embedded_expression(
        &mut self,
        begin: usize,
        end: usize,
    ) -> Result<biwac_ast::Exprs, NovelParseError> {
        let saved_line = self.take_line_for_embedded_expression(begin, end);

        let result = (|| {
            let expr = self.consume_expression()?;

            // 測った範囲を使い切っていなければ、式として読めていない。
            match self.peek_token()? {
                NCodeTokenOption::None { .. } => Ok(expr),
                NCodeTokenOption::Some(t) => Err(NovelParseError::LineEndExpected {
                    found: Box::new(t.to_owned()),
                }),
            }
        })();

        self.restore_line_after_embedded_expression(saved_line);
        result
    }
}

/// 溜めたテキストを 1 つの文として出す。空なら何もしない。
fn push_text(out: &mut Vec<NovelStmt>, text: &mut String, span: Span) {
    if text.is_empty() {
        return;
    }
    out.push(NovelStmt::ContentPush(NovelContent::Text {
        text: std::mem::take(text),
        span,
    }));
}
