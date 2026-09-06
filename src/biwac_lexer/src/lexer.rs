use biwac_base::ModId;
use biwac_span::Span;

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
    // 開いている DSL の `{{` の位置。閉じが見つからなかったときの指し先に使う。
    let mut dsl_open_idx = 0;

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

                        // トリム分と }} 分イテレータを進める。
                        //
                        // 直前に取り出したのが i の `\n` なので、
                        // 次に出てくるのは i+1 である。
                        // 進めたいのは i+1 から `}}` の 2 文字目 (i+trimmed_len+1) までの
                        // trimmed_len+1 文字で、`nth(n)` は n+1 文字を消費する。
                        // (間は空白文字なのでバイト数と文字数は一致する)
                        char_indices_iter.nth(trimmed_len);
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

                        // `{{` の後ろに同じ行で何か書かれていたら、その場で弾く。
                        //
                        // DSL の中身は Biwa の文法ではないので、閉じの判定に中身は使えない
                        // (native code に `}}` が現れても不思議ではない)。
                        // そのため閉じは `}}` で始まる行だけと決めてあり、
                        // 1 行で書かれた `{{ ... }}` は閉じたことにならない。
                        // 放っておくと後続の定義まで DSL に飲み込まれ、
                        // 「定義が無い」という遠い場所のエラーになる。
                        let body_begin_idx = i + 2;
                        let rest_of_line =
                            src[body_begin_idx..].split('\n').next().unwrap_or_default();
                        if !rest_of_line.trim().is_empty() {
                            return Err(TokenizeError::DslOpenNotAtLineEnd {
                                span: Span::new(mod_id, i, body_begin_idx),
                            });
                        }

                        regions.push(SrcRegion {
                            kind: RegionKind::Raw,
                            span: Span::new(mod_id, region_begin_idx, i),
                        });
                        dsl_open_idx = i;
                        region_begin_idx = body_begin_idx;
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
    } else if inner_dsl {
        // `}}` の行が来ないままファイルが終わった。
        Err(TokenizeError::DslCloseNotFound {
            span: Span::new(mod_id, dsl_open_idx, dsl_open_idx + 2),
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
pub(crate) struct PreToken<'src> {
    pub kind: PreTkKind<'src>,
    pub span: Span,
}

#[derive(Debug)]
pub(crate) enum PreTkKind<'src> {
    Word, // 識別子または予約語; identifier ([a-zA-Z_][a-zA-Z0-9_]) or reserved word (only alphabet)
    Mark(TkKind<'src>), // 記号; reserved mark, such as `+`, `/`, `::`
    StringLiteral, // 文字列リテラル; string literal `"..."`, span contains double quotes
    Dsl,
}

pub(crate) fn pre_lex<'src>(
    mod_id: ModId,
    src: &'src str,
    regions: Vec<SrcRegion>,
) -> Vec<PreToken<'src>> {
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
                            ('<', '=') => Some(TkKind::MarkLesEq),
                            ('>', '=') => Some(TkKind::MarkGrtEq),
                            ('=', '=') => Some(TkKind::MarkEqual),
                            ('!', '=') => Some(TkKind::MarkNotEq),
                            ('-', '>') => Some(TkKind::MarkArrow),
                            ('=', '>') => Some(TkKind::MarkFatArrow),
                            (':', ':') => Some(TkKind::MarkDoubleColon),
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
                        '.' => Some(TkKind::MarkDot),
                        '(' => Some(TkKind::MarkLPare),
                        ')' => Some(TkKind::MarkRPare),
                        '{' => Some(TkKind::MarkLBrace),
                        '}' => Some(TkKind::MarkRBrace),
                        '[' => Some(TkKind::MarkLBracket),
                        ']' => Some(TkKind::MarkRBracket),
                        '+' => Some(TkKind::MarkPlus),
                        '-' => Some(TkKind::MarkMinus),
                        '*' => Some(TkKind::MarkAsterisk),
                        '/' => Some(TkKind::MarkSlash),
                        '%' => Some(TkKind::MarkPercent),
                        '&' => Some(TkKind::MarkAmpersand),
                        '<' => Some(TkKind::MarkLesser),
                        '>' => Some(TkKind::MarkGreater),
                        '=' => Some(TkKind::MarkAssign),
                        ',' => Some(TkKind::MarkComma),
                        ':' => Some(TkKind::MarkColon),
                        ';' => Some(TkKind::MarkSemiColon),
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
