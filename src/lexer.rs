pub mod token;

use token::Token;

use crate::lexer::token::{Range, TokenKind};

#[derive(Debug)]
pub enum TokenizeError {
    SingleQuoteCloseNotFound,
    DoubleQuoteCloseNotFound,
}

pub fn tokenize(str: &str) -> Result<Vec<Token>, TokenizeError> {
    let mut pretokens: Vec<PreToken> = vec![];

    #[derive(Debug)]
    enum PreToken<'src> {
        Raw(&'src str, usize),
        CharLiteral(u8, usize, usize),
        StringLiteral(&'src str, usize, usize),
    }

    let mut i = 0;
    let mut raw_head = 0;
    while i < str.len() {
        if &str[i..i + 1] == "\'" {
            if i - raw_head > 0 {
                pretokens.push(PreToken::Raw(&str[raw_head..i], raw_head));
            }
            i += 1;

            let mut iquote = i;
            while iquote < str.len() {
                // TODO:
                // if backslach appear, start escape
                if &str[iquote..iquote + 1] == "\'" {
                    raw_head = iquote + 1;
                    if iquote == i + 1 {
                        let c: u8 = str[i..i + 1].parse().expect("failed to parse u8");
                        pretokens.push(PreToken::CharLiteral(c, i - 1, iquote + 1));
                        // char literal range contains single quotes before and after the character
                        i = iquote + 1;
                        break;
                    } else {
                        panic!("TODO: Too Many Characters in Single Quote `'`");
                    }
                }

                iquote += 1;
            }

            if iquote >= str.len() {
                return Err(TokenizeError::SingleQuoteCloseNotFound);
            }
        } else if &str[i..i + 1] == "\"" {
            if i - raw_head > 0 {
                pretokens.push(PreToken::Raw(&str[raw_head..i], raw_head));
            }
            i += 1;

            let mut iquote = i;
            while iquote < str.len() {
                // TODO:
                // if backslach appear, start escape
                if &str[iquote..iquote + 1] == "\"" {
                    raw_head = iquote + 1;
                    pretokens.push(PreToken::StringLiteral(&str[i..iquote], i - 1, iquote + 1));
                    // stirng literal range contains single quotes before and after the string
                    i = iquote + 1;
                    break;
                }

                iquote += 1;
            }

            if iquote >= str.len() {
                return Err(TokenizeError::DoubleQuoteCloseNotFound);
            }
        } else {
            i += 1;
        }
    }
    if i - raw_head > 0 {
        pretokens.push(PreToken::Raw(&str[raw_head..i], raw_head));
    }

    // delimiter operators
    // ISSUE: must sort longer by operator chars length
    let delims: Vec<TokenKind> = vec![
        TokenKind::LesEq,       // <=
        TokenKind::GrtEq,       // >=
        TokenKind::Equal,       // ==
        TokenKind::NotEq,       // !=
        TokenKind::Arrow,       // ->
        TokenKind::DoubleColon, // ::
        TokenKind::Dot,         // .
        TokenKind::LPare,       // (
        TokenKind::RPare,       // )
        TokenKind::LBrace,      // {
        TokenKind::RBrace,      // }
        TokenKind::LBracket,    // [
        TokenKind::RBracket,    // ]
        TokenKind::Plus,        // +
        TokenKind::Minus,       // -
        TokenKind::Asterisk,    // *
        TokenKind::Slash,       // /
        TokenKind::Percent,     // %
        TokenKind::Ampersand,   // &
        TokenKind::Lesser,      // <
        TokenKind::Greater,     // >
        TokenKind::Assign,      // =
        TokenKind::Comma,       // ,
        TokenKind::Colon,       // :
        TokenKind::SemiColon,   // ;
    ];

    let mut token_vecs: Vec<Vec<Token>> = vec![];
    for pretoken in &pretokens {
        match pretoken {
            PreToken::CharLiteral(c, begin, end) => {
                token_vecs.push(vec![Token {
                    kind: TokenKind::CharLiteral(*c),
                    range: Range {
                        begin: *begin,
                        end: *end,
                    },
                }]);
            }
            PreToken::StringLiteral(s, begin, end) => {
                token_vecs.push(vec![Token {
                    kind: TokenKind::StringLiteral(s.to_string()),
                    range: Range {
                        begin: *begin,
                        end: *end,
                    },
                }]);
            }
            PreToken::Raw(r, offset) => {
                let mut words: Vec<(&str, usize)> = vec![];
                let mut begin = None;

                for (i, c) in r.char_indices() {
                    if c == ' ' || c == '\t' || c == '\n' {
                        if let Some(bg) = begin {
                            words.push((&r[bg..i], bg));
                            begin = None;
                        }
                    } else if begin.is_none() {
                        begin = Some(i);
                    }
                }

                // the last token
                if let Some(bg) = begin {
                    words.push((&r[bg..], bg));
                }

                let mut tmp_tokens = vec![];

                for (w, begin) in words {
                    tmp_tokens.append(&mut to_tokens(w, &delims, offset + begin));
                }

                token_vecs.push(tmp_tokens);
            }
        }
    }

    // check tokens judged as string but can be judged as a reserved word
    let tokens: Vec<Token> = token_vecs
        .into_iter()
        .flatten()
        .map(|t| {
            match t.kind {
                TokenKind::Identifier(s) => {
                    // check literal expressions
                    if let Some(ikind) = try_get_dec_int(&s) {
                        Token {
                            kind: ikind,
                            range: Range {
                                begin: t.range.begin,
                                end: t.range.end,
                            },
                        }
                    } else if let Some(ikind) = try_get_prefixed_int(&s) {
                        Token {
                            kind: ikind,
                            range: Range {
                                begin: t.range.begin,
                                end: t.range.end,
                            },
                        }
                    } else {
                        let replacers = vec![
                            TokenKind::Langlibfn,
                            TokenKind::Import,
                            TokenKind::Package,
                            TokenKind::Fn,
                            TokenKind::Let,
                            TokenKind::If,
                            TokenKind::Else,
                            TokenKind::While,
                            TokenKind::Return,
                            TokenKind::SizeOf,
                            TokenKind::Uint,
                            TokenKind::Int,
                            TokenKind::Bool,
                            TokenKind::Struct,
                            TokenKind::BoolLiteralTrue,
                            TokenKind::BoolLiteralFalse,
                        ];

                        for r in replacers {
                            if r.pattern() == s {
                                return Token {
                                    kind: r.to_owned(),
                                    range: Range {
                                        begin: t.range.begin,
                                        end: t.range.end,
                                    },
                                };
                            }
                        }

                        Token {
                            kind: TokenKind::Identifier(s),
                            range: Range {
                                begin: t.range.begin,
                                end: t.range.end,
                            },
                        }
                    }
                }
                _ => t,
            }
        })
        .collect();

    Ok(tokens)
}

fn to_tokens(str: &str, delims: &Vec<TokenKind>, offset: usize) -> Vec<Token> {
    let mut tokens: Vec<Token> = vec![];
    let mut last_index = 0; // last index of char which is not pushed to tokens vector

    let mut i: usize = 0;
    while i < str.len() {
        for d in delims {
            if (|p: &str| {
                if i + p.len() - 1 < str.len() && p == &str[i..i + p.len()] {
                    if i - last_index > 0 {
                        tokens.push(Token {
                            kind: TokenKind::Identifier(str[last_index..i].to_string()),
                            range: Range {
                                begin: offset + last_index,
                                end: offset + i,
                            },
                        });
                    }
                    tokens.push(Token {
                        kind: d.clone(),
                        range: Range {
                            begin: offset + i,
                            end: offset + i + d.pattern().len(),
                        },
                    });

                    last_index = i + p.len();
                    i = i + p.len() - 1;

                    return true;
                }

                false
            })(&d.pattern())
            {
                break; // if delims sorted by length of pattern string, match with longest
            }
        }

        i += 1;
    }

    if last_index < str.len() {
        tokens.push(Token {
            kind: TokenKind::Identifier(str[last_index..].to_string()),
            range: Range {
                begin: offset + last_index,
                end: offset + last_index + str.len(),
            },
        });
    }

    tokens
}

fn try_get_dec_int(str: &str) -> Option<TokenKind> {
    if let Ok(dec) = str.parse::<u32>() {
        Some(TokenKind::IntLiteral(dec))
    } else {
        None
    }
}

fn try_get_prefixed_int(str: &str) -> Option<TokenKind> {
    if str.len() > 2 && str.starts_with('0') {
        let radix = match str.chars().nth(1).unwrap().to_ascii_lowercase() {
            'b' => 2,
            'o' => 8,
            'x' => 16,
            _ => 0,
        };

        if radix != 0 {
            if let Ok(u) = usize::from_str_radix(&str[2..], radix) {
                Some(TokenKind::IntLiteral(u as u32))
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    }
}
