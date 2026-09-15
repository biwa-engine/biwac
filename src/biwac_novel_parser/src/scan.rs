//! ノベル DSL の行を走査する状態機械。
//!
//! 行の中身は 4 つの状態を行き来する。
//!
//! ```text
//!           ┌──`\`──> エスケープ ──┐
//!           │                      │
//!   コード ──`"`──> 文字列 <────────┘
//!     │      <──`"`──┘
//!     └──`//`──> コメント (行末まで)
//! ```
//!
//! 生ノベルテキスト行ではこの外側にもう 1 つ状態がある。
//! **地の文の中では `"` も `//` もただの文字**で、
//! `$` の埋め込み式に入って初めてコードとして読まれる。
//!
//! ```biwa
//! $gray("// コメントも書けるよ")  ← この `//` はコメントではない
//! これは "引用" です。            ← この `"` は文字列を開かない
//! ```
//!
//! 行コメントの切り出しと埋め込み式の範囲測定は、どちらもこの走査の
//! 途中経過にすぎない。2 箇所に分けて書くと必ず食い違うので、
//! ここに集めてある。

use biwac_base::string_body_end;

/// 行コメントの導入記号。
const COMMENT: &str = "//";

/// 埋め込み式の導入記号。
const EMBED: char = '$';

/// エスケープの導入記号。
const ESCAPE: char = '\\';

/// 行の読み方。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LineMode {
    /// 行全体がコードである (`#` / `@` / `}` 行)。
    Code,
    /// 地の文。`$` の埋め込み式だけがコードになる。
    Novel,
}

/// 走査の途中で見つかる、式として読めない形。
///
/// span を持たないのは、ここが「行の中のバイト位置」しか知らないからである。
/// 診断に直すのは呼び出し側 ([`crate::symbols::statements`]) の仕事で、
/// 行コメントを探しているだけの場面ではそもそも報告しない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ScanError {
    /// `$` の後ろが埋め込み式の形になっていない。
    NotAnExpression { at: usize },
    /// 括弧が同じ行で閉じていない。
    UnclosedParen { at: usize },
    /// 呼び出しで終わっていない。
    NotEndingWithCall { begin: usize, end: usize },
}

/// 行コメント `//` の開始位置を返す。無ければ `None`。
///
/// 文字列の中の `//` はコメントではない。
/// 生ノベルテキストでは、埋め込み式の中の文字列まで見分ける。
///
/// 埋め込み式が壊れている行では、そこで探すのをやめる。
/// 形の誤りは本文を読むときに span 付きで報告されるので、
/// ここで重ねて報告しても同じ誤りが 2 回出るだけである。
pub(crate) fn find_line_comment(line: &str, mode: LineMode) -> Option<usize> {
    let mut i = 0;

    while i < line.len() {
        let rest = &line[i..];
        let c = rest.chars().next().expect("in range");

        if rest.starts_with(COMMENT) {
            return Some(i);
        }

        match mode {
            LineMode::Code => {
                if c == '"' {
                    i = skip_string(line, i);
                    continue;
                }
            }
            LineMode::Novel => {
                // `\$` は `$` そのもので、式を開かない。
                if c == ESCAPE && rest[c.len_utf8()..].starts_with(EMBED) {
                    i += c.len_utf8() + EMBED.len_utf8();
                    continue;
                }
                if c == EMBED {
                    match embedded_expr_end(line, i + c.len_utf8(), line.len()) {
                        Ok(end) => {
                            i = end;
                            continue;
                        }
                        Err(_) => return None,
                    }
                }
            }
        }

        i += c.len_utf8();
    }

    None
}

/// 開き `"` の位置から、閉じ `"` の次の位置を返す。
///
/// 閉じが無ければ行末を返す。閉じていないこと自体は
/// トークン化のときに報告されるので、ここでは走査を終えるだけでよい。
fn skip_string(src: &str, open_quote: usize) -> usize {
    let body = open_quote + 1;
    match string_body_end(&src[body..]) {
        Some(len) => body + len + 1,
        None => src.len(),
    }
}

/// `$` の直後 `begin` から、埋め込み式の終端を返す。
///
/// 文法はどちらの形も `)` で終わる。
/// 自由テキストの中で範囲を決められるのはこの性質のおかげである。
///
/// ```ebnf
/// <embeded-expression> ::= `$` `(` <expression> `)`
///   | `$` <identifier> ( <argument-list> | <member-access-or-method-calling>* <method-calling> )
/// ```
pub(crate) fn embedded_expr_end(src: &str, begin: usize, limit: usize) -> Result<usize, ScanError> {
    let mut i = begin;
    let mut ends_with_call;

    match src[i..limit].chars().next() {
        Some('(') => {
            i = balanced_paren_end(src, i, limit)?;
            // `$(expr)` はそれだけで完結する。
            ends_with_call = true;
        }
        Some(c) if is_ident_start(c) => {
            i = ident_end(src, i, limit);
            if src[i..limit].starts_with('(') {
                i = balanced_paren_end(src, i, limit)?;
                ends_with_call = true;
            } else {
                ends_with_call = false;
            }
        }
        _ => return Err(ScanError::NotAnExpression { at: begin }),
    }

    // メンバアクセスとメソッドチェーン。
    while src[i..limit].starts_with('.') {
        i += 1;
        let next = ident_end(src, i, limit);
        if next == i {
            return Err(ScanError::NotAnExpression { at: begin });
        }
        i = next;

        if src[i..limit].starts_with('(') {
            i = balanced_paren_end(src, i, limit)?;
            ends_with_call = true;
        } else {
            ends_with_call = false;
        }
    }

    // 連なりの最後は必ず呼び出しでなければならない。
    // そうでないと、どこまでが式でどこからがテキストかを決められない。
    if !ends_with_call {
        return Err(ScanError::NotEndingWithCall { begin, end: i });
    }

    Ok(i)
}

/// `(` から対応する `)` の次の位置を返す。
///
/// 文字列リテラルの中の括弧は数えない。
/// これを忘れると `$foo(")")` で範囲が壊れる。
fn balanced_paren_end(src: &str, begin: usize, limit: usize) -> Result<usize, ScanError> {
    let mut depth = 0usize;
    let mut i = begin;

    while i < limit {
        let c = src[i..limit].chars().next().expect("in range");

        match c {
            // 文字列の中は数えない。エスケープの扱いは共通の走査に任せる。
            '"' => {
                i = skip_string(&src[..limit], i);
                continue;
            }
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Ok(i + c.len_utf8());
                }
            }
            _ => {}
        }

        i += c.len_utf8();
    }

    Err(ScanError::UnclosedParen { at: begin })
}

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn is_ident_continue(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// `begin` から続く識別子の終わり。識別子でなければ `begin` を返す。
fn ident_end(src: &str, begin: usize, limit: usize) -> usize {
    let mut i = begin;
    for (off, c) in src[begin..limit].char_indices() {
        let ok = if off == 0 {
            is_ident_start(c)
        } else {
            is_ident_continue(c)
        };
        if !ok {
            break;
        }
        i = begin + off + c.len_utf8();
    }
    i
}

#[cfg(test)]
mod tests {
    use super::*;

    fn code(line: &str) -> Option<usize> {
        find_line_comment(line, LineMode::Code)
    }

    fn novel(line: &str) -> Option<usize> {
        find_line_comment(line, LineMode::Novel)
    }

    #[test]
    fn finds_a_plain_comment() {
        assert_eq!(code("foo() // hi"), Some(6));
        assert_eq!(novel("text // hi"), Some(5));
        assert_eq!(code("// hi"), Some(0));
    }

    #[test]
    fn a_comment_marker_inside_a_string_is_not_a_comment() {
        assert_eq!(code(r#"foo("// not")"#), None);
        assert_eq!(code(r#"foo("// not") // yes"#), Some(14));
        assert_eq!(novel(r#"$gray("// not")"#), None);
        assert_eq!(novel(r#"$gray("// not") // yes"#), Some(16));
    }

    #[test]
    fn an_escaped_quote_does_not_close_the_string() {
        assert_eq!(code(r#"foo("a\"// b")"#), None);
        assert_eq!(novel(r#"$foo("a\"// b")"#), None);
        // `\\` の後ろの `"` は閉じなので、そこから先はコードである。
        assert_eq!(code(r#"foo("a\\")// c"#), Some(10));
    }

    #[test]
    fn a_quote_in_prose_does_not_open_a_string() {
        // 地の文の `"` はただの文字。これを文字列と見ると、
        // 後ろの `//` がコメントだと分からなくなる。
        assert_eq!(
            novel(r#"彼は "そうだ" と言った // これはコメント"#),
            Some(32)
        );
    }

    #[test]
    fn an_escaped_dollar_does_not_open_an_expression() {
        assert_eq!(novel(r"100\$ // c"), Some(6));
    }

    #[test]
    fn gives_up_on_a_broken_expression() {
        // 形の誤りは本文を読むときに報告される。ここでは黙って探すのをやめる。
        assert_eq!(novel("$foo( // c"), None);
        assert_eq!(novel("$foo // c"), None);
    }

    #[test]
    fn measures_an_embedded_expression() {
        let src = r#"$blue(bold("琵琶"))です。"#;
        assert_eq!(embedded_expr_end(src, 1, src.len()), Ok(21));

        let src = r#"$foo(")")x"#;
        assert_eq!(embedded_expr_end(src, 1, src.len()), Ok(9));

        let src = r#"$foo("\"")x"#;
        assert_eq!(embedded_expr_end(src, 1, src.len()), Ok(10));
    }

    #[test]
    fn rejects_an_expression_that_does_not_end_with_a_call() {
        let src = "$foo.bar";
        assert!(matches!(
            embedded_expr_end(src, 1, src.len()),
            Err(ScanError::NotEndingWithCall { .. })
        ));
    }
}
