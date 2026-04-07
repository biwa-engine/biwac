use biwac_base::Span;

use crate::{NovelParseError, NovelSourceStream};

#[derive(Debug)]
pub struct NCodeToken<'src> {
    pub(crate) kind: NCodeTkKind<'src>,
    pub(crate) span: Span,
}

#[derive(Debug)]
pub enum NCodeTkKind<'src> {
    Ident(&'src str),         // <identifier>
    LiteralInteger(usize),    // integer literal
    LiteralString(&'src str), // string literal
    KwTrue,                   // bool literal `TRUE`
    KwFalse,                  // bool literal `FALSE`
    KwPackage,                // package
    KwLet,                    // let
    KwIf,                     // if
    KwElse,                   // else
    KwWhile,                  // while
    KwReturn,                 // return
    KwUint,                   // Uint (reserved word of type)
    KwInt,                    // Int (reserved word of type)
    KwFloat,                  // Float (reserved word of type)
    KwBool,                   // Bool (reserved word of type)
    MarkLPare,                // (
    MarkRPare,                // )
    MarkLBrace,               // {
    MarkRBrace,               // }
    MarkLBracket,             // [
    MarkRBracket,             // ]
    MarkPlus,                 // +
    MarkMinus,                // -
    MarkAsterisk,             // *
    MarkSlash,                // /
    MarkPercent,              // %
    MarkAmpersand,            // &
    MarkLesser,               // <
    MarkGreater,              // >
    MarkLesEq,                // <=
    MarkGrtEq,                // >=
    MarkEqual,                // ==
    MarkNotEq,                // !=
    MarkAssign,               // =
    MarkNot,                  // !
    MarkComma,                // ,
    MarkDot,                  // .
    MarkArrow,                // ->
    MarkColon,                // :
    MarkSemiColon,            // ;
    MarkDoubleColon,          // ::
}

pub enum NCodeTkKindName {
    Ident,           // <identifier>
    LiteralInteger,  // integer literal
    LiteralString,   // string literal
    KwTrue,          // bool literal `TRUE`
    KwFalse,         // bool literal `FALSE`
    KwPackage,       // package
    KwLet,           // let
    KwIf,            // if
    KwElse,          // else
    KwWhile,         // while
    KwReturn,        // return
    KwUint,          // Uint (reserved word of type)
    KwInt,           // Int (reserved word of type)
    KwFloat,         // Float (reserved word of type)
    KwBool,          // Bool (reserved word of type)
    MarkLPare,       // (
    MarkRPare,       // )
    MarkLBrace,      // {
    MarkRBrace,      // }
    MarkLBracket,    // [
    MarkRBracket,    // ]
    MarkPlus,        // +
    MarkMinus,       // -
    MarkAsterisk,    // *
    MarkSlash,       // /
    MarkPercent,     // %
    MarkAmpersand,   // &
    MarkLesser,      // <
    MarkGreater,     // >
    MarkLesEq,       // <=
    MarkGrtEq,       // >=
    MarkEqual,       // ==
    MarkNotEq,       // !=
    MarkAssign,      // =
    MarkNot,         // !
    MarkComma,       // ,
    MarkDot,         // .
    MarkArrow,       // ->
    MarkColon,       // :
    MarkSemiColon,   // ;
    MarkDoubleColon, // ::
}

impl<'src> NovelSourceStream<'src> {
    pub(crate) fn next_token(&mut self) -> Result<Option<NCodeToken<'src>>, NovelParseError<'src>> {
        match self.peeked.take() {
            Some(t) => {
                // カーソル位置を更新する
                if let Some(t) = &t {
                    self.cursor.idx = t.span.end().idx();
                }

                Ok(t)
            }
            None => {
                self.next_peek()?;

                // SAFETY: .next_peek() で .peeked は必ず Some になっている
                Ok(self.peeked.take().unwrap())
            }
        }
    }

    pub(crate) fn next_peek(&mut self) -> Result<Option<&NCodeToken<'src>>, NovelParseError<'src>> {
        let line = self.current_line;

        // peeked にキャッシュがなければ先に更新する
        // NOTE: multiple mutable borrowing
        // を回避するために、予めis_noneなら更新する方法を取らざるを得ない
        if self.peeked.is_none() {
            if self.cursor.idx >= line.len() {
                self.peeked = Some(None);
            } else {
                let mut remain_chars = line[self.cursor.idx..].chars().peekable();

                // SAFETY: 現在のインデックスより行の長さが大きいため、
                // 次の文字は必ず存在する
                let (kind, token_len) = match remain_chars.next().unwrap() {
                    '(' => (Some(NCodeTkKind::MarkLPare), 1),
                    ')' => (Some(NCodeTkKind::MarkRPare), 1),
                    '{' => (Some(NCodeTkKind::MarkLBrace), 1),
                    '}' => (Some(NCodeTkKind::MarkRBrace), 1),
                    '[' => (Some(NCodeTkKind::MarkLBracket), 1),
                    ']' => (Some(NCodeTkKind::MarkRBracket), 1),
                    '+' => (Some(NCodeTkKind::MarkPlus), 1),
                    '*' => (Some(NCodeTkKind::MarkAsterisk), 1),
                    '/' => (Some(NCodeTkKind::MarkSlash), 1),
                    '%' => (Some(NCodeTkKind::MarkPercent), 1),
                    '&' => (Some(NCodeTkKind::MarkAmpersand), 1),
                    ';' => (Some(NCodeTkKind::MarkSemiColon), 1),
                    ',' => (Some(NCodeTkKind::MarkComma), 1),
                    '.' => (Some(NCodeTkKind::MarkDot), 1),
                    '-' => match remain_chars.peek() {
                        Some('>') => {
                            remain_chars.next();
                            (Some(NCodeTkKind::MarkArrow), 2)
                        }
                        _ => (Some(NCodeTkKind::MarkMinus), 1),
                    },
                    ':' => match remain_chars.peek() {
                        Some(':') => {
                            remain_chars.next();
                            (Some(NCodeTkKind::MarkDoubleColon), 2)
                        }
                        _ => (Some(NCodeTkKind::MarkColon), 1),
                    },
                    '<' => match remain_chars.peek() {
                        Some('=') => {
                            remain_chars.next();
                            (Some(NCodeTkKind::MarkLesEq), 2)
                        }
                        _ => (Some(NCodeTkKind::MarkLesser), 1),
                    },
                    '>' => match remain_chars.peek() {
                        Some('=') => {
                            remain_chars.next();
                            (Some(NCodeTkKind::MarkGrtEq), 2)
                        }
                        _ => (Some(NCodeTkKind::MarkGreater), 1),
                    },
                    '=' => match remain_chars.peek() {
                        Some('=') => {
                            remain_chars.next();
                            (Some(NCodeTkKind::MarkEqual), 2)
                        }
                        _ => (Some(NCodeTkKind::MarkAssign), 1),
                    },
                    '!' => match remain_chars.peek() {
                        Some('=') => {
                            remain_chars.next();
                            (Some(NCodeTkKind::MarkNotEq), 2)
                        }
                        _ => (Some(NCodeTkKind::MarkNot), 1),
                    },

                    // 数字始まりなら、数値リテラルでなければならない
                    '0'..='9' => {
                        let mut token_len = 1;
                        while let Some(c) = remain_chars.peek() {
                            match char_kind(*c) {
                                CharKind::Numeric => {
                                    token_len += 1;
                                    remain_chars.next();
                                }
                                CharKind::Mark | CharKind::WhiteSpace => {
                                    break;
                                }
                                // CharKind::Alpha | CharKind::UnderScore | CharKind::Others
                                x => {
                                    self.cursor.idx += token_len;
                                    return Err(NovelParseError::InvalidChar {
                                        expecteds: vec![
                                            CharKind::Numeric,
                                            CharKind::Mark,
                                            CharKind::WhiteSpace,
                                        ],
                                        found: x,
                                        span: self.current_span(1),
                                    });
                                }
                            }
                        }

                        (
                            Some(NCodeTkKind::LiteralInteger(
                                line[self.cursor.idx..self.cursor.idx + token_len]
                                    .parse()
                                    .expect("must be parsed as usize"),
                            )),
                            token_len,
                        )
                    }

                    // アルファベット等始まりは、識別子または予約語である
                    'a'..='z' | 'A'..='Z' | '_' => {
                        let mut token_len = 1;
                        while let Some(c) = remain_chars.peek() {
                            match char_kind(*c) {
                                CharKind::Alpha | CharKind::Numeric | CharKind::UnderScore => {
                                    token_len += 1;
                                    remain_chars.next();
                                }
                                CharKind::Mark | CharKind::WhiteSpace => {
                                    break;
                                }
                                CharKind::Others => {
                                    self.cursor.idx += token_len;
                                    return Err(NovelParseError::InvalidChar {
                                        expecteds: vec![
                                            CharKind::Alpha,
                                            CharKind::Numeric,
                                            CharKind::UnderScore,
                                            CharKind::Mark,
                                            CharKind::WhiteSpace,
                                        ],
                                        found: CharKind::Others,
                                        span: self.current_span(1),
                                    });
                                }
                            }
                        }

                        match &line[self.cursor.idx..self.cursor.idx + token_len] {
                            "TRUE" => (Some(NCodeTkKind::KwTrue), 4),
                            "FALSE" => (Some(NCodeTkKind::KwFalse), 5),
                            "package" => (Some(NCodeTkKind::KwPackage), 7),
                            "let" => (Some(NCodeTkKind::KwLet), 3),
                            "if" => (Some(NCodeTkKind::KwIf), 2),
                            "else" => (Some(NCodeTkKind::KwElse), 4),
                            "while" => (Some(NCodeTkKind::KwWhile), 5),
                            "return" => (Some(NCodeTkKind::KwReturn), 6),
                            "Uint" => (Some(NCodeTkKind::KwUint), 4),
                            "Int" => (Some(NCodeTkKind::KwInt), 3),
                            "Float" => (Some(NCodeTkKind::KwFloat), 5),
                            "Bool" => (Some(NCodeTkKind::KwBool), 4),
                            x => (Some(NCodeTkKind::Ident(x)), x.chars().count()),
                        }
                    }

                    c => {
                        if c.is_whitespace() {
                            // 空白はトークンではない
                            (None, c.len_utf8())
                        } else {
                            return Err(NovelParseError::InvalidChar {
                                expecteds: vec![
                                    CharKind::Alpha,
                                    CharKind::Numeric,
                                    CharKind::UnderScore,
                                    CharKind::Mark,
                                    CharKind::WhiteSpace,
                                ],
                                found: CharKind::Others,
                                span: self.current_span(1),
                            });
                        }
                    }
                };

                self.cursor.idx += token_len;

                if let Some(kind) = kind {
                    self.peeked = Some(Some(NCodeToken {
                        kind,
                        span: self.current_span(token_len),
                    }));
                } else {
                    self.next_peek()?;
                }
            };
        }

        Ok(self.peeked.as_ref().unwrap().as_ref())
    }
}

pub enum CharKind {
    Mark,       // mark
    Numeric,    // 0 ..= 9
    Alpha,      // [a-zA-Z]
    UnderScore, // _
    WhiteSpace, // ' ',  '\t', etc ...
    Others,     // others
}

fn char_kind(c: char) -> CharKind {
    match c {
        '0'..='9' => CharKind::Numeric,
        'a'..='z' | 'A'..='Z' => CharKind::Alpha,
        '_' => CharKind::UnderScore,
        '!' | '"' | '#' | '$' | '%' | '&' | '\'' | '(' | ')' | '*' | '+' | ',' | '-' | '.'
        | '/' | ':' | ';' | '<' | '=' | '>' | '?' | '@' | '[' | '\\' | ']' | '^' | '`' | '{'
        | '|' | '}' | '~' => CharKind::Mark,
        c => {
            if c.is_whitespace() {
                CharKind::WhiteSpace
            } else {
                CharKind::Others
            }
        }
    }
}
