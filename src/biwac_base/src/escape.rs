//! 文字列リテラルのエスケープ。
//!
//! 通常コード (`biwac_lexer`) とノベル DSL (`biwac_novel_parser`) の
//! 両方から使う。**どちらでも同じ文字列が同じ意味になる**ようにするため、
//! 対応表はここ 1 箇所にしかない。

/// `\` に続けて書ける文字と、その意味。
///
/// **どの言語でも同じ書き方・同じ意味になるものだけを入れてある。**
/// `\u{..}` (Rust) / `\uXXXX` (JS, Java) / `\U........` のように
/// 言語ごとに割れるものは、決めるまで受け付けない。
///
/// `\'` は入れない。biwa の文字列は必ず `"` で囲むので、
/// `'` はエスケープせずそのまま書けばよい。
///
/// 未知のエスケープはエラーにする。黙って `\` を残すと、
/// あとから `\u` のような形を足すときに意味が変わってしまう。
const ESCAPES: &[(char, char)] = &[
    ('"', '"'),
    ('\\', '\\'),
    ('n', '\n'),
    ('t', '\t'),
    ('r', '\r'),
];

/// `\` の次の文字を、それが表す文字に直す。知らない文字なら `None`。
pub fn unescape_char(c: char) -> Option<char> {
    ESCAPES
        .iter()
        .find_map(|(from, to)| (*from == c).then_some(*to))
}

/// エスケープとして書ける文字を、診断に出せる形で並べる。
pub fn known_escapes() -> String {
    ESCAPES
        .iter()
        .map(|(from, _)| format!("`\\{from}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EscapeError {
    /// 知らないエスケープ。`offset` は `\` の位置 (中身の先頭からのバイト数)。
    Unknown { offset: usize, found: char },
    /// `\` で中身が終わっている。
    Trailing { offset: usize },
}

/// 文字列リテラルの**中身**(囲みの `"` を除いたもの) を展開する。
///
/// 閉じの `"` を探すのは呼び出し側の仕事である
/// ([`string_body_end`] を使う)。ここに来るのは既に切り出された中身で、
/// エスケープされていない `"` は含まれない。
pub fn unescape(body: &str) -> Result<String, EscapeError> {
    // `\` が 1 つも無いのが普通なので、その場合は何もしない。
    if !body.contains('\\') {
        return Ok(body.to_string());
    }

    let mut out = String::with_capacity(body.len());
    let mut chars = body.char_indices();

    while let Some((i, c)) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }

        match chars.next() {
            Some((_, next)) => match unescape_char(next) {
                Some(unescaped) => out.push(unescaped),
                None => {
                    return Err(EscapeError::Unknown {
                        offset: i,
                        found: next,
                    });
                }
            },
            None => return Err(EscapeError::Trailing { offset: i }),
        }
    }

    Ok(out)
}

/// 開き `"` の**次**から始まる文字列を見て、閉じ `"` までのバイト数を返す。
///
/// `\"` は閉じない。閉じが見つからなければ `None`。
///
/// ```text
/// "a\"b"
///  ^^^^^  ← ここを渡すと 4 が返る
/// ```
pub fn string_body_end(after_open_quote: &str) -> Option<usize> {
    let mut chars = after_open_quote.char_indices();

    while let Some((i, c)) = chars.next() {
        match c {
            '"' => return Some(i),
            // エスケープされた文字は中身を見ない。
            // これを飛ばさないと `"\""` の 2 つ目の `"` で閉じてしまう。
            '\\' => {
                chars.next();
            }
            _ => {}
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unescapes_the_known_forms() {
        assert_eq!(unescape(r#"a\"b"#).unwrap(), "a\"b");
        assert_eq!(unescape(r"a\\b").unwrap(), "a\\b");
        assert_eq!(unescape(r"a\nb").unwrap(), "a\nb");
        assert_eq!(unescape(r"a\tb").unwrap(), "a\tb");
        assert_eq!(unescape(r"a\rb").unwrap(), "a\rb");
    }

    #[test]
    fn leaves_plain_text_alone() {
        assert_eq!(unescape("// not a comment").unwrap(), "// not a comment");
        assert_eq!(unescape("it's fine").unwrap(), "it's fine");
    }

    #[test]
    fn rejects_what_is_not_decided_yet() {
        assert!(matches!(
            unescape(r"\u{1F600}"),
            Err(EscapeError::Unknown { found: 'u', .. })
        ));
        // `'` は囲みに使わないのでエスケープしない。
        assert!(matches!(
            unescape(r"\'"),
            Err(EscapeError::Unknown { found: '\'', .. })
        ));
        assert_eq!(unescape(r"a\"), Err(EscapeError::Trailing { offset: 1 }));
    }

    #[test]
    fn finds_the_closing_quote_past_escapes() {
        assert_eq!(string_body_end(r#"ab""#), Some(2));
        assert_eq!(string_body_end(r#"a\"b""#), Some(4));
        // `\\` の後ろの `"` は閉じである。
        assert_eq!(string_body_end(r#"a\\""#), Some(3));
        assert_eq!(string_body_end("no close"), None);
    }
}
