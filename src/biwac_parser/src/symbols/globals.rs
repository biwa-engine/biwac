use biwac_base::Span;
use biwac_lexer::{TkKindName, token::TkKind};

use biwac_ast::{
    ArgDecl, ArgDeclList, CompilerFlag, FnDef, Globals, Ident, ImplCtx, ImportDecl, MethodDef,
    NativeCode, NativeFnDef, NativeMethodDef, NativeTypeAlias, NovelScene, RetTypRepr, StructDef,
    TypRepr, TypeAlias, TypeDef,
};

use crate::{ExprOrStmt, ParseError, TokenStream};

// FnParseCtx
// 関数内のパースをするときのコンテキスト
#[derive(Debug, Clone)]
pub(crate) struct FnParseCtx {
    // implブロック内の関連関数やメソッドのとき、Self型が何の型か
    // パース時点で型を決定してしまう
    // Noneのとき、Selfキーワードは出現するべきでないため、エラー
    pub(crate) self_typ: Option<TypRepr>,
    // is_method trueならselfキーワードを使える
    pub(crate) is_method: bool,
}

impl<'t, 'src> TokenStream<'t, 'src> {
    fn consume_function_or_method_definition(
        &mut self,
        flags: Vec<CompilerFlag>,
        impl_ctx: Option<ImplCtx>,
    ) -> Result<Globals, ParseError<'src>> {
        let begin = self.must_consume_next(vec![TkKindName::KwFn])?.span.clone();

        let id = self.consume_identifier()?;

        let genargs = self.opt_consume_generic_argument_declaration()?;

        // 実装対象の型typがSomeならメソッドである可能性がある
        let (args, self_ident) = if let Some(impl_ctx) = &impl_ctx {
            self.consume_method_argsdec(&Some(impl_ctx.self_typ.clone()))?
        } else {
            (self.consume_argsdec(&None)?, None)
        };

        let rtype =
            if self
                .consume_next_if_match(vec![TkKindName::MarkArrow])
                .is_some()
            {
                RetTypRepr::Typ(self.consume_type_representaion(
                    &impl_ctx.as_ref().map(|ctx| ctx.self_typ.clone()),
                )?)
            } else {
                RetTypRepr::Void(Span::new(
                    args.span.module(),
                    args.span.end(),
                    args.span.end(),
                ))
            };

        if flags.iter().any(|f| &f.flag.id == "native") {
            if let Some(t) = self.next() {
                if let TkKind::DslLiteral(str) = t.kind {
                    if let Some(impl_ctx) = impl_ctx {
                        if let Some(self_ident) = self_ident {
                            Ok(Globals::NativeMethodDef(NativeMethodDef {
                                impl_genargs: impl_ctx.genargs,
                                self_typ: impl_ctx.self_typ,
                                self_ident,
                                id,
                                args,
                                rtype,
                                native: str.to_string(),
                                native_span: t.span.clone(),
                                span: Span::merge(&begin, &t.span),
                                flags,
                                genargs,
                            }))
                        } else {
                            Ok(Globals::NativeFnDef(NativeFnDef {
                                impl_ctx: Some(impl_ctx),
                                id,
                                args,
                                native: str.to_string(),
                                rtype,
                                span: Span::merge(&begin, &t.span),
                                native_span: t.span.clone(),
                                flags,
                                genargs,
                            }))
                        }
                    } else {
                        Ok(Globals::NativeFnDef(NativeFnDef {
                            impl_ctx: None,
                            id,
                            args,
                            native: str.to_string(),
                            rtype,
                            span: Span::merge(&begin, &t.span),
                            native_span: t.span.clone(),
                            flags,
                            genargs,
                        }))
                    }
                } else {
                    Err(ParseError::InvalidToken {
                        expecteds: vec![TkKindName::DslLiteral],
                        found: t.clone(),
                    })
                }
            } else {
                Err(ParseError::InvalidEOF {
                    expecteds: vec![TkKindName::DslLiteral],
                })
            }
        } else {
            // Self型を表すtypをコンテキストとして渡してパースする
            let ctx = FnParseCtx {
                self_typ: impl_ctx.as_ref().map(|ctx| ctx.self_typ.clone()),
                is_method: self_ident.is_some(),
            };
            let (stmts, expr, end) = match self.consume_block_expression_or_statement(&ctx)? {
                ExprOrStmt::Expr(block_expr) => {
                    (block_expr.stmts, Some(*block_expr.expr), block_expr.span)
                }
                ExprOrStmt::Stmt(block_stmt) => (block_stmt.stmts, None, block_stmt.span),
            };

            if let Some(self_ident) = self_ident {
                // SAFETY: self_ident Someになるのはimpl_ctx Someのときだけ
                let impl_ctx = impl_ctx.expect("compiler bug: not a method");

                Ok(Globals::MethodDef(MethodDef {
                    impl_genargs: impl_ctx.genargs,
                    self_typ: impl_ctx.self_typ,
                    self_ident,
                    id,
                    args,
                    stmts,
                    expr,
                    rtype,
                    span: Span::merge(&begin, &end),
                    flags,
                    genargs,
                }))
            } else {
                Ok(Globals::FnDef(FnDef {
                    impl_ctx,
                    id,
                    args,
                    stmts,
                    expr,
                    rtype,
                    span: Span::merge(&begin, &end),
                    flags,
                    genargs,
                }))
            }
        }
    }

    pub(super) fn opt_consume_global_symbols(&mut self) -> Result<Vec<Globals>, ParseError<'src>> {
        let flags = self.consume_compiler_flags()?;

        if let Some(t) = self.peek() {
            match t.kind {
                TkKind::KwImport => {
                    // "import" <qualified-identifier> ";"
                    let begin = t.span.clone();
                    self.next();
                    let qualid = self.consume_qualified_identifier()?;

                    // ";"
                    let end = self.must_consume_semicolon()?.span.clone();

                    Ok(vec![Globals::Import(ImportDecl {
                        qualid,
                        span: Span::merge(&begin, &end),
                    })])
                }
                TkKind::KwFn => Ok(vec![
                    self.consume_function_or_method_definition(flags, None)?,
                ]),
                TkKind::KwLet => Ok(vec![Globals::VarDecl(
                    // グローバル変数のパースには当然関数内の文脈を与えない
                    // constキーワードのみのほうが良いかも
                    self.consume_variable_declaration_statment(None)?,
                )]),
                TkKind::KwStruct => {
                    self.next();

                    let id = self.consume_identifier()?;

                    let genargs = self.opt_consume_generic_argument_declaration()?;

                    let _ = self.must_consume_next(vec![TkKindName::MarkLBrace])?;

                    let mut members = vec![];

                    loop {
                        let t = self.peek().ok_or(ParseError::InvalidEOF {
                            expecteds: vec![TkKindName::Ident, TkKindName::MarkRBrace],
                        })?;

                        if let TkKind::MarkRBrace = t.kind {
                            self.next();

                            return Ok(vec![Globals::TypeDef(TypeDef::Struct(StructDef {
                                id,
                                members,
                                genargs,
                            }))]);
                        } else {
                            let t = self
                                .next()
                                .ok_or(ParseError::InvalidEOF {
                                    expecteds: vec![TkKindName::Ident],
                                })?
                                .to_owned();
                            if let TkKind::Ident(memberid) = &t.kind {
                                let t = t.clone();
                                // WARN: really?
                                let typ = self.must_consume_type_annotation(&None)?;

                                members.push((
                                    Ident {
                                        id: memberid.to_string(),
                                        span: t.span,
                                    },
                                    typ,
                                ));

                                let t = self.must_consume_next(vec![
                                    TkKindName::MarkComma,
                                    TkKindName::MarkRBrace,
                                ])?;
                                if let TkKind::MarkComma = t.kind {
                                    continue;
                                } else if let TkKind::MarkRBrace = t.kind {
                                    return Ok(vec![Globals::TypeDef(TypeDef::Struct(
                                        StructDef {
                                            id,
                                            members,
                                            genargs,
                                        },
                                    ))]);
                                }
                            } else {
                                return Err(ParseError::InvalidToken {
                                    expecteds: vec![TkKindName::Ident],
                                    found: t.clone(),
                                });
                            }
                        }
                    }
                }
                TkKind::KwType => {
                    if flags.iter().any(|f| &f.flag.id == "native") {
                        // "type" <identifier> ( <generic-argument-declaration> )?
                        //     "=" {{
                        //         native type implementation
                        //     }} ";"

                        self.next();

                        let ident = self.consume_identifier()?;

                        let genargs = self.opt_consume_generic_argument_declaration()?;

                        let _ = self.must_consume_next(vec![TkKindName::MarkAssign])?;

                        let t = self.next().ok_or(ParseError::InvalidEOF {
                            expecteds: vec![TkKindName::DslLiteral],
                        })?;
                        if let TkKind::DslLiteral(str) = t.kind {
                            let native = str.to_string();
                            let native_span = t.span.clone();

                            let _ = self.must_consume_next(vec![TkKindName::MarkSemiColon])?;

                            Ok(vec![Globals::TypeDef(TypeDef::NativeTypeAlias(
                                NativeTypeAlias {
                                    ident,
                                    genargs,
                                    native,
                                    native_span,
                                },
                            ))])
                        } else {
                            Err(ParseError::InvalidEOF {
                                expecteds: vec![TkKindName::DslLiteral],
                            })
                        }
                    } else {
                        // "type" <identifier> ( <generic-argument-declaration> )? "=" <type-representation> ";"
                        self.next();

                        let ident = self.consume_identifier()?;

                        let genargs = self.opt_consume_generic_argument_declaration()?;

                        let _ = self.must_consume_next(vec![TkKindName::MarkAssign])?;

                        let right = self.consume_type_representaion(&None)?;

                        let _ = self.must_consume_next(vec![TkKindName::MarkSemiColon])?;

                        Ok(vec![Globals::TypeDef(TypeDef::TypeAlias(TypeAlias {
                            ident,
                            genargs,
                            right,
                        }))])
                    }
                }
                TkKind::KwImpl => {
                    // "impl" ( <generic-argument-declaration> )? <type-representation> "{" ... "}"
                    self.next();

                    let genargs = self.opt_consume_generic_argument_declaration()?;

                    let self_typ = self.consume_type_representaion(&None)?;

                    self.must_consume_next(vec![TkKindName::MarkLBrace])?;

                    let mut type_impls = vec![];

                    loop {
                        if let Some(t) = self.peek().copied()
                            && TkKind::MarkRBrace == t.kind
                        {
                            self.next();

                            return Ok(type_impls);
                        } else {
                            let flags = self.consume_compiler_flags()?;
                            let f = self.consume_function_or_method_definition(
                                flags,
                                Some(ImplCtx {
                                    genargs: genargs.clone(),
                                    self_typ: self_typ.clone(),
                                }),
                            )?;

                            type_impls.push(f);
                        }
                    }
                }
                TkKind::DslLiteral(str) => {
                    let native = str.to_string();
                    let native_span = t.span.clone();
                    self.next();

                    Ok(vec![Globals::NativeCode(NativeCode {
                        native,
                        native_span,
                        flags,
                    })])
                }
                TkKind::KwScene => {
                    let begin = t.span.clone();
                    self.next();

                    let id = self.consume_identifier()?;

                    let args = self.consume_argsdec(&None)?;

                    let rtype = if self
                        .consume_next_if_match(vec![TkKindName::MarkArrow])
                        .is_some()
                    {
                        RetTypRepr::Typ(self.consume_type_representaion(&None)?)
                    } else {
                        RetTypRepr::Void(Span::new(
                            args.span.module(),
                            args.span.end(),
                            args.span.end(),
                        ))
                    };

                    let t = self.next().ok_or(ParseError::InvalidEOF {
                        expecteds: vec![TkKindName::DslLiteral],
                    })?;
                    if let TkKind::DslLiteral(str) = t.kind {
                        let novel_stmts =
                            biwac_novel_parser::NovelSourceStream::new(str, t.span.clone())
                                .parse()
                                .map_err(ParseError::NovelParseError)?;

                        Ok(vec![Globals::NovelScene(NovelScene {
                            id,
                            args,
                            rtype,
                            stmts: novel_stmts,
                            span: Span::merge(&begin, &t.span),
                            flags,
                        })])
                    } else {
                        Err(ParseError::InvalidEOF {
                            expecteds: vec![TkKindName::DslLiteral],
                        })
                    }
                }
                _ => Err(ParseError::InvalidToken {
                    expecteds: vec![
                        TkKindName::KwFn,
                        TkKindName::KwLet,
                        TkKindName::KwStruct,
                        TkKindName::KwImport,
                        TkKindName::KwImpl,
                        TkKindName::KwScene,
                    ],
                    found: t.to_owned().clone(),
                }),
            }
        } else {
            Ok(vec![])
        }
    }

    pub(crate) fn consume_argsdec(
        &mut self,
        self_typ: &Option<TypRepr>,
    ) -> Result<ArgDeclList, ParseError<'src>> {
        let begin = self
            .must_consume_next(vec![TkKindName::MarkLPare])?
            .span
            .clone();

        let mut args = vec![];

        loop {
            let t = self
                .next()
                .ok_or(ParseError::InvalidEOF {
                    expecteds: vec![TkKindName::MarkRPare, TkKindName::Ident],
                })?
                .clone();

            if let TkKind::MarkRPare = t.kind {
                return Ok(ArgDeclList {
                    args,
                    span: Span::merge(&begin, &t.span),
                });
            } else if let TkKind::Ident(arg) = &t.kind {
                let typ = self.must_consume_type_annotation(self_typ)?;

                args.push(ArgDecl {
                    span: Span::merge(&t.span, &typ.span),
                    typ,
                    id: Ident {
                        id: arg.to_string(),
                        span: t.span.clone(),
                    },
                });

                if let Some(t) = self.peek() {
                    if let TkKind::MarkComma = t.kind {
                        self.next();
                    } else if let TkKind::MarkRPare = t.kind {
                        continue;
                    } else {
                        return Err(ParseError::InvalidToken {
                            expecteds: vec![TkKindName::MarkComma, TkKindName::MarkRPare],
                            found: t.to_owned().clone(),
                        });
                    }
                } else {
                    return Err(ParseError::InvalidEOF {
                        expecteds: vec![TkKindName::MarkComma, TkKindName::MarkRPare],
                    });
                }
            }
        }
    }

    pub(crate) fn consume_method_argsdec(
        &mut self,
        self_typ: &Option<TypRepr>,
    ) -> Result<(ArgDeclList, Option<Ident>), ParseError<'src>> {
        // (args, self_ident)
        let begin = self
            .must_consume_next(vec![TkKindName::MarkLPare])?
            .span
            .clone();

        let mut args = vec![];

        // first arg `self` or not
        let t = self.peek().ok_or(ParseError::InvalidEOF {
            expecteds: vec![
                TkKindName::MarkRPare,
                TkKindName::Ident,
                TkKindName::KwSelfVar,
            ],
        })?;

        let self_ident = match t.kind {
            TkKind::MarkRPare => {
                let end = t.span.clone();
                self.next();

                return Ok((
                    ArgDeclList {
                        args,
                        span: Span::merge(&begin, &end),
                    },
                    None,
                ));
            }
            TkKind::KwSelfVar => {
                let span = t.span.clone();
                self.next();

                Some(Ident {
                    id: "self".to_string(),
                    span,
                })
            }
            _ => None,
        };

        loop {
            let t = self
                .next()
                .ok_or(ParseError::InvalidEOF {
                    expecteds: vec![TkKindName::MarkRPare, TkKindName::Ident],
                })?
                .clone();

            if let TkKind::MarkRPare = t.kind {
                return Ok((
                    ArgDeclList {
                        args,
                        span: Span::merge(&begin, &t.span),
                    },
                    self_ident,
                ));
            } else if let TkKind::Ident(arg) = &t.kind {
                let typ = self.must_consume_type_annotation(self_typ)?;

                args.push(ArgDecl {
                    span: Span::merge(&t.span, &typ.span),
                    typ,
                    id: Ident {
                        id: arg.to_string(),
                        span: t.span.clone(),
                    },
                });

                if let Some(t) = self.peek() {
                    if let TkKind::MarkComma = t.kind {
                        self.next();
                    } else if let TkKind::MarkRPare = t.kind {
                        continue;
                    } else {
                        return Err(ParseError::InvalidToken {
                            expecteds: vec![TkKindName::MarkComma, TkKindName::MarkRPare],
                            found: t.to_owned().clone(),
                        });
                    }
                } else {
                    return Err(ParseError::InvalidEOF {
                        expecteds: vec![TkKindName::MarkComma, TkKindName::MarkRPare],
                    });
                }
            }
        }
    }
}
