use biwac_ast::{Ident, QualifiedId};
use biwac_base::Span;

use crate::{NovelParseError, NovelSourceStream};

#[derive(Debug, Clone)]
pub struct NCodeToken {
    pub(crate) kind: NCodeTkKind,
    pub(crate) span: Span,
}

#[derive(Debug, Clone)]
pub enum NCodeTkKind {
    Ident(String),         // <identifier>
    LiteralInteger(u64),   // integer literal
    LiteralString(String), // string literal
    KwTrue,                // bool literal `TRUE`
    KwFalse,               // bool literal `FALSE`
    KwPackage,             // package
    KwLet,                 // let
    KwIf,                  // if
    KwElse,                // else
    KwWhile,               // while
    KwEndScene,            // endscene
    KwUint,                // Uint (reserved word of type)
    KwInt,                 // Int (reserved word of type)
    KwFloat,               // Float (reserved word of type)
    KwBool,                // Bool (reserved word of type)
    MarkLPare,             // (
    MarkRPare,             // )
    MarkLBrace,            // {
    MarkRBrace,            // }
    MarkLBracket,          // [
    MarkRBracket,          // ]
    MarkPlus,              // +
    MarkMinus,             // -
    MarkAsterisk,          // *
    MarkSlash,             // /
    MarkPercent,           // %
    MarkAmpersand,         // &
    MarkLesser,            // <
    MarkGreater,           // >
    MarkLesEq,             // <=
    MarkGrtEq,             // >=
    MarkEqual,             // ==
    MarkNotEq,             // !=
    MarkAssign,            // =
    MarkNot,               // !
    MarkComma,             // ,
    MarkDot,               // .
    MarkArrow,             // ->
    MarkColon,             // :
    MarkSemiColon,         // ;
    MarkDoubleColon,       // ::
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    KwEndScene,      // endscene
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
    pub(crate) fn next_token(&mut self) -> Result<Option<NCodeToken>, NovelParseError> {
        match self.peeked.take() {
            Some(t) => {
                // カーソル位置を更新する
                if let Some(t) = &t {
                    self.cursor.idx = t.span.end().idx();
                }

                Ok(t)
            }
            None => {
                self.peek_token()?;

                let t = self.peeked.take().unwrap();
                if let Some(t) = &t {
                    self.cursor.idx = t.span.end().idx();
                }

                // SAFETY: .next_peek() で .peeked は必ず Some になっている
                Ok(t)
            }
        }
    }

    // peek_token
    // std::iter::Peekable のように、
    // 次トークンをイテレート位置を進めずに取得できるようにする
    // std::iter::Peekable がiterのラッパとして提供されているのに対し、
    // ここでは NovelSourceStream の機能として提供されており、
    // 1トークン先しか見られない。
    // 複数回呼ばれても1トークン先が何度も返されるだけである
    // syntax的にはそれで十分である。
    // 多分LL(2)文法ということになる
    // ノベルモード中のコードについては、おそらく実際には変換すればLL(1)として表せるだろうが、
    // パーサの実装のしやすさからpeekは用いたい。
    pub(crate) fn peek_token(&mut self) -> Result<Option<&NCodeToken>, NovelParseError> {
        let line = match self.lines.get(self.cursor.lidx) {
            Some(line) => line,
            None => {
                return Ok(None);
            }
        };

        // peeked にキャッシュがなければ先に更新する
        // NOTE: multiple mutable borrowing
        // を回避するために、予めis_noneなら更新する方法を取らざるを得ない
        if self.peeked.is_none() {
            if self.cursor.idx >= line.chars().count() {
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
                            "endscene" => (Some(NCodeTkKind::KwEndScene), 8),
                            "Uint" => (Some(NCodeTkKind::KwUint), 4),
                            "Int" => (Some(NCodeTkKind::KwInt), 3),
                            "Float" => (Some(NCodeTkKind::KwFloat), 5),
                            "Bool" => (Some(NCodeTkKind::KwBool), 4),
                            x => (Some(NCodeTkKind::Ident(x.to_string())), x.chars().count()),
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

                if let Some(kind) = kind {
                    self.peeked = Some(Some(NCodeToken {
                        kind,
                        span: self.current_span(token_len),
                    }));
                } else {
                    // NOTE: 空白文字のときは次トークンを探すために、peek内でもcursor.idxを進める
                    // そうでないとスタックオーバーフローする
                    self.cursor.idx += token_len;
                    self.peek_token()?;
                }
            };
        }

        Ok(self.peeked.as_ref().unwrap().as_ref())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

impl NCodeTkKind {
    fn as_kind_name(&self) -> NCodeTkKindName {
        match self {
            Self::Ident(_) => NCodeTkKindName::Ident, // <identifier>
            Self::LiteralInteger(_) => NCodeTkKindName::LiteralInteger, // integer literal
            Self::LiteralString(_) => NCodeTkKindName::LiteralString, // string literal
            Self::KwTrue => NCodeTkKindName::KwTrue,  // bool literal `TRUE`
            Self::KwFalse => NCodeTkKindName::KwFalse, // bool literal `FALSE`
            Self::KwPackage => NCodeTkKindName::KwPackage, // package
            Self::KwLet => NCodeTkKindName::KwLet,    // let
            Self::KwIf => NCodeTkKindName::KwIf,      // if
            Self::KwElse => NCodeTkKindName::KwElse,  // else
            Self::KwWhile => NCodeTkKindName::KwWhile, // while
            Self::KwEndScene => NCodeTkKindName::KwEndScene, // endscene
            Self::KwUint => NCodeTkKindName::KwUint,  // Uint (reserved word of type)
            Self::KwInt => NCodeTkKindName::KwInt,    // Int (reserved word of type)
            Self::KwFloat => NCodeTkKindName::KwFloat, // Float (reserved word of type)
            Self::KwBool => NCodeTkKindName::KwBool,  // Bool (reserved word of type)
            Self::MarkLPare => NCodeTkKindName::MarkLPare, // (
            Self::MarkRPare => NCodeTkKindName::MarkRPare, // )
            Self::MarkLBrace => NCodeTkKindName::MarkLBrace, // {
            Self::MarkRBrace => NCodeTkKindName::MarkRBrace, // }
            Self::MarkLBracket => NCodeTkKindName::MarkLBracket, // [
            Self::MarkRBracket => NCodeTkKindName::MarkRBracket, // ]
            Self::MarkPlus => NCodeTkKindName::MarkPlus, // +
            Self::MarkMinus => NCodeTkKindName::MarkMinus, // -
            Self::MarkAsterisk => NCodeTkKindName::MarkAsterisk, // *
            Self::MarkSlash => NCodeTkKindName::MarkSlash, // /
            Self::MarkPercent => NCodeTkKindName::MarkPercent, // %
            Self::MarkAmpersand => NCodeTkKindName::MarkAmpersand, // &
            Self::MarkLesser => NCodeTkKindName::MarkLesser, // <
            Self::MarkGreater => NCodeTkKindName::MarkGreater, // >
            Self::MarkLesEq => NCodeTkKindName::MarkLesEq, // <=
            Self::MarkGrtEq => NCodeTkKindName::MarkGrtEq, // >=
            Self::MarkEqual => NCodeTkKindName::MarkEqual, // ==
            Self::MarkNotEq => NCodeTkKindName::MarkNotEq, // !=
            Self::MarkAssign => NCodeTkKindName::MarkAssign, // =
            Self::MarkNot => NCodeTkKindName::MarkNot, // !
            Self::MarkComma => NCodeTkKindName::MarkComma, // ,
            Self::MarkDot => NCodeTkKindName::MarkDot, // .
            Self::MarkArrow => NCodeTkKindName::MarkArrow, // ->
            Self::MarkColon => NCodeTkKindName::MarkColon, // :
            Self::MarkSemiColon => NCodeTkKindName::MarkSemiColon, // ;
            Self::MarkDoubleColon => NCodeTkKindName::MarkDoubleColon, // ::
        }
    }
}

impl<'src> NovelSourceStream<'src> {
    pub(crate) fn must_consume_next(
        &mut self,
        kinds: Vec<NCodeTkKindName>,
    ) -> Result<NCodeToken, NovelParseError> {
        let t: NCodeToken = self.next_token()?.ok_or(NovelParseError::InvalidLineEnd {
            expecteds: kinds.clone(),
            span: self.current_span(1),
        })?;

        for kind in &kinds {
            if kind == &t.kind.as_kind_name() {
                return Ok(t);
            }
        }

        Err(NovelParseError::InvalidToken {
            expecteds: kinds,
            found: Box::new(t),
        })
    }

    pub(crate) fn consume_identifier(&mut self) -> Result<Ident, NovelParseError> {
        let t = self.next_token()?.ok_or(NovelParseError::InvalidLineEnd {
            expecteds: vec![NCodeTkKindName::Ident],
            span: self.current_span(1),
        })?;

        if let NCodeTkKind::Ident(ident) = &t.kind {
            Ok(Ident {
                id: ident.to_string(),
                span: t.span.clone(),
            })
        } else {
            Err(NovelParseError::InvalidToken {
                expecteds: vec![NCodeTkKindName::Ident],
                found: Box::new(t),
            })
        }
    }

    pub(crate) fn consume_qualified_identifier(&mut self) -> Result<QualifiedId, NovelParseError> {
        let mut ids = vec![];
        let (is_from_root, begin, mut end) = if let Some(t) = self.peek_token()?.cloned()
            && matches!(t.kind.as_kind_name(), NCodeTkKindName::KwPackage)
        {
            self.next_token()?;
            self.must_consume_next(vec![NCodeTkKindName::MarkDoubleColon])?;

            ids.push(self.consume_identifier()?.id);

            (true, t.span.clone(), t.span)
        } else {
            let ident = self.consume_identifier()?;
            ids.push(ident.id);

            (false, ident.span.clone(), ident.span)
        };

        loop {
            if let Some(t) = self.peek_token()? {
                if let NCodeTkKind::MarkDoubleColon = t.kind {
                    self.next_token()?;
                    let ident = self.consume_identifier()?;
                    ids.push(ident.id);
                    end = ident.span;
                } else {
                    return Ok(QualifiedId {
                        is_from_root,
                        quals: ids[..ids.len() - 1].to_vec(),
                        id: ids.last().expect("no identifier parsed").clone(),
                        span: Span::merge(&begin, &end),
                    });
                }
            } else {
                return Ok(QualifiedId {
                    is_from_root,
                    quals: ids[..ids.len() - 1].to_vec(),
                    id: ids.last().expect("no identifier parsed").clone(),
                    span: Span::merge(&begin, &end),
                });
            }
        }
    }
}
