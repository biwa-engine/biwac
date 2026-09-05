pub(crate) mod line;

use std::fmt::Display;

use biwac_ast::{AbsolutePathHeader, Ident, Path};
use biwac_base::IdentInterner;
use biwac_span::Span;

use crate::{NovelLineHandler, NovelLineKind, NovelParseError};

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

const BIWAC_NOVEL_INDENT_STEP_DEPTH: usize = 4;

#[derive(Debug)]
pub struct NovelSourceStream<'src> {
    span: Span,
    // 現在のネストの深さ
    // ネストの深さから理想的なフォーマットでのインデントが決定される
    // 理想的なフォーマットでのインデント位置は、
    // ネストするたびに空白文字 ' ' 4?文字分下がることになっている
    // この位置からのさらなるインデントは、
    // 生ノベルテキストの場合はノベルテキスト自体だとして、表示に反映される
    nest_depth: usize,
    peeked: Option<NCodeTokenOption<NCodeToken>>,

    src: &'src str,

    interner: &'src mut IdentInterner,

    // DSL部分の文字列スライス src のインデックスで持つ:
    current_line: NovelLineHandler,
    next_line_begin_idx: usize,
}

#[derive(Debug)]
pub(crate) enum NCodeTokenOption<T> {
    Some(T),
    None { idx: usize },
}

impl NCodeTokenOption<&NCodeToken> {
    pub(crate) fn cloned(&self) -> NCodeTokenOption<NCodeToken> {
        match self {
            Self::Some(t) => NCodeTokenOption::Some(t.to_owned().clone()),
            Self::None { idx } => NCodeTokenOption::None { idx: *idx },
        }
    }
}

impl NCodeTokenOption<NCodeToken> {
    pub(crate) fn as_ref(&self) -> NCodeTokenOption<&NCodeToken> {
        match self {
            Self::Some(t) => NCodeTokenOption::Some(t),
            Self::None { idx } => NCodeTokenOption::None { idx: *idx },
        }
    }
}

impl<T> NCodeTokenOption<T> {
    pub(crate) fn ok_or_else<F: FnOnce(usize) -> NovelParseError>(
        self,
        f: F,
    ) -> Result<T, NovelParseError> {
        match self {
            Self::Some(t) => Ok(t),
            Self::None { idx } => Err(f(idx)),
        }
    }
}

impl<'src> NovelSourceStream<'src> {
    pub fn new(src: &'src str, span: Span, interner: &'src mut IdentInterner) -> Self {
        Self {
            span,
            nest_depth: 1, // scene の中であるため1階層分ネスト
            peeked: None,

            src,

            interner,

            // dummy
            current_line: NovelLineHandler::new(0, 0, NovelLineKind::RawNovel),

            next_line_begin_idx: 0,
        }
    }

    // begin_idx は src の中での byte index
    // ファイル全体ではない
    pub(crate) fn span_from(&self, begin_idx: usize, token_len: usize) -> Span {
        Span::new(
            self.span.module(),
            self.span.begin() + begin_idx,
            self.span.begin() + begin_idx + token_len,
        )
    }

    pub(crate) fn indent_depth(&self) -> usize {
        self.nest_depth * BIWAC_NOVEL_INDENT_STEP_DEPTH
    }

    pub(crate) fn indent_enter(&mut self) {
        self.nest_depth += 1;
    }

    pub(crate) fn indent_return(&mut self) {
        self.nest_depth = self.nest_depth.saturating_sub(1);
    }

    pub(crate) fn next_token(&mut self) -> Result<NCodeTokenOption<NCodeToken>, NovelParseError> {
        let t = match self.peeked.take() {
            Some(t) => t,
            None => {
                self.peek_token()?;

                // SAFETY: .peek_token() で .peeked は必ず Some になっている
                self.peeked.take().unwrap()
            }
        };

        // カーソル位置を更新する
        if let NCodeTokenOption::Some(t) = &t {
            // current line が末尾でかつpeekしたtokenがあるならば
            // 継続tokenにより次の行へ継続しているので、
            // ここで行を更新
            if self.current_line.is_line_end() {
                let (mut next_line, next_line_begin_idx) =
                    self.next_line_as_continuing_command().unwrap();
                next_line.proceed_to(t.span.end() - self.span.begin());
                self.current_line = next_line;
                self.next_line_begin_idx = next_line_begin_idx;
            } else {
                self.current_line
                    .proceed_to(t.span.end() - self.span.begin());
                self.current_line
                    .set_last_token_continues_over_line(t.kind.continues_over_line());
            }
        }

        Ok(t)
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
    pub(crate) fn peek_token(&mut self) -> Result<NCodeTokenOption<&NCodeToken>, NovelParseError> {
        // 行の終端であり、かつ最後のトークンは継続トークンでないなら
        // 直ちに終了
        if self.current_line.is_end_and_not_continued() {
            self.peeked = Some(NCodeTokenOption::None {
                idx: self.current_line.end_idx(),
            });
            return Ok(NCodeTokenOption::None {
                idx: self.current_line.end_idx(),
            });
        }

        // peeked にキャッシュがなければ先に更新する
        // NOTE: multiple mutable borrowing
        // を回避するために、予めis_noneなら更新する方法を取らざるを得ない
        if self.peeked.is_none() {
            let is_peeking_next_line = self.current_line.is_line_end()
                && self.current_line.last_token_continues_over_line();

            // remain_str は先頭の空白文字は trim 済み
            let (remain_str, peeking_begin_idx) = if is_peeking_next_line {
                // 継続トークンの場合、自動で次の行に進む
                match self.next_line_as_continuing_command() {
                    Some((next_line_handler, _)) => (
                        self.line_str(&next_line_handler),
                        next_line_handler.begin_idx(),
                    ),
                    None => {
                        self.peeked = Some(NCodeTokenOption::None {
                            idx: self.current_line.end_idx(),
                        });
                        return Ok(NCodeTokenOption::None {
                            idx: self.current_line.end_idx(),
                        });
                    }
                }
            } else {
                let current_line = self.line_str(&self.current_line);
                let trimmed_line = current_line.trim_start();
                (
                    trimmed_line,
                    self.current_line.begin_idx() + (current_line.len() - trimmed_line.len()),
                )
            };

            if remain_str.is_empty() {
                self.peeked = Some(NCodeTokenOption::None {
                    idx: peeking_begin_idx,
                });
                return Ok(NCodeTokenOption::None {
                    idx: peeking_begin_idx,
                });
            }

            let mut remain_chars = remain_str.chars().peekable();

            // SAFETY: 現在の位置が行内であることを検査済み
            let (kind, token_len) = match remain_chars.next().unwrap() {
                '(' => (NCodeTkKind::MarkLPare, 1),
                ')' => (NCodeTkKind::MarkRPare, 1),
                '{' => (NCodeTkKind::MarkLBrace, 1),
                '}' => (NCodeTkKind::MarkRBrace, 1),
                '[' => (NCodeTkKind::MarkLBracket, 1),
                ']' => (NCodeTkKind::MarkRBracket, 1),
                '+' => (NCodeTkKind::MarkPlus, 1),
                '*' => (NCodeTkKind::MarkAsterisk, 1),
                '/' => (NCodeTkKind::MarkSlash, 1),
                '%' => (NCodeTkKind::MarkPercent, 1),
                '&' => (NCodeTkKind::MarkAmpersand, 1),
                ';' => (NCodeTkKind::MarkSemiColon, 1),
                ',' => (NCodeTkKind::MarkComma, 1),
                '.' => (NCodeTkKind::MarkDot, 1),
                '-' => match remain_chars.peek() {
                    Some('>') => {
                        remain_chars.next();
                        (NCodeTkKind::MarkArrow, 2)
                    }
                    _ => (NCodeTkKind::MarkMinus, 1),
                },
                ':' => match remain_chars.peek() {
                    Some(':') => {
                        remain_chars.next();
                        (NCodeTkKind::MarkDoubleColon, 2)
                    }
                    _ => (NCodeTkKind::MarkColon, 1),
                },
                '<' => match remain_chars.peek() {
                    Some('=') => {
                        remain_chars.next();
                        (NCodeTkKind::MarkLesEq, 2)
                    }
                    _ => (NCodeTkKind::MarkLesser, 1),
                },
                '>' => match remain_chars.peek() {
                    Some('=') => {
                        remain_chars.next();
                        (NCodeTkKind::MarkGrtEq, 2)
                    }
                    _ => (NCodeTkKind::MarkGreater, 1),
                },
                '=' => match remain_chars.peek() {
                    Some('=') => {
                        remain_chars.next();
                        (NCodeTkKind::MarkEqual, 2)
                    }
                    _ => (NCodeTkKind::MarkAssign, 1),
                },
                '!' => match remain_chars.peek() {
                    Some('=') => {
                        remain_chars.next();
                        (NCodeTkKind::MarkNotEq, 2)
                    }
                    _ => (NCodeTkKind::MarkNot, 1),
                },

                // `"` 始まりなら、行内で閉じる文字列リテラルでなければならない
                // NOTE: 通常コード側の字句解析 (biwac_lexer) と同様、
                // エスケープシーケンスは未対応である
                '"' => {
                    let mut token_len = 1; // 開き `"` の分
                    let mut val = String::new();
                    let mut closed = false;

                    for c in remain_chars.by_ref() {
                        token_len += c.len_utf8();

                        if c == '"' {
                            closed = true;
                            break;
                        }

                        val.push(c);
                    }

                    if !closed {
                        return Err(NovelParseError::StringLiteralNotClosed {
                            span: self.span_from(peeking_begin_idx, 1),
                        });
                    }

                    (NCodeTkKind::LiteralString(val), token_len)
                }

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
                                return Err(NovelParseError::InvalidChar {
                                    expecteds: vec![
                                        CharKind::Numeric,
                                        CharKind::Mark,
                                        CharKind::WhiteSpace,
                                    ],
                                    found: x,
                                    span: self.span_from(peeking_begin_idx + token_len, 1),
                                });
                            }
                        }
                    }

                    (
                        NCodeTkKind::LiteralInteger(
                            remain_str[..token_len]
                                .parse()
                                .expect("must be parsed as usize"),
                        ),
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
                                return Err(NovelParseError::InvalidChar {
                                    expecteds: vec![
                                        CharKind::Alpha,
                                        CharKind::Numeric,
                                        CharKind::UnderScore,
                                        CharKind::Mark,
                                        CharKind::WhiteSpace,
                                    ],
                                    found: CharKind::Others,
                                    span: self.span_from(peeking_begin_idx + token_len, 1),
                                });
                            }
                        }
                    }

                    match &remain_str[..token_len] {
                        "TRUE" => (NCodeTkKind::KwTrue, 4),
                        "FALSE" => (NCodeTkKind::KwFalse, 5),
                        "package" => (NCodeTkKind::KwPackage, 7),
                        "let" => (NCodeTkKind::KwLet, 3),
                        "if" => (NCodeTkKind::KwIf, 2),
                        "else" => (NCodeTkKind::KwElse, 4),
                        "while" => (NCodeTkKind::KwWhile, 5),
                        "endscene" => (NCodeTkKind::KwEndScene, 8),
                        "Uint" => (NCodeTkKind::KwUint, 4),
                        "Int" => (NCodeTkKind::KwInt, 3),
                        "Float" => (NCodeTkKind::KwFloat, 5),
                        "Bool" => (NCodeTkKind::KwBool, 4),
                        x => (NCodeTkKind::Ident(x.to_string()), x.chars().count()),
                    }
                }

                c => {
                    // trim 済みのため空白は来ないはず
                    return Err(NovelParseError::InvalidChar {
                        expecteds: vec![
                            CharKind::Alpha,
                            CharKind::Numeric,
                            CharKind::UnderScore,
                            CharKind::Mark,
                            CharKind::WhiteSpace,
                        ],
                        found: char_kind(c),
                        span: self.span_from(peeking_begin_idx, 1),
                    });
                }
            };

            self.peeked = Some(NCodeTokenOption::Some(NCodeToken {
                kind,
                span: self.span_from(peeking_begin_idx, token_len),
            }));
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

impl CharKind {
    pub fn pattern(&self) -> &str {
        match self {
            Self::Numeric => "numeric character [0-9]",
            Self::Alpha => "alphabetic character [a-zA-Z]",
            Self::UnderScore => "underscore `_`",
            Self::Mark => "mark character",
            Self::WhiteSpace => "whitespace character",
            Self::Others => "other character",
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
        let t: NCodeToken =
            self.next_token()?
                .ok_or_else(|begin_idx| NovelParseError::InvalidLineEnd {
                    expecteds: kinds.clone(),
                    span: self.span_from(begin_idx, 1),
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
        let t = self
            .next_token()?
            .ok_or_else(|begin_idx| NovelParseError::InvalidLineEnd {
                expecteds: vec![NCodeTkKindName::Ident],
                span: self.span_from(begin_idx, 1),
            })?;

        if let NCodeTkKind::Ident(ident) = &t.kind {
            let interned = self.interner.get_or_insert(ident);
            Ok(Ident {
                id: interned,
                span: t.span.clone(),
            })
        } else {
            Err(NovelParseError::InvalidToken {
                expecteds: vec![NCodeTkKindName::Ident],
                found: Box::new(t),
            })
        }
    }

    pub(crate) fn consume_qualified_identifier(&mut self) -> Result<Path, NovelParseError> {
        let mut segments = vec![];
        let abs_header = if let NCodeTokenOption::Some(t) = self.peek_token()?.cloned()
            && matches!(t.kind.as_kind_name(), NCodeTkKindName::KwPackage)
        {
            self.next_token()?;
            self.must_consume_next(vec![NCodeTkKindName::MarkDoubleColon])?;

            segments.push(self.consume_identifier()?.into());

            Some(AbsolutePathHeader::Package(t.span.clone()))
        } else {
            let ident = self.consume_identifier()?;
            segments.push(ident.into());

            None
        };

        loop {
            if let NCodeTokenOption::Some(t) = self.peek_token()? {
                if let NCodeTkKind::MarkDoubleColon = t.kind {
                    self.next_token()?;
                    let ident = self.consume_identifier()?;
                    segments.push(ident.into());
                } else {
                    return Ok(Path::new(abs_header, segments));
                }
            } else {
                return Ok(Path::new(abs_header, segments));
            }
        }
    }
}

impl NCodeTkKind {
    pub fn pattern(&self) -> String {
        match self {
            Self::Ident(ident) => {
                format!("<identifier> `{ident}`")
            }
            Self::LiteralInteger(i) => format!("<integer-literal> `{i}`"),
            Self::LiteralString(s) => format!("<string-literal> `\"{s}\"`"),
            _ => format!("`{}`", self.as_kind_name().pattern()),
        }
    }

    pub fn continues_over_line(&self) -> bool {
        match self {
            Self::MarkLPare
            | Self::MarkLBracket
            | Self::MarkComma
            | Self::MarkDot
            | Self::MarkDoubleColon => true,
            _ => false,
            // NOTE: 将来拡大するかもしれない対象
            // | Self::MarkLBrace
            // | Self::MarkPlus
            // | Self::MarkMinus
            // | Self::MarkAsterisk
            // | Self::MarkSlash
            // | Self::MarkPercent
            // | Self::MarkLesser
            // | Self::MarkGreater
            // | Self::MarkLesEq
            // | Self::MarkGrtEq
            // | Self::MarkEqual
            // | Self::MarkNotEq
            // | Self::MarkAssign
            // | Self::MarkNot
        }
    }
}

impl NCodeTkKindName {
    fn pattern(&self) -> &str {
        match self {
            Self::Ident => "<identifier>",
            Self::LiteralInteger => "<integer-literal>",
            Self::LiteralString => "<string-literal>",
            Self::KwTrue => "TRUE",
            Self::KwFalse => "FALSE",
            Self::KwPackage => "package",
            Self::KwLet => "let",
            Self::KwIf => "if",
            Self::KwElse => "else",
            Self::KwWhile => "while",
            Self::KwEndScene => "endscene",
            Self::KwUint => "Uint",
            Self::KwInt => "Int",
            Self::KwFloat => "Float",
            Self::KwBool => "Bool",
            Self::MarkLPare => "(",
            Self::MarkRPare => ")",
            Self::MarkLBrace => "{",
            Self::MarkRBrace => "}",
            Self::MarkLBracket => "[",
            Self::MarkRBracket => "]",
            Self::MarkPlus => "+",
            Self::MarkMinus => "-",
            Self::MarkAsterisk => "*",
            Self::MarkSlash => "/",
            Self::MarkPercent => "%",
            Self::MarkAmpersand => "&",
            Self::MarkLesser => "<",
            Self::MarkGreater => ">",
            Self::MarkLesEq => "<=",
            Self::MarkGrtEq => ">=",
            Self::MarkEqual => "==",
            Self::MarkNotEq => "!=",
            Self::MarkAssign => "=",
            Self::MarkNot => "!",
            Self::MarkComma => ",",
            Self::MarkDot => ".",
            Self::MarkArrow => "->",
            Self::MarkColon => ":",
            Self::MarkSemiColon => ";",
            Self::MarkDoubleColon => "::",
        }
    }
}

impl Display for NCodeTkKindName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "`{}`", self.pattern())
    }
}
