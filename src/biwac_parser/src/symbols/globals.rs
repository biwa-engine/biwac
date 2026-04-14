use biwac_base::Span;
use biwac_lexer::token::{TkKind, TkVal};

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

impl<'t> TokenStream<'t> {
    fn consume_function_or_method_definition(
        &mut self,
        flags: Vec<CompilerFlag>,
        impl_ctx: Option<ImplCtx>,
    ) -> Result<Globals, ParseError> {
        let begin = self.must_consume_next(vec![TkKind::Fn])?.span.clone();

        let id = self.consume_identifier()?;

        let genargs = self.opt_consume_generic_argument_declaration()?;

        // 実装対象の型typがSomeならメソッドである可能性がある
        let (args, self_ident) = if let Some(impl_ctx) = &impl_ctx {
            self.consume_method_argsdec(&Some(impl_ctx.self_typ.clone()))?
        } else {
            (self.consume_argsdec(&None)?, None)
        };

        let rtype =
            if self.consume_next_if_match(vec![TkKind::Arrow]).is_some() {
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
                if TkKind::DslLiteral == t.kind {
                    if let Some(impl_ctx) = impl_ctx {
                        if let Some(self_ident) = self_ident {
                            Ok(Globals::NativeMethodDef(NativeMethodDef {
                                impl_genargs: impl_ctx.genargs,
                                self_typ: impl_ctx.self_typ,
                                self_ident,
                                id,
                                args,
                                rtype,
                                native: t.unwrap_string_value(),
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
                                native: t.unwrap_string_value(),
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
                            native: t.unwrap_string_value(),
                            rtype,
                            span: Span::merge(&begin, &t.span),
                            native_span: t.span.clone(),
                            flags,
                            genargs,
                        }))
                    }
                } else {
                    Err(ParseError::InvalidToken(
                        vec![TkKind::DslLiteral],
                        t.clone(),
                    ))
                }
            } else {
                Err(ParseError::InvalidEOF(vec![TkKind::DslLiteral]))
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

    pub(super) fn opt_consume_global_symbols(&mut self) -> Result<Vec<Globals>, ParseError> {
        let flags = self.consume_compiler_flags()?;

        if let Some(t) = self.peek() {
            match t.kind {
                TkKind::Import => {
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
                TkKind::Fn => Ok(vec![
                    self.consume_function_or_method_definition(flags, None)?,
                ]),
                TkKind::Let => Ok(vec![Globals::VarDecl(
                    // グローバル変数のパースには当然関数内の文脈を与えない
                    // constキーワードのみのほうが良いかも
                    self.consume_variable_declaration_statment(None)?,
                )]),
                TkKind::Struct => {
                    self.next();

                    let id = self.consume_identifier()?;

                    let genargs = self.opt_consume_generic_argument_declaration()?;

                    let _ = self.must_consume_next(vec![TkKind::LBrace])?;

                    let mut members = vec![];

                    loop {
                        let t = self
                            .peek()
                            .ok_or(ParseError::InvalidEOF(vec![TkKind::Ident, TkKind::RBrace]))?;

                        if let TkKind::RBrace = t.kind {
                            self.next();

                            return Ok(vec![Globals::TypeDef(TypeDef::Struct(StructDef {
                                id,
                                members,
                                genargs,
                            }))]);
                        } else {
                            let t = self
                                .next()
                                .ok_or(ParseError::InvalidEOF(vec![TkKind::Ident]))?;
                            if let TkKind::Ident = &t.kind
                                && let Some(TkVal::String(memberid)) = t.val.clone()
                            {
                                let t = t.clone();
                                // WARN: really?
                                let typ = self.must_consume_type_annotation(&None)?;

                                members.push((
                                    Ident {
                                        id: memberid,
                                        span: t.span,
                                    },
                                    typ,
                                ));

                                let t =
                                    self.must_consume_next(vec![TkKind::Comma, TkKind::RBrace])?;
                                if let TkKind::Comma = t.kind {
                                    continue;
                                } else if let TkKind::RBrace = t.kind {
                                    return Ok(vec![Globals::TypeDef(TypeDef::Struct(
                                        StructDef {
                                            id,
                                            members,
                                            genargs,
                                        },
                                    ))]);
                                }
                            } else {
                                return Err(ParseError::InvalidToken(
                                    vec![TkKind::Ident],
                                    t.clone(),
                                ));
                            }
                        }
                    }
                }
                TkKind::Type => {
                    if flags.iter().any(|f| &f.flag.id == "native") {
                        // "type" <identifier> ( <generic-argument-declaration> )?
                        //     "=" {{
                        //         native type implementation
                        //     }} ";"

                        self.next();

                        let ident = self.consume_identifier()?;

                        let genargs = self.opt_consume_generic_argument_declaration()?;

                        let _ = self.must_consume_next(vec![TkKind::Assign])?;

                        let t = self.must_consume_next(vec![TkKind::DslLiteral])?;
                        let native = t.unwrap_string_value();
                        let native_span = t.span.clone();

                        let _ = self.must_consume_next(vec![TkKind::SemiColon])?;

                        Ok(vec![Globals::TypeDef(TypeDef::NativeTypeAlias(
                            NativeTypeAlias {
                                ident,
                                genargs,
                                native,
                                native_span,
                            },
                        ))])
                    } else {
                        // "type" <identifier> ( <generic-argument-declaration> )? "=" <type-representation> ";"
                        self.next();

                        let ident = self.consume_identifier()?;

                        let genargs = self.opt_consume_generic_argument_declaration()?;

                        let _ = self.must_consume_next(vec![TkKind::Assign])?;

                        let right = self.consume_type_representaion(&None)?;

                        let _ = self.must_consume_next(vec![TkKind::SemiColon])?;

                        Ok(vec![Globals::TypeDef(TypeDef::TypeAlias(TypeAlias {
                            ident,
                            genargs,
                            right,
                        }))])
                    }
                }
                TkKind::Impl => {
                    // "impl" ( <generic-argument-declaration> )? <type-representation> "{" ... "}"
                    self.next();

                    let genargs = self.opt_consume_generic_argument_declaration()?;

                    let self_typ = self.consume_type_representaion(&None)?;

                    self.must_consume_next(vec![TkKind::LBrace])?;

                    let mut type_impls = vec![];

                    loop {
                        if let Some(t) = self.peek().copied()
                            && TkKind::RBrace == t.kind
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
                TkKind::DslLiteral => {
                    let native = t.unwrap_string_value();
                    let native_span = t.span.clone();
                    self.next();

                    Ok(vec![Globals::NativeCode(NativeCode {
                        native,
                        native_span,
                        flags,
                    })])
                }
                TkKind::Scene => {
                    let begin = t.span.clone();
                    self.next();

                    let id = self.consume_identifier()?;

                    let args = self.consume_argsdec(&None)?;

                    let rtype = if self.consume_next_if_match(vec![TkKind::Arrow]).is_some() {
                        RetTypRepr::Typ(self.consume_type_representaion(&None)?)
                    } else {
                        RetTypRepr::Void(Span::new(
                            args.span.module(),
                            args.span.end(),
                            args.span.end(),
                        ))
                    };

                    let dsl = self.must_consume_next(vec![TkKind::DslLiteral])?;
                    let novel_stmts = biwac_novel_parser::NovelSourceStream::new(
                        &dsl.unwrap_string_value(),
                        dsl.span.clone(),
                    )
                    .parse()
                    .map_err(ParseError::NovelParseError)?;

                    Ok(vec![Globals::NovelScene(NovelScene {
                        id,
                        args,
                        rtype,
                        stmts: novel_stmts,
                        span: Span::merge(&begin, &dsl.span),
                        flags,
                    })])
                }
                _ => Err(ParseError::InvalidToken(
                    vec![TkKind::Fn, TkKind::Let, TkKind::Struct],
                    t.to_owned().clone(),
                )),
            }
        } else {
            Ok(vec![])
        }
    }

    pub(crate) fn consume_argsdec(
        &mut self,
        self_typ: &Option<TypRepr>,
    ) -> Result<ArgDeclList, ParseError> {
        let begin = self.must_consume_next(vec![TkKind::LPare])?.span.clone();

        let mut args = vec![];

        loop {
            let t = self
                .next()
                .ok_or(ParseError::InvalidEOF(vec![TkKind::RPare, TkKind::Ident]))?
                .clone();

            if let TkKind::RPare = t.kind {
                return Ok(ArgDeclList {
                    args,
                    span: Span::merge(&begin, &t.span),
                });
            } else if let TkKind::Ident = &t.kind
                && let Some(TkVal::String(arg)) = &t.val
            {
                let typ = self.must_consume_type_annotation(self_typ)?;

                args.push(ArgDecl {
                    span: Span::merge(&t.span, &typ.span),
                    typ,
                    id: Ident {
                        id: arg.clone(),
                        span: t.span.clone(),
                    },
                });

                if let Some(t) = self.peek() {
                    if let TkKind::Comma = t.kind {
                        self.next();
                    } else if let TkKind::RPare = t.kind {
                        continue;
                    } else {
                        return Err(ParseError::InvalidToken(
                            vec![TkKind::Comma, TkKind::RPare],
                            t.to_owned().clone(),
                        ));
                    }
                } else {
                    return Err(ParseError::InvalidEOF(vec![TkKind::Comma, TkKind::RPare]));
                }
            }
        }
    }

    pub(crate) fn consume_method_argsdec(
        &mut self,
        self_typ: &Option<TypRepr>,
    ) -> Result<(ArgDeclList, Option<Ident>), ParseError> {
        // (args, self_ident)
        let begin = self.must_consume_next(vec![TkKind::LPare])?.span.clone();

        let mut args = vec![];

        // first arg `self` or not
        let t = self.peek().ok_or(ParseError::InvalidEOF(vec![
            TkKind::RPare,
            TkKind::Ident,
            TkKind::SelfVar,
        ]))?;

        let self_ident = match t.kind {
            TkKind::RPare => {
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
            TkKind::SelfVar => {
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
                .ok_or(ParseError::InvalidEOF(vec![TkKind::RPare, TkKind::Ident]))?
                .clone();

            if let TkKind::RPare = t.kind {
                return Ok((
                    ArgDeclList {
                        args,
                        span: Span::merge(&begin, &t.span),
                    },
                    self_ident,
                ));
            } else if let TkKind::Ident = &t.kind
                && let Some(TkVal::String(arg)) = &t.val
            {
                let typ = self.must_consume_type_annotation(self_typ)?;

                args.push(ArgDecl {
                    span: Span::merge(&t.span, &typ.span),
                    typ,
                    id: Ident {
                        id: arg.clone(),
                        span: t.span.clone(),
                    },
                });

                if let Some(t) = self.peek() {
                    if let TkKind::Comma = t.kind {
                        self.next();
                    } else if let TkKind::RPare = t.kind {
                        continue;
                    } else {
                        return Err(ParseError::InvalidToken(
                            vec![TkKind::Comma, TkKind::RPare],
                            t.to_owned().clone(),
                        ));
                    }
                } else {
                    return Err(ParseError::InvalidEOF(vec![TkKind::Comma, TkKind::RPare]));
                }
            }
        }
    }
}
