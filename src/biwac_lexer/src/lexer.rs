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
    let src_len = src.len();
    let mut regions: Vec<SrcRegion> = vec![];

    let mut inner_quoted = false;
    let mut inner_dsl = false;
    let mut inner_line_comment = false;

    let mut region_begin_idx = 0;

    // UTF-8 バイトインデックス
    let mut char_indices_iter = src.char_indices();
    while let Some((i, c)) = char_indices_iter.next() {
        match c {
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
                    let remained_len = src[i..].len(); // len returns UTF-8 byte len
                    let trimmed = src[i..].trim_start_matches('\n').trim_start();
                    let trimmed_len = remained_len - trimmed.len();
                    if trimmed.starts_with("}}") {
                        // end of DSL
                        regions.push(SrcRegion {
                            kind: RegionKind::Dsl,
                            span: Span::new(mod_id, region_begin_idx, i + trimmed_len),
                        });

                        // トリム分と}}分イテレータを進める
                        char_indices_iter.nth(trimmed_len + 2);
                        region_begin_idx = i + trimmed_len + 2;
                        inner_dsl = false;
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
                    } else if c == '/' && i + 1 < src_len && src[i + 1..].starts_with("/") {
                        // start of comment (to line end)
                        regions.push(SrcRegion {
                            kind: RegionKind::Raw,
                            span: Span::new(mod_id, region_begin_idx, i),
                        });
                        region_begin_idx = i;
                        inner_line_comment = true;
                        char_indices_iter.next(); // 2個目の `/` を飛ばす
                    } else if let '{' = c
                        && i + 1 < src_len
                        && src[i + 1..].starts_with("{")
                    {
                        // start of DSL such as novel mode, or inline native code.
                        regions.push(SrcRegion {
                            kind: RegionKind::Raw,
                            span: Span::new(mod_id, region_begin_idx, i),
                        });
                        region_begin_idx = i + 2;
                        inner_dsl = true;
                        char_indices_iter.next(); // 2個目の `{` を飛ばす
                    }
                }
            }
        }
    }

    if inner_quoted {
        Err(TokenizeError::DoubleQuoteCloseNotFound {
            span: Span::new(mod_id, src_len, src_len + 1),
        })
    } else {
        regions.push(SrcRegion {
            kind: RegionKind::Raw,
            span: Span::new(mod_id, region_begin_idx, src_len),
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
