//! 数値リテラルの読み取り。
//!
//! 数字で始まる並びは、その場で 1 つのトークンとして読み切る。
//! 空白と記号で切ってから語を解釈する、という他のトークンの流れには乗せられない。
//! `1.5` の `.` が記号として切り出されてしまうためである。
//!
//! # 形
//!
//! ```text
//! <number>  ::= <digit>+ ( "." <digit>+ )?        10 進
//!             | "0" [bBoOxX] <radix-digit>+       基数つき
//! ```
//!
//! いずれも、直後が **識別子に使える文字であってはならない**。
//! biwa の識別子は `[a-zA-Z_][a-zA-Z0-9_]*` で数字始まりになりえないので、
//! `012abc` や `1.5x` のような並びはどう解釈しても意味を持たない。
//! 黙って識別子にすると遠くの段で不可解なエラーになるので、ここで弾く。
//!
//! `1.foo()` は「整数 `1`」「`.`」「`foo`」である。
//! 小数点として扱うのは **後ろに数字が続くとき**だけで、
//! そうでなければメソッド呼び出しのドットとして残す。

use crate::TkKind;

pub(crate) struct ScannedNumber {
    pub kind: NumberKind,
    /// 消費した長さ (バイト)。
    pub len: usize,
}

pub(crate) enum NumberKind {
    Int(u64),
    Float(f64),
}

impl NumberKind {
    pub(crate) fn into_token_kind<'src>(self) -> TkKind<'src> {
        match self {
            Self::Int(v) => TkKind::LiteralInteger(v),
            Self::Float(v) => TkKind::LiteralFloat(v),
        }
    }
}

/// 数値リテラルとして読み切れなかった。
pub(crate) struct NumberError {
    /// 「数値リテラルのつもりで書かれた範囲」の長さ。
    /// 診断で下線を引く範囲に使う。
    pub len: usize,
}

/// 数字で始まる `s` の先頭から数値リテラルを 1 つ読む。
pub(crate) fn scan_number(s: &str) -> Result<ScannedNumber, NumberError> {
    debug_assert!(
        s.starts_with(|c: char| c.is_ascii_digit()),
        "compiler bug: scan_number must be called at a digit"
    );

    let b = s.as_bytes();

    // --- 基数つき (`0b1010` / `0o755` / `0xFF`) ---
    //
    // 先頭が `0` で 2 文字めが基数の印のときだけ。
    // `00x1` や `10x1` は 10 進として読み、その結果 `x` で弾かれる。
    if b.len() >= 2
        && b[0] == b'0'
        && let Some(radix) = match b[1] {
            b'b' | b'B' => Some(2),
            b'o' | b'O' => Some(8),
            b'x' | b'X' => Some(16),
            _ => None,
        }
    {
        // 桁の並びは「識別子に使える文字」で取り切る。
        // `0xZZ` のように基数に合わない文字も含めて食べてから、
        // 変換の失敗としてまとめて弾く。
        let end = ident_like_end(s, 2);
        return match u64::from_str_radix(&s[2..end], radix) {
            Ok(value) => Ok(ScannedNumber {
                kind: NumberKind::Int(value),
                len: end,
            }),
            Err(_) => Err(NumberError { len: end }),
        };
    }

    // --- 10 進 ---
    let int_end = digits_end(b, 0);

    // 小数点として扱うのは、後ろに数字が続くときだけである。
    // `1.foo()` の `.` はメソッド呼び出しなので、ここでは触らない。
    let (end, is_float) =
        if b.get(int_end) == Some(&b'.') && b.get(int_end + 1).is_some_and(u8::is_ascii_digit) {
            (digits_end(b, int_end + 1), true)
        } else {
            (int_end, false)
        };

    // 区切りの検査。
    //
    // 直後が識別子に使える文字なら、数値リテラルでも識別子でもない。
    // 非 ASCII の文字も識別子には使えないので、まとめて弾く。
    if s[end..]
        .chars()
        .next()
        .is_some_and(|c| c.is_alphanumeric() || c == '_')
    {
        return Err(NumberError {
            len: ident_like_end(s, end),
        });
    }

    if is_float {
        match s[..end].parse::<f64>() {
            Ok(value) => Ok(ScannedNumber {
                kind: NumberKind::Float(value),
                len: end,
            }),
            Err(_) => Err(NumberError { len: end }),
        }
    } else {
        match s[..end].parse::<u64>() {
            Ok(value) => Ok(ScannedNumber {
                kind: NumberKind::Int(value),
                len: end,
            }),
            Err(_) => Err(NumberError { len: end }),
        }
    }
}

/// `from` から続く 10 進数字の終わり。
fn digits_end(b: &[u8], from: usize) -> usize {
    let mut i = from;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    i
}

/// `from` から続く「識別子に使える文字」の終わり。
///
/// 数値リテラルとして壊れている範囲を、診断のために取り切るのに使う。
fn ident_like_end(s: &str, from: usize) -> usize {
    let mut i = from;
    for c in s[from..].chars() {
        if c.is_alphanumeric() || c == '_' {
            i += c.len_utf8();
        } else {
            break;
        }
    }
    i
}

#[cfg(test)]
mod tests {
    use super::*;

    fn int(s: &str) -> (u64, usize) {
        match scan_number(s) {
            Ok(ScannedNumber {
                kind: NumberKind::Int(v),
                len,
            }) => (v, len),
            Ok(_) => panic!("expected an integer for `{s}`"),
            Err(_) => panic!("expected a number for `{s}`"),
        }
    }

    fn float(s: &str) -> (f64, usize) {
        match scan_number(s) {
            Ok(ScannedNumber {
                kind: NumberKind::Float(v),
                len,
            }) => (v, len),
            Ok(_) => panic!("expected a float for `{s}`"),
            Err(_) => panic!("expected a number for `{s}`"),
        }
    }

    fn err_len(s: &str) -> usize {
        match scan_number(s) {
            Err(e) => e.len,
            Ok(_) => panic!("expected an error for `{s}`"),
        }
    }

    #[test]
    fn integers() {
        assert_eq!((0, 1), int("0"));
        assert_eq!((123, 3), int("123"));
        assert_eq!((123, 3), int("123 + 1"));
        assert_eq!((123, 3), int("123;"));
        assert_eq!((123, 3), int("123)"));
    }

    #[test]
    fn floats() {
        assert_eq!((1.5, 3), float("1.5"));
        assert_eq!((0.25, 4), float("0.25 "));
        assert_eq!((12.0, 4), float("12.0)"));
        // 小数のあとのドットは区切りである
        assert_eq!((1.5, 3), float("1.5.abs()"));
    }

    #[test]
    fn dot_without_digits_is_not_a_decimal_point() {
        // `1.foo()` はメソッド呼び出し。整数 `1` で切れる。
        assert_eq!((1, 1), int("1.foo()"));
        assert_eq!((1, 1), int("1..2"));
        assert_eq!((1, 1), int("1."));
    }

    #[test]
    fn radix_prefixed() {
        assert_eq!((0xff, 4), int("0xFF"));
        assert_eq!((0b1010, 6), int("0b1010"));
        assert_eq!((0o755, 5), int("0o755"));
        assert_eq!((0xff, 4), int("0xff;"));
    }

    #[test]
    fn broken_literals() {
        // 数字の連続のあとに識別子の文字が続く
        assert_eq!(6, err_len("012abc"));
        assert_eq!(10, err_len("012.345abc"));
        assert_eq!(3, err_len("1e5"));
        // `_` は区切りではない (桁区切りは未対応)
        assert_eq!(5, err_len("1_000"));
        // 基数に合わない桁
        assert_eq!(4, err_len("0xZZ"));
        assert_eq!(4, err_len("0b12"));
        assert_eq!(2, err_len("0x"));
        // 桁あふれ
        assert_eq!(20, err_len("99999999999999999999"));
    }
}
