use biwac_base::Span;
use biwac_lexer::{TkKind, Token};

use crate::{
    BlockExpr, BlockStmt, BoolLiteral, CompilerFlag, CompilerFlagArg, CompilerFlagLiteral, DefTyp,
    IntegerLiteral, ParseError, PrimTyp, Stmt, StringLiteral, TypRepr, TypReprVal,
    symbols::{ExprOrStmt, Ident, QualifiedId, globals::FnParseCtx},
};

#[derive(Debug)]
pub(crate) struct TokenStream<'t> {
    tokens: std::iter::Peekable<std::slice::Iter<'t, Token>>,
}

impl<'t> TokenStream<'t> {
    pub(crate) fn new(tokens: std::iter::Peekable<std::slice::Iter<'t, Token>>) -> Self {
        Self { tokens }
    }

    pub(crate) fn next(&mut self) -> Option<&Token> {
        self.tokens.next()
    }

    pub(crate) fn peek(&mut self) -> Option<&&'t Token> {
        self.tokens.peek()
    }

    pub(crate) fn consume_next_if_match(&mut self, kinds: Vec<TkKind>) -> Option<&Token> {
        let t: &Token = self.peek()?;

        for kind in &kinds {
            if kind == &t.kind {
                self.next();
                return Some(t);
            }
        }

        None
    }

    pub(crate) fn must_consume_next(&mut self, kinds: Vec<TkKind>) -> Result<&Token, ParseError> {
        let t: &Token = self.next().ok_or(ParseError::InvalidEOF(kinds.clone()))?;

        for kind in &kinds {
            if kind == &t.kind {
                return Ok(t);
            }
        }

        Err(ParseError::InvalidToken(kinds, t.clone()))
    }

    pub(crate) fn must_consume_semicolon(&mut self) -> Result<Token, ParseError> {
        let t = self
            .next()
            .ok_or(ParseError::InvalidEOF(vec![TkKind::SemiColon]))?
            .clone();

        if t.kind == TkKind::SemiColon {
            Ok(t)
        } else {
            Err(ParseError::InvalidToken(vec![TkKind::SemiColon], t.clone()))
        }
    }

    pub(crate) fn opt_consume_semicolon(&mut self) -> Option<Token> {
        if let Some(t) = self.peek().cloned()
            && t.kind == TkKind::SemiColon
        {
            self.next();
            Some(t.to_owned())
        } else {
            None
        }
    }

    pub(crate) fn must_consume_type_annotation(
        &mut self,
        self_typ: &Option<TypRepr>,
    ) -> Result<TypRepr, ParseError> {
        let t = self
            .peek()
            .ok_or(ParseError::InvalidEOF(vec![TkKind::Colon]))?
            .to_owned();

        if let TkKind::Colon = t.kind {
            self.next();

            Ok(self.consume_type_representaion(self_typ)?)
        } else {
            Err(ParseError::InvalidToken(vec![TkKind::Colon], t.clone()))
        }
    }

    pub(crate) fn opt_consume_type_annotation(
        &mut self,
        self_typ: &Option<TypRepr>,
    ) -> Result<Option<TypRepr>, ParseError> {
        let t = self
            .peek()
            .ok_or(ParseError::InvalidEOF(vec![TkKind::Colon]))?
            .to_owned();

        if let TkKind::Colon = t.kind {
            self.next();

            Ok(Some(self.consume_type_representaion(self_typ)?))
        } else {
            Ok(None)
        }
    }

    pub(crate) fn consume_type_representaion(
        &mut self,
        self_typ: &Option<TypRepr>,
    ) -> Result<TypRepr, ParseError> {
        if let Some(t) = self.peek() {
            if let TkKind::Uint = t.kind {
                let span = t.span.clone();
                self.next();
                Ok(TypRepr {
                    val: TypReprVal::Primitive(PrimTyp::Uint),
                    span,
                })
            } else if let TkKind::Int = t.kind {
                let span = t.span.clone();
                self.next();
                Ok(TypRepr {
                    val: TypReprVal::Primitive(PrimTyp::Int),
                    span,
                })
            } else if let TkKind::Float = t.kind {
                let span = t.span.clone();
                self.next();
                Ok(TypRepr {
                    val: TypReprVal::Primitive(PrimTyp::Float),
                    span,
                })
            } else if let TkKind::Bool = t.kind {
                let span = t.span.clone();
                self.next();
                Ok(TypRepr {
                    val: TypReprVal::Primitive(PrimTyp::Bool),
                    span,
                })
            } else if let TkKind::Ident = t.kind {
                // NOTE: idのみ得られた場合、ジェネリクス型(`T`)である可能性がある
                let qualid = self.consume_qualified_identifier()?;
                let genargs = self.opt_consume_generic_args(self_typ)?;

                Ok(TypRepr {
                    span: qualid.span.clone(),
                    val: TypReprVal::Defined(DefTyp { qualid, genargs }),
                })
            } else if let TkKind::Package = t.kind {
                let qualid = self.consume_qualified_identifier()?;
                let genargs = self.opt_consume_generic_args(self_typ)?;

                Ok(TypRepr {
                    span: qualid.span.clone(),
                    val: TypReprVal::Defined(DefTyp { qualid, genargs }),
                })
            } else if let TkKind::SelfTyp = t.kind
                && let Some(self_typ) = self_typ
            {
                // Self型がある場合のみSelfは有効
                self.next();
                Ok(self_typ.clone())
            } else {
                Err(ParseError::InvalidToken(
                    vec![TkKind::Uint, TkKind::Int, TkKind::Bool, TkKind::Ident],
                    t.to_owned().clone(),
                ))
            }
        } else {
            Err(ParseError::InvalidEOF(vec![
                TkKind::Uint,
                TkKind::Int,
                TkKind::Bool,
                TkKind::Ident,
                TkKind::Package,
            ]))
        }
    }

    /// consume generic argument declaration
    /// ```biwa
    /// struct Hoge[T, U] { ... }
    ///            ^^^^^^
    ///
    /// fn hoge[T, U](t: T, i: Int) -> U { ... }
    ///        ^^^^^^
    ///
    /// impl[T, U] Hoge[T, U] { ... }
    ///     ^^^^^^
    /// ```
    /// Generic argument declaration is declaration of generic type which appears for the first
    /// time in the scope, so it only contains <identifier>.
    /// It has diffinitly different meaning with generic argument assignment which makes generic
    /// type argument concrete type.
    ///
    /// ジェネリック型引数宣言は、そのスコープで始めて現れるジェネリック型の宣言であり、
    /// その引数列には<identifier>しか含まれない。
    /// ジェネリック型を具体化するときのジェネリック型引数の代入列とは別の意味合いである。
    pub(crate) fn opt_consume_generic_argument_declaration(
        &mut self,
    ) -> Result<Vec<Ident>, ParseError> {
        let mut genargs = vec![];
        if let Some(t) = self.peek()
            && matches!(t.kind, TkKind::LBracket)
        {
            self.next();
        } else {
            return Ok(genargs);
        }

        loop {
            if let Some(t) = self.peek()
                && let TkKind::RBracket = t.kind
            {
                self.next();

                return Ok(genargs);
            } else {
                genargs.push(self.consume_identifier()?);

                if let Some(t) = self.next() {
                    if let TkKind::RBracket = t.kind {
                        return Ok(genargs);
                    } else if let TkKind::Comma = t.kind {
                        continue;
                    } else {
                        return Err(ParseError::InvalidToken(
                            vec![TkKind::RBracket, TkKind::Comma],
                            t.clone(),
                        ));
                    }
                } else {
                    return Err(ParseError::InvalidEOF(vec![
                        TkKind::RBracket,
                        TkKind::Comma,
                    ]));
                }
            }
        }
    }

    /// Optionaly consumes tokens and parses to get generic arguments.
    /// We should use here:
    /// let a: foo::bar[Int] = ...
    ///                ^
    ///                |
    // pub(crate) fn opt_consume_generic_argument_assignment(
    pub(crate) fn opt_consume_generic_args(
        &mut self,
        self_typ: &Option<TypRepr>,
    ) -> Result<Vec<TypRepr>, ParseError> {
        let mut genargs = vec![];
        if let Some(t) = self.peek()
            && matches!(t.kind, TkKind::LBracket)
        {
            self.next();
        } else {
            return Ok(genargs);
        }

        loop {
            if let Some(t) = self.peek()
                && let TkKind::RBracket = t.kind
            {
                self.next();

                return Ok(genargs);
            } else {
                genargs.push(self.consume_type_representaion(self_typ)?);

                if let Some(t) = self.next() {
                    if let TkKind::RBracket = t.kind {
                        return Ok(genargs);
                    } else if let TkKind::Comma = t.kind {
                        continue;
                    } else {
                        return Err(ParseError::InvalidToken(
                            vec![TkKind::RBracket, TkKind::Comma],
                            t.clone(),
                        ));
                    }
                } else {
                    return Err(ParseError::InvalidEOF(vec![
                        TkKind::RBracket,
                        TkKind::Comma,
                    ]));
                }
            }
        }
    }

    pub(crate) fn consume_identifier(&mut self) -> Result<Ident, ParseError> {
        let t = self
            .next()
            .ok_or(ParseError::InvalidEOF(vec![TkKind::Ident]))?;

        if let TkKind::Ident = &t.kind {
            Ok(Ident {
                id: t.unwrap_string_value(),
                span: t.span.clone(),
            })
        } else {
            Err(ParseError::InvalidToken(vec![TkKind::Ident], t.clone()))
        }
    }

    pub(crate) fn consume_qualified_identifier(&mut self) -> Result<QualifiedId, ParseError> {
        let mut ids = vec![];
        let (is_from_root, begin, mut end) = if let Some(t) = self.peek().cloned()
            && matches!(t.kind, TkKind::Package)
        {
            self.next();
            self.must_consume_next(vec![TkKind::DoubleColon])?;

            ids.push(self.consume_identifier()?.id);

            (true, t.span.clone(), t.span.clone())
        } else {
            let ident = self.consume_identifier()?;
            ids.push(ident.id);

            (false, ident.span.clone(), ident.span)
        };

        loop {
            if let Some(t) = self.peek() {
                if let TkKind::DoubleColon = t.kind {
                    self.next();
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

    pub(crate) fn consume_block_expression_or_statement(
        &mut self,
        ctx: &FnParseCtx,
    ) -> Result<ExprOrStmt<BlockExpr, BlockStmt>, ParseError> {
        let begin = self.must_consume_next(vec![TkKind::LBrace])?.span.clone();

        let mut stmts: Vec<Stmt> = vec![];

        loop {
            if let Some(t) = self.peek().copied()
                && TkKind::RBrace == t.kind
            {
                self.next();

                return Ok(ExprOrStmt::Stmt(BlockStmt {
                    stmts,
                    span: Span::merge(&begin, &t.span),
                }));
            } else {
                match self.consume_expression_or_statement(ctx)? {
                    ExprOrStmt::Expr(expr) => {
                        let end = self.must_consume_next(vec![TkKind::RBrace])?.span.clone();

                        return Ok(ExprOrStmt::Expr(BlockExpr {
                            stmts,
                            expr: Box::new(expr),
                            span: Span::merge(&begin, &end),
                        }));
                    }
                    ExprOrStmt::Stmt(stmt) => {
                        stmts.push(stmt);
                    }
                }
            }
        }
    }

    fn consume_compiler_flag_literal(&mut self) -> Result<CompilerFlagLiteral, ParseError> {
        if let Some(t) = self.next() {
            match t.kind {
                TkKind::IntegerLiteral => Ok(CompilerFlagLiteral::Integer(IntegerLiteral {
                    span: t.span.clone(),
                    val: t.unwrap_integer_value(),
                })),
                TkKind::BoolLiteralTrue => Ok(CompilerFlagLiteral::Bool(BoolLiteral {
                    span: t.span.clone(),
                    val: true,
                })),
                TkKind::BoolLiteralFalse => Ok(CompilerFlagLiteral::Bool(BoolLiteral {
                    span: t.span.clone(),
                    val: false,
                })),
                TkKind::StringLiteral => Ok(CompilerFlagLiteral::String(StringLiteral {
                    span: t.span.clone(),
                    val: t.unwrap_string_value(),
                })),
                _ => Err(ParseError::InvalidToken(
                    vec![
                        TkKind::IntegerLiteral,
                        TkKind::BoolLiteralTrue,
                        TkKind::BoolLiteralFalse,
                        TkKind::StringLiteral,
                    ],
                    t.clone(),
                )),
            }
        } else {
            Err(ParseError::InvalidEOF(vec![
                TkKind::IntegerLiteral,
                TkKind::BoolLiteralTrue,
                TkKind::BoolLiteralFalse,
                TkKind::StringLiteral,
            ]))
        }
    }

    fn consume_compiler_flag_args(&mut self) -> Result<Vec<CompilerFlagArg>, ParseError> {
        let _ = self.must_consume_next(vec![TkKind::LPare])?;

        let mut args = vec![];

        loop {
            if let Some(t) = self.peek().copied()
                && TkKind::RPare == t.kind
            {
                self.next();
                return Ok(args);
            } else {
                let arg = self.consume_identifier()?;

                if let Some(t) = self.peek().copied()
                    && TkKind::Assign == t.kind
                {
                    self.next();

                    let val = self.consume_compiler_flag_literal()?;

                    args.push(CompilerFlagArg {
                        arg,
                        val: Some(val),
                    });

                    if let Some(t) = self.peek().copied() {
                        if TkKind::RPare == t.kind {
                            self.next();
                            return Ok(args);
                        } else if TkKind::Comma == t.kind {
                            self.next();
                            continue;
                        } else {
                            return Err(ParseError::InvalidToken(
                                vec![TkKind::Comma, TkKind::RPare],
                                t.clone(),
                            ));
                        }
                    } else {
                        return Err(ParseError::InvalidEOF(vec![TkKind::Comma, TkKind::RPare]));
                    }
                } else {
                    args.push(CompilerFlagArg { arg, val: None });
                }
            }
        }
    }

    fn opt_consume_compiler_flag(&mut self) -> Result<Option<CompilerFlag>, ParseError> {
        // "[" "[" <identifier> ( "(" ( <identifier> ( "=" <compiler-flag-literal> )? "," )* ")" )? "]" "]"
        if let Some(t) = self.peek().copied()
            && TkKind::LBracket == t.kind
        {
            self.next();
            if let Some(t2) = self.peek().copied()
                && TkKind::LBracket == t2.kind
            {
                self.next();
                let flag = self.consume_identifier()?;

                if let Some(t) = self.peek().copied()
                    && TkKind::LPare == t.kind
                {
                    let args = self.consume_compiler_flag_args()?;

                    let _ = self.must_consume_next(vec![TkKind::RBracket])?;
                    let _ = self.must_consume_next(vec![TkKind::RBracket])?;

                    Ok(Some(CompilerFlag { flag, args }))
                } else {
                    let _ = self.must_consume_next(vec![TkKind::RBracket])?;
                    let _ = self.must_consume_next(vec![TkKind::RBracket])?;

                    Ok(Some(CompilerFlag { flag, args: vec![] }))
                }
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    pub(crate) fn consume_compiler_flags(&mut self) -> Result<Vec<CompilerFlag>, ParseError> {
        let mut flags = vec![];

        while let Some(flag) = self.opt_consume_compiler_flag()? {
            flags.push(flag);
        }

        Ok(flags)
    }
}
