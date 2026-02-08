use biwac_base::{ModPath, Pos, Span};

use crate::{TokenizeError, token::TkKind};

#[derive(Debug)]
pub(crate) struct SrcRegion {
    kind: RegionKind,
    span: Span,
}

#[derive(Debug)]
enum RegionKind {
    Raw,
    StringLiteral,
}

pub(crate) fn divide_regions(modu: ModPath, src: &str) -> Result<Vec<SrcRegion>, TokenizeError> {
    let mut pretokens: Vec<SrcRegion> = vec![];

    let mut quoted = false;
    let mut last_pos = Pos::new(0, 0);
    let mut lmaxidx = 0;
    for (lidx, l) in src.lines().enumerate() {
        lmaxidx = lidx;
        let mut idx = 0;
        let mut comment_end = false;
        while idx < l.len() {
            if quoted {
                // TODO: if backslach appear, start escape
                // if &l[idx..idx + 1] == "\\" {}
                if let '\"' = l.chars().nth(idx).unwrap() {
                    // end of string literal
                    quoted = false;
                    pretokens.push(SrcRegion {
                        kind: RegionKind::StringLiteral,
                        span: Span::new(modu.clone(), last_pos, Pos::new(lidx, idx + 1)),
                    });
                    last_pos = Pos::new(lidx, idx + 1);
                }

                idx += 1;
            } else {
                if let '\"' = l.chars().nth(idx).unwrap() {
                    // start of string literal
                    quoted = true;
                    if idx > 0 {
                        pretokens.push(SrcRegion {
                            kind: RegionKind::Raw,
                            span: Span::new(modu.clone(), last_pos, Pos::new(lidx, idx)),
                        });
                        last_pos = Pos::new(lidx, idx);
                    }
                } else if let '/' = l.chars().nth(idx).unwrap()
                    && let Some('/') = l.chars().nth(idx + 1)
                {
                    // start of comment out (to line end)
                    pretokens.push(SrcRegion {
                        kind: RegionKind::Raw,
                        span: Span::new(modu.clone(), last_pos, Pos::new(lidx, idx)),
                    });
                    last_pos = Pos::new(lidx + 1, 0);

                    comment_end = true;
                    break;
                }

                idx += 1;
            }
        }

        if quoted {
            return Err(TokenizeError::DoubleQuoteCloseNotFound);
        } else if !comment_end && idx < l.len() {
            pretokens.push(SrcRegion {
                kind: RegionKind::Raw,
                span: Span::new(modu.clone(), last_pos, Pos::new(lidx, l.len())),
            });
            last_pos = Pos::new(lidx + 1, 0);
        }
    }

    if quoted {
        Err(TokenizeError::DoubleQuoteCloseNotFound)
    } else {
        pretokens.push(SrcRegion {
            kind: RegionKind::Raw,
            span: Span::new(
                modu.clone(),
                last_pos,
                Pos::new(lmaxidx, src.lines().last().map_or(0, |l| l.len())),
            ),
        });

        Ok(pretokens)
    }
}

#[derive(Debug)]
pub(crate) struct PreToken {
    pub kind: PreTkKind,
    pub span: Span,
}

#[derive(Debug)]
pub(crate) enum PreTkKind {
    Word, // 識別子または予約語; identifier ([a-zA-Z_][a-zA-Z0-9_]) or reserved word (only alphabet)
    Mark(TkKind), // 記号; reserved mark, such as `+`, `/`, `::`
    StringLiteral, // 文字列リテラル; string literal `"..."`, span contains double quotes
}

pub(crate) fn pre_lex(modu: ModPath, src: &str, regions: Vec<SrcRegion>) -> Vec<PreToken> {
    let lines: Vec<&str> = src.lines().collect();
    let mut pretokens = vec![];
    for r in &regions {
        match r.kind {
            RegionKind::Raw => {
                let mut lidx = r.span.begin().line();
                while lidx <= r.span.end().line() {
                    // NOTE: region の開始行は開始インデックスに注意
                    let mut idx = if lidx == r.span.begin().line() {
                        r.span.begin().idx()
                    } else {
                        0
                    };
                    let mut last_idx = idx;

                    // NOTE: region の終了行は終了インデックスに注意
                    let line = if lidx == r.span.end().line() {
                        &lines.get(lidx).unwrap()[idx..r.span.end().idx()]
                    } else {
                        &lines.get(lidx).unwrap()[idx..]
                    };
                    while idx < line.len() {
                        // two characters reserved mark
                        if idx + 1 < line.len() {
                            if let Some(kind) = match &line[idx..idx + 2] {
                                "<=" => Some(TkKind::LesEq),
                                ">=" => Some(TkKind::GrtEq),
                                "==" => Some(TkKind::Equal),
                                "!=" => Some(TkKind::NotEq),
                                "->" => Some(TkKind::Arrow),
                                "::" => Some(TkKind::DoubleColon),
                                _ => None,
                            } {
                                if last_idx < idx {
                                    pretokens.push(PreToken {
                                        kind: PreTkKind::Word,
                                        span: Span::new(
                                            modu.clone(),
                                            Pos::new(lidx, last_idx),
                                            Pos::new(lidx, idx),
                                        ),
                                    });
                                }

                                pretokens.push(PreToken {
                                    kind: PreTkKind::Mark(kind),
                                    span: Span::new(
                                        modu.clone(),
                                        Pos::new(lidx, idx),
                                        Pos::new(lidx, idx + 2),
                                    ),
                                });

                                idx += 2;
                                last_idx = idx;
                                continue;
                            }
                        }

                        // single character reserved mark
                        if let Some(kind) = match &line[idx..idx + 1] {
                            "." => Some(TkKind::Dot),
                            "(" => Some(TkKind::LPare),
                            ")" => Some(TkKind::RPare),
                            "{" => Some(TkKind::LBrace),
                            "}" => Some(TkKind::RBrace),
                            "[" => Some(TkKind::LBracket),
                            "]" => Some(TkKind::RBracket),
                            "+" => Some(TkKind::Plus),
                            "-" => Some(TkKind::Minus),
                            "*" => Some(TkKind::Asterisk),
                            "/" => Some(TkKind::Slash),
                            "%" => Some(TkKind::Percent),
                            "&" => Some(TkKind::Ampersand),
                            "<" => Some(TkKind::Lesser),
                            ">" => Some(TkKind::Greater),
                            "=" => Some(TkKind::Assign),
                            "," => Some(TkKind::Comma),
                            ":" => Some(TkKind::Colon),
                            ";" => Some(TkKind::SemiColon),
                            _ => None,
                        } {
                            if last_idx < idx {
                                pretokens.push(PreToken {
                                    kind: PreTkKind::Word,
                                    span: Span::new(
                                        modu.clone(),
                                        Pos::new(lidx, last_idx),
                                        Pos::new(lidx, idx),
                                    ),
                                });
                            }

                            pretokens.push(PreToken {
                                kind: PreTkKind::Mark(kind),
                                span: Span::new(
                                    modu.clone(),
                                    Pos::new(lidx, idx),
                                    Pos::new(lidx, idx + 1),
                                ),
                            });

                            idx += 1;
                            last_idx = idx;
                            continue;
                        }

                        match &line[idx..idx + 1] {
                            " " | "\t" => {
                                if last_idx < idx {
                                    pretokens.push(PreToken {
                                        kind: PreTkKind::Word,
                                        span: Span::new(
                                            modu.clone(),
                                            Pos::new(lidx, last_idx),
                                            Pos::new(lidx, idx),
                                        ),
                                    });
                                }

                                idx += 1;
                                last_idx = idx;
                            }
                            _ => {
                                idx += 1;
                            }
                        }
                    }

                    if last_idx + 1 < line.len() {
                        pretokens.push(PreToken {
                            kind: PreTkKind::Word,
                            span: Span::new(
                                modu.clone(),
                                Pos::new(lidx, last_idx),
                                Pos::new(lidx, line.len()),
                            ),
                        });
                    }

                    lidx += 1;
                }
            }
            RegionKind::StringLiteral => {
                pretokens.push(PreToken {
                    kind: PreTkKind::StringLiteral,
                    span: r.span.clone(),
                });
            }
        }
    }

    pretokens
}

pub(crate) fn try_get_dec_integer(str: &str) -> Option<u64> {
    str.parse::<u64>().ok()
}

pub(crate) fn try_get_prefixed_int(str: &str) -> Option<u64> {
    if str.len() > 2 && str.starts_with('0') {
        let radix = match str.chars().nth(1).unwrap().to_ascii_lowercase() {
            'b' => 2,
            'o' => 8,
            'x' => 16,
            _ => 0,
        };

        if radix != 0 {
            u64::from_str_radix(&str[2..], radix).ok()
        } else {
            None
        }
    } else {
        None
    }
}
