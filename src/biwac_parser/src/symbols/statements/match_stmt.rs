use std::cell::OnceCell;

use biwac_lexer::{TkKind, TkKindName};
use biwac_span::Span;

use biwac_ast::{
    BlockExpr, IdentPattern, MatchExpr, MatchExprArm, MatchStmt, MatchStmtArm, Path, Pattern,
    PatternFields, VariantPattern,
};

use crate::{ExprOrStmt, ParseError, TokenStream};

impl<'t, 'src, 'i> TokenStream<'t, 'src, 'i> {
    // "match" <expression> "{" ( <pattern> "=>" <arm-body> ","? )* "}"
    //
    // `if` と同じく式にも文にもなる。
    // どちらであるかは**最初のアームの本体**で決まり、
    // 以降のアームもそれに合わせて読む。
    // 揃っていなければ、ブロックのパーサが通常のエラーを出す。
    pub(super) fn consume_match_expression_or_statement(
        &mut self,
    ) -> Result<ExprOrStmt<MatchExpr, MatchStmt>, ParseError<'src>> {
        let mod_id = self.mod_id;
        let begin = self
            .must_consume_next(vec![TkKindName::KwMatch])?
            .span
            .clone();

        // 直後にブロックの `{` が来るので、`if` / `while` と同じく
        // 構造体リテラルを式の候補から外して読む。
        let scrutinee = self.consume_condition_expression()?;

        let _ = self.must_consume_next(vec![TkKindName::MarkLBrace])?;

        // 空の match は、何が返るのか決められないので受け付けない。
        if let Some(t) = self.peek()
            && let TkKind::MarkRBrace = t.kind
        {
            return Err(ParseError::InvalidToken {
                expecteds: vec![TkKindName::Ident, TkKindName::KwUnderscore],
                found: (*t).clone(),
            });
        }

        // --- 1 つめのアームで形を決める ---
        let first_pattern = self.consume_pattern()?;
        let _ = self.must_consume_next(vec![TkKindName::MarkFatArrow])?;

        match self.consume_match_arm_body()? {
            ExprOrStmt::Expr(body) => {
                let mut arms = vec![MatchExprArm {
                    span: Span::merge(&first_pattern.span(), &body.span),
                    pattern: first_pattern,
                    body,
                }];

                let end = loop {
                    let t = self.peek().ok_or(ParseError::InvalidEOF {
                        mod_id,
                        expecteds: vec![TkKindName::MarkRBrace],
                    })?;
                    if let TkKind::MarkRBrace = t.kind {
                        let end = t.span.clone();
                        self.next();
                        break end;
                    }

                    let pattern = self.consume_pattern()?;
                    let _ = self.must_consume_next(vec![TkKindName::MarkFatArrow])?;
                    let body = self.consume_match_arm_body_expr()?;

                    arms.push(MatchExprArm {
                        span: Span::merge(&pattern.span(), &body.span),
                        pattern,
                        body,
                    });
                };

                Ok(ExprOrStmt::Expr(MatchExpr {
                    scrutinee: Box::new(scrutinee),
                    arms,
                    span: Span::merge(&begin, &end),
                }))
            }
            ExprOrStmt::Stmt(body) => {
                let mut arms = vec![MatchStmtArm {
                    span: Span::merge(&first_pattern.span(), &body.span),
                    pattern: first_pattern,
                    body,
                }];

                let end = loop {
                    let t = self.peek().ok_or(ParseError::InvalidEOF {
                        mod_id,
                        expecteds: vec![TkKindName::MarkRBrace],
                    })?;
                    if let TkKind::MarkRBrace = t.kind {
                        let end = t.span.clone();
                        self.next();
                        break end;
                    }

                    let pattern = self.consume_pattern()?;
                    let _ = self.must_consume_next(vec![TkKindName::MarkFatArrow])?;
                    let body = self.consume_block_statement()?;
                    self.opt_consume_arm_comma();

                    arms.push(MatchStmtArm {
                        span: Span::merge(&pattern.span(), &body.span),
                        pattern,
                        body,
                    });
                };

                Ok(ExprOrStmt::Stmt(MatchStmt {
                    scrutinee,
                    arms,
                    span: Span::merge(&begin, &end),
                }))
            }
        }
    }

    /// 式であることが決まっている位置の `match`。
    ///
    /// `let n = match o { .. };` のように、値が要る場所から呼ばれる。
    /// すべてのアームは値を返さなければならず、
    /// 返さないものはブロック式のパーサが通常のエラーを出す。
    pub(crate) fn consume_match_expression(&mut self) -> Result<MatchExpr, ParseError<'src>> {
        let mod_id = self.mod_id;
        let begin = self
            .must_consume_next(vec![TkKindName::KwMatch])?
            .span
            .clone();

        let scrutinee = self.consume_condition_expression()?;
        let _ = self.must_consume_next(vec![TkKindName::MarkLBrace])?;

        let mut arms = Vec::new();

        let end = loop {
            let t = self.peek().ok_or(ParseError::InvalidEOF {
                mod_id,
                expecteds: vec![TkKindName::MarkRBrace],
            })?;
            if let TkKind::MarkRBrace = t.kind {
                let end = t.span.clone();
                self.next();
                break end;
            }

            let pattern = self.consume_pattern()?;
            let _ = self.must_consume_next(vec![TkKindName::MarkFatArrow])?;
            let body = self.consume_match_arm_body_expr()?;

            arms.push(MatchExprArm {
                span: Span::merge(&pattern.span(), &body.span),
                pattern,
                body,
            });
        };

        if arms.is_empty() {
            return Err(ParseError::InvalidToken {
                expecteds: vec![TkKindName::Ident, TkKindName::KwUnderscore],
                found: self
                    .peek()
                    .map(|t| (*t).clone())
                    .ok_or(ParseError::InvalidEOF {
                        mod_id,
                        expecteds: vec![TkKindName::Ident],
                    })?,
            });
        }

        Ok(MatchExpr {
            scrutinee: Box::new(scrutinee),
            arms,
            span: Span::merge(&begin, &end),
        })
    }

    /// アームの本体。`{ .. }` か、`,` で終わる裸の式。
    fn consume_match_arm_body(
        &mut self,
    ) -> Result<ExprOrStmt<BlockExpr, biwac_ast::BlockStmt>, ParseError<'src>> {
        if let Some(t) = self.peek()
            && let TkKind::MarkLBrace = t.kind
        {
            let body = self.consume_block_expression_or_statement()?;
            self.opt_consume_arm_comma();
            return Ok(body);
        }

        Ok(ExprOrStmt::Expr(self.consume_bare_arm_body()?))
    }

    /// 式であることが決まっているアームの本体。
    fn consume_match_arm_body_expr(&mut self) -> Result<BlockExpr, ParseError<'src>> {
        if let Some(t) = self.peek()
            && let TkKind::MarkLBrace = t.kind
        {
            let body = self.consume_block_expression()?;
            self.opt_consume_arm_comma();
            return Ok(body);
        }

        self.consume_bare_arm_body()
    }

    /// `Color::Red => 0,` のような、ブロックを書かない形。
    /// 文を持たないブロック式として扱う。
    fn consume_bare_arm_body(&mut self) -> Result<BlockExpr, ParseError<'src>> {
        let expr = self.consume_expression()?;
        let span = expr.span();
        self.opt_consume_arm_comma();

        Ok(BlockExpr {
            stmts: Vec::new(),
            expr: Box::new(expr),
            span,
        })
    }

    /// アームの区切りの `,`。ブロック本体のあとでは省略できる。
    fn opt_consume_arm_comma(&mut self) {
        self.consume_next_if_match(vec![TkKindName::MarkComma]);
    }

    // <pattern> ::= "_"
    //             | <identifier>
    //             | <qualified-identifier> ( "(" <pattern>* ")" | "{" ... "}" )?
    pub(crate) fn consume_pattern(&mut self) -> Result<Pattern, ParseError<'src>> {
        let mod_id = self.mod_id;
        let t = *self.peek().ok_or(ParseError::InvalidEOF {
            mod_id,
            expecteds: vec![TkKindName::Ident, TkKindName::KwUnderscore],
        })?;

        match &t.kind {
            TkKind::KwUnderscore => {
                let span = t.span.clone();
                self.next();
                Ok(Pattern::Wildcard(span))
            }
            TkKind::Ident(_) | TkKind::KwPackage => {
                let begin = t.span.clone();
                let path = self.consume_qualified_identifier()?;

                // 単独の識別子は、この時点では束縛か unit バリアントか決められない。
                // 名前解決で「その名前がバリアントに解決されるか」を見て振り分ける。
                // rustc と同じ扱いである。
                if path.abs_header.is_none()
                    && path.segments.len() == 1
                    && !matches!(
                        self.peek().map(|t| &t.kind),
                        Some(TkKind::MarkLPare) | Some(TkKind::MarkLBrace)
                    )
                {
                    return Ok(Pattern::Ident(IdentPattern {
                        path,
                        var_id: OnceCell::new(),
                    }));
                }

                let (fields, end) = match self.peek().map(|t| &t.kind) {
                    Some(TkKind::MarkLPare) => {
                        let (pats, span) = self.consume_tuple_pattern_fields()?;
                        (PatternFields::Tuple(pats), span)
                    }
                    Some(TkKind::MarkLBrace) => {
                        let (pats, span) = self.consume_struct_pattern_fields()?;
                        (PatternFields::Struct(pats), span)
                    }
                    _ => (PatternFields::Unit, begin.clone()),
                };

                Ok(Pattern::Variant(VariantPattern {
                    path,
                    fields,
                    span: Span::merge(&begin, &end),
                }))
            }
            _ => Err(ParseError::InvalidToken {
                expecteds: vec![TkKindName::Ident, TkKindName::KwUnderscore],
                found: t.clone(),
            }),
        }
    }

    fn consume_tuple_pattern_fields(&mut self) -> Result<(Vec<Pattern>, Span), ParseError<'src>> {
        let mod_id = self.mod_id;
        let begin = self
            .must_consume_next(vec![TkKindName::MarkLPare])?
            .span
            .clone();

        let mut pats = Vec::new();

        loop {
            let t = self.peek().ok_or(ParseError::InvalidEOF {
                mod_id,
                expecteds: vec![TkKindName::MarkRPare],
            })?;
            if let TkKind::MarkRPare = t.kind {
                let end = t.span.clone();
                self.next();
                return Ok((pats, Span::merge(&begin, &end)));
            }

            pats.push(self.consume_pattern()?);

            let t = self.must_consume_next(vec![TkKindName::MarkComma, TkKindName::MarkRPare])?;
            if let TkKind::MarkRPare = t.kind {
                let end = t.span.clone();
                return Ok((pats, Span::merge(&begin, &end)));
            }
        }
    }

    // `{ name = n, alpha }`
    //
    // 構造体リテラルと同じく `=` を使う。
    // 値を書かない省略形は同名への束縛に展開する。
    fn consume_struct_pattern_fields(
        &mut self,
    ) -> Result<(Vec<(biwac_ast::Ident, Pattern)>, Span), ParseError<'src>> {
        let mod_id = self.mod_id;
        let begin = self
            .must_consume_next(vec![TkKindName::MarkLBrace])?
            .span
            .clone();

        let mut fields = Vec::new();

        loop {
            let t = self.peek().ok_or(ParseError::InvalidEOF {
                mod_id,
                expecteds: vec![TkKindName::Ident, TkKindName::MarkRBrace],
            })?;
            if let TkKind::MarkRBrace = t.kind {
                let end = t.span.clone();
                self.next();
                return Ok((fields, Span::merge(&begin, &end)));
            }

            let name = self.consume_identifier()?;

            // `{ alpha }` は `{ alpha = alpha }` の省略形である。
            let pattern = if self
                .consume_next_if_match(vec![TkKindName::MarkAssign])
                .is_some()
            {
                self.consume_pattern()?
            } else {
                Pattern::Ident(IdentPattern {
                    path: Path::new(None, vec![name.clone().into()]),
                    var_id: OnceCell::new(),
                })
            };

            fields.push((name, pattern));

            let t = self.must_consume_next(vec![TkKindName::MarkComma, TkKindName::MarkRBrace])?;
            if let TkKind::MarkRBrace = t.kind {
                let end = t.span.clone();
                return Ok((fields, Span::merge(&begin, &end)));
            }
        }
    }
}
