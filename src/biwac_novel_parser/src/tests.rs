//! `$` 埋め込み式の読み取り。
//!
//! 生テキストの断片と式にどう切り分けられるかを、AST の形で見る。

use biwac_ast::NovelStmt;
use biwac_base::{IdentInterner, ModId};
use biwac_span::Span;

use crate::{NovelParseError, NovelSourceStream};

/// scene の DSL 本文を読み、statement の列にする。
fn parse(src: &str) -> Result<Vec<NovelStmt>, NovelParseError> {
    let mut interner = IdentInterner::new();
    let span = Span::new(ModId::new_in_self(0), 0, src.len());
    NovelSourceStream::new(src, span, &mut interner).parse()
}

/// 出力されるテキストと式を、見分けやすい形に潰す。
///
/// 式は中身まで見ずに `$` として扱う。
/// 切り分けの位置が合っているかだけをこのテストで確かめたい。
fn shape(src: &str) -> Vec<String> {
    parse(src)
        .expect("should parse")
        .into_iter()
        .filter_map(|s| match s {
            NovelStmt::NovelWrite(m) => Some(format!("text({})", m.msg)),
            NovelStmt::NovelWriteExpr(_) => Some("expr".to_string()),
            NovelStmt::NovelWait(_) => Some("wait".to_string()),
            _ => None,
        })
        .collect()
}

#[test]
fn plain_text_is_one_write() {
    assert_eq!(vec!["text(こんにちは\n)"], shape("こんにちは\n"));
}

#[test]
fn wait_only_line_writes_nothing() {
    assert_eq!(vec!["wait"], shape(">>\n"));
}

#[test]
fn embedded_expression_splits_the_line() {
    assert_eq!(
        vec!["text(私は)", "expr", "text(です。\n)"],
        shape("私は$name()です。\n")
    );
}

#[test]
fn parenthesized_form() {
    assert_eq!(
        vec!["text(HP は )", "expr", "text( です。\n)"],
        shape("HP は $(player.hp) です。\n")
    );
}

#[test]
fn method_chain() {
    assert_eq!(
        vec!["expr", "text(\n)"],
        shape("$blue(bold(italic(\"琵琶\")))\n")
    );
    assert_eq!(vec!["expr", "text(\n)"], shape("$a.b().c()\n"));
    assert_eq!(vec!["expr", "text(\n)"], shape("$(x).y()\n"));
}

#[test]
fn parens_inside_a_string_do_not_close_the_expression() {
    // `")"` の中の括弧を数えると、ここで範囲が壊れる。
    assert_eq!(vec!["expr", "text( です\n)"], shape("$f(\")\") です\n"));
}

#[test]
fn escaped_dollar_is_literal() {
    assert_eq!(vec!["text(100$ です\n)"], shape("100\\$ です\n"));
}

#[test]
fn backslash_before_anything_else_is_kept() {
    assert_eq!(vec!["text(a\\b\n)"], shape("a\\b\n"));
}

#[test]
fn wait_after_an_expression() {
    assert_eq!(
        vec!["text(私は)", "expr", "text(です。\n)", "wait"],
        shape("私は$name()です。>>\n")
    );
}

#[test]
fn broken_forms_are_rejected() {
    // 呼び出しで終わらない
    assert!(matches!(
        parse("$player.hp です\n"),
        Err(NovelParseError::EmbeddedExpressionMustEndWithCall { .. })
    ));
    // 括弧が閉じない
    assert!(matches!(
        parse("$f(\"x\" です\n"),
        Err(NovelParseError::EmbeddedExpressionNotClosed { .. })
    ));
    // `$` の後ろが式でない
    assert!(matches!(
        parse("100$ です\n"),
        Err(NovelParseError::EmbeddedExpressionExpected { .. })
    ));
}
