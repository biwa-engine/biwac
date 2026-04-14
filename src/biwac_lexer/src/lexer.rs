use biwac_base::{ModId, Span};

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
    Dsl,
}

pub(crate) fn divide_regions(mod_id: ModId, src: &str) -> Result<Vec<SrcRegion>, TokenizeError> {
    let mut regions: Vec<SrcRegion> = vec![];

    let mut inner_quoted = false;
    let mut inner_dsl = false;
    let mut inner_line_comment = false;

    let mut region_begin_idx = 0;

    let chars: Vec<char> = src.chars().collect();
    let mut i = 0;
    while let Some(c) = chars.get(i) {
        match *c {
            // 改行ならリセット
            '\n' => {
                // DSLの領域は
                // ```biwa
                //   }}
                // ```
                // のように空白文字を除けば`}}`で始まる行が来たら終了
                // それまではここでは何もしない
                // パースは独自のパーサに委譲する
                if inner_dsl {
                    let remained_len = src[i..].chars().count();
                    let trimmed = src[i..].trim_start_matches('\n').trim_start();
                    let trimmed_len = remained_len - trimmed.len();
                    if trimmed.starts_with("}}") {
                        // end of DSL
                        regions.push(SrcRegion {
                            kind: RegionKind::Dsl,
                            span: Span::new(mod_id, region_begin_idx, i + trimmed_len),
                        });
                        i += 2 + trimmed_len;
                        region_begin_idx = i;
                        inner_dsl = false;
                        continue;
                    }
                } else if inner_quoted {
                    return Err(TokenizeError::DoubleQuoteCloseNotFound {
                        span: Span::new(mod_id, i, i + 1),
                    });
                } else if inner_line_comment {
                    inner_line_comment = false;
                    region_begin_idx = i + 1;
                }
            }
            c => {
                if !inner_dsl && !inner_line_comment {
                    if inner_quoted {
                        // TODO: if backslach appear, start escape
                        // if &l[idx..idx + 1] == "\\" {}
                        if c == '\"' {
                            // end of string literal
                            inner_quoted = false;
                            regions.push(SrcRegion {
                                kind: RegionKind::StringLiteral,
                                span: Span::new(mod_id, region_begin_idx, i + 1),
                            });
                            region_begin_idx = i + 1;
                        }
                    } else if c == '\"' {
                        // start of string literal
                        inner_quoted = true;
                        if i > region_begin_idx {
                            regions.push(SrcRegion {
                                kind: RegionKind::Raw,
                                span: Span::new(mod_id, region_begin_idx, i),
                            });
                            region_begin_idx = i;
                        }
                    } else if c == '/'
                        && let Some('/') = chars.get(i + 1)
                    {
                        // start of comment (to line end)
                        regions.push(SrcRegion {
                            kind: RegionKind::Raw,
                            span: Span::new(mod_id, region_begin_idx, i),
                        });
                        region_begin_idx = i;
                        inner_line_comment = true;
                        i += 2;

                        continue;
                    } else if let '{' = c
                        && let Some('{') = chars.get(i + 1)
                    {
                        // start of DSL such as novel mode, or inline native code.
                        regions.push(SrcRegion {
                            kind: RegionKind::Raw,
                            span: Span::new(mod_id, region_begin_idx, i),
                        });
                        region_begin_idx = i + 2;

                        inner_dsl = true;
                        i += 2;
                        continue;
                    }
                }
            }
        }

        i += 1;
        // i += c.len_utf8();
    }

    if inner_quoted {
        Err(TokenizeError::DoubleQuoteCloseNotFound {
            span: Span::new(mod_id, i, i + 1),
        })
    } else {
        regions.push(SrcRegion {
            kind: RegionKind::Raw,
            span: Span::new(mod_id, region_begin_idx, i),
        });

        Ok(regions)
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
    Dsl,
}

pub(crate) fn pre_lex(mod_id: ModId, src: &str, regions: Vec<SrcRegion>) -> Vec<PreToken> {
    let mut pretokens = vec![];

    for r in &regions {
        match r.kind {
            RegionKind::Raw => {
                let region_src: Vec<char> = src[r.span.begin()..r.span.end()].chars().collect();
                let mut idx = 0; // region 内のインデックス
                let mut token_begin_idx = 0;
                while let Some(c) = region_src.get(idx) {
                    if *c == '\n' {
                        if token_begin_idx < idx {
                            pretokens.push(PreToken {
                                kind: PreTkKind::Word,
                                span: Span::new(
                                    mod_id,
                                    r.span.begin() + token_begin_idx,
                                    r.span.begin() + idx,
                                ),
                            });
                        }
                        idx += 1;
                        token_begin_idx = idx;
                    } else if let Some(c2) = region_src.get(idx + 1)
                        && let Some(kind) = match (*c, *c2) {
                            ('<', '=') => Some(TkKind::LesEq),
                            ('>', '=') => Some(TkKind::GrtEq),
                            ('=', '=') => Some(TkKind::Equal),
                            ('!', '=') => Some(TkKind::NotEq),
                            ('-', '>') => Some(TkKind::Arrow),
                            (':', ':') => Some(TkKind::DoubleColon),
                            _ => None,
                        }
                    {
                        if token_begin_idx < idx {
                            pretokens.push(PreToken {
                                kind: PreTkKind::Word,
                                span: Span::new(
                                    mod_id,
                                    r.span.begin() + token_begin_idx,
                                    r.span.begin() + idx,
                                ),
                            });
                        }

                        pretokens.push(PreToken {
                            kind: PreTkKind::Mark(kind),
                            span: Span::new(
                                mod_id,
                                r.span.begin() + token_begin_idx,
                                r.span.begin() + idx + 2,
                            ),
                        });

                        idx += 2;
                        token_begin_idx = idx;
                    } else if let Some(kind) = match *c {
                        '.' => Some(TkKind::Dot),
                        '(' => Some(TkKind::LPare),
                        ')' => Some(TkKind::RPare),
                        '{' => Some(TkKind::LBrace),
                        '}' => Some(TkKind::RBrace),
                        '[' => Some(TkKind::LBracket),
                        ']' => Some(TkKind::RBracket),
                        '+' => Some(TkKind::Plus),
                        '-' => Some(TkKind::Minus),
                        '*' => Some(TkKind::Asterisk),
                        '/' => Some(TkKind::Slash),
                        '%' => Some(TkKind::Percent),
                        '&' => Some(TkKind::Ampersand),
                        '<' => Some(TkKind::Lesser),
                        '>' => Some(TkKind::Greater),
                        '=' => Some(TkKind::Assign),
                        ',' => Some(TkKind::Comma),
                        ':' => Some(TkKind::Colon),
                        ';' => Some(TkKind::SemiColon),
                        _ => None,
                    } {
                        if token_begin_idx < idx {
                            pretokens.push(PreToken {
                                kind: PreTkKind::Word,
                                span: Span::new(
                                    mod_id,
                                    r.span.begin() + token_begin_idx,
                                    r.span.begin() + idx,
                                ),
                            });
                        }

                        pretokens.push(PreToken {
                            kind: PreTkKind::Mark(kind),
                            span: Span::new(mod_id, r.span.begin() + idx, r.span.begin() + idx + 1),
                        });

                        idx += 1;
                        token_begin_idx = idx;
                    } else if *c == ' ' || *c == '\t' {
                        if token_begin_idx < idx {
                            pretokens.push(PreToken {
                                kind: PreTkKind::Word,
                                span: Span::new(
                                    mod_id,
                                    r.span.begin() + token_begin_idx,
                                    r.span.begin() + idx,
                                ),
                            });
                        }

                        idx += 1;
                        token_begin_idx = idx;
                    } else {
                        idx += 1;
                    }
                }

                if token_begin_idx < idx {
                    pretokens.push(PreToken {
                        kind: PreTkKind::Word,
                        span: Span::new(mod_id, r.span.begin() + token_begin_idx, r.span.end()),
                    });
                }
            }
            RegionKind::StringLiteral => {
                pretokens.push(PreToken {
                    kind: PreTkKind::StringLiteral,
                    span: r.span.clone(),
                });
            }
            RegionKind::Dsl => {
                pretokens.push(PreToken {
                    kind: PreTkKind::Dsl,
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
