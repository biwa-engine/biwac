//! enum と `match` の TypeScript 出力。
//!
//! enum は判別可能なユニオンにする。
//!
//! ```ts
//! type Color = { __tag: 0 } | { __tag: 1; _0: number };
//! ```
//!
//! `__tag` は数値リテラル型なので、`if (c.__tag === 0)` と書けば
//! TypeScript 自身が絞り込む。payload の読み出しにキャストが要らない。
//!
//! `match` は if / else if の連鎖に落とす。
//! `switch` にすると `break` とスコープの扱いが増えるだけで、得るものがない。

use std::cell::Cell;

use biwac_hir::{
    BlockExpr, EnumDef, MatchExpr, MatchStmt, Pattern, PatternFields, ResolvedVariant, VariantCtor,
    VariantCtorFields,
};
use biwac_span::VarId;

use crate::arch::typescript::{
    AsOxc, AsOxcGlobal, AsOxcLocal, AstBuildCtx, FnAstBuildCtx, Mangled, span,
};

/// タグを入れるプロパティの名前。
///
/// biwa の識別子は `_` 2 つで始まらないので、利用者のフィールドと衝突しない。
pub(super) const TAG: &str = "__tag";

// ---------------------------------------------------------------
// 型宣言
// ---------------------------------------------------------------

impl<'a> AsOxcGlobal<'a, oxc_ast::ast::Statement<'a>> for EnumDef {
    fn as_oxc_global(&'a self, id: String, ctx: &'a AstBuildCtx) -> oxc_ast::ast::Statement<'a> {
        let variants = oxc_allocator::Vec::from_iter_in(
            self.variants.iter().enumerate().map(|(index, variant)| {
                let mut members = oxc_allocator::Vec::new_in(ctx.allocator);
                members.push(property_signature(
                    ctx,
                    TAG,
                    numeric_literal_ty(ctx, index as f64),
                ));
                for (name, ty) in &variant.fields {
                    members.push(property_signature(
                        ctx,
                        ctx.str_of(&name.id),
                        ty.kind.as_oxc(ctx),
                    ));
                }

                oxc_ast::ast::TSType::TSTypeLiteral(oxc_allocator::Box::new_in(
                    oxc_ast::ast::TSTypeLiteral {
                        span: span(),
                        members,
                    },
                    ctx.allocator,
                ))
            }),
            ctx.allocator,
        );

        let union = oxc_ast::ast::TSType::TSUnionType(oxc_allocator::Box::new_in(
            oxc_ast::ast::TSUnionType {
                span: span(),
                types: variants,
            },
            ctx.allocator,
        ));

        oxc_ast::ast::Statement::TSTypeAliasDeclaration(oxc_allocator::Box::new_in(
            oxc_ast::ast::TSTypeAliasDeclaration {
                span: span(),
                id: oxc_ast::ast::BindingIdentifier {
                    span: span(),
                    name: oxc_span::Ident::new_const(ctx.allocator.alloc_str(&id)),
                    symbol_id: Cell::new(None),
                },
                type_parameters: if self.genargs.is_empty() {
                    None
                } else {
                    Some(oxc_allocator::Box::new_in(
                        oxc_ast::ast::TSTypeParameterDeclaration {
                            span: span(),
                            params: oxc_allocator::Vec::from_iter_in(
                                self.genargs
                                    .iter()
                                    .map(|gid| oxc_ast::ast::TSTypeParameter {
                                        span: span(),
                                        name: oxc_ast::ast::BindingIdentifier {
                                            span: span(),
                                            name: oxc_span::Ident::new_const(
                                                ctx.allocator.alloc_str(&gid.mangled(ctx)),
                                            ),
                                            symbol_id: Cell::new(None),
                                        },
                                        constraint: None,
                                        default: None,
                                        r#in: false,
                                        out: false,
                                        r#const: false,
                                    }),
                                ctx.allocator,
                            ),
                        },
                        ctx.allocator,
                    ))
                },
                type_annotation: union,
                declare: false,
                scope_id: Cell::new(None),
            },
            ctx.allocator,
        ))
    }
}

fn property_signature<'a>(
    ctx: &'a AstBuildCtx<'a>,
    name: &str,
    ty: oxc_ast::ast::TSType<'a>,
) -> oxc_ast::ast::TSSignature<'a> {
    oxc_ast::ast::TSSignature::TSPropertySignature(oxc_allocator::Box::new_in(
        oxc_ast::ast::TSPropertySignature {
            span: span(),
            computed: false,
            optional: false,
            readonly: false,
            key: oxc_ast::ast::PropertyKey::StaticIdentifier(oxc_allocator::Box::new_in(
                oxc_ast::ast::IdentifierName {
                    span: span(),
                    name: oxc_span::Ident::new_const(ctx.allocator.alloc_str(name)),
                },
                ctx.allocator,
            )),
            type_annotation: Some(oxc_allocator::Box::new_in(
                oxc_ast::ast::TSTypeAnnotation {
                    span: span(),
                    type_annotation: ty,
                },
                ctx.allocator,
            )),
        },
        ctx.allocator,
    ))
}

/// `0` のような数値リテラル型。タグの絞り込みに要る。
fn numeric_literal_ty<'a>(ctx: &'a AstBuildCtx<'a>, value: f64) -> oxc_ast::ast::TSType<'a> {
    oxc_ast::ast::TSType::TSLiteralType(oxc_allocator::Box::new_in(
        oxc_ast::ast::TSLiteralType {
            span: span(),
            literal: oxc_ast::ast::TSLiteral::NumericLiteral(oxc_allocator::Box::new_in(
                oxc_ast::ast::NumericLiteral {
                    span: span(),
                    value,
                    raw: None,
                    base: oxc_ast::ast::NumberBase::Decimal,
                },
                ctx.allocator,
            )),
        },
        ctx.allocator,
    ))
}

// ---------------------------------------------------------------
// 構築
// ---------------------------------------------------------------

impl<'a> AsOxcLocal<'a, oxc_ast::ast::Expression<'a>> for VariantCtor {
    fn as_oxc_local(
        &'a self,
        ctx: &'a AstBuildCtx<'a>,
        fctx: &mut FnAstBuildCtx<'a>,
    ) -> oxc_ast::ast::Expression<'a> {
        let resolved = self
            .resolved
            .get()
            .expect("compiler bug: variant ctor was not resolved by inference");

        let mut properties = oxc_allocator::Vec::new_in(ctx.allocator);
        properties.push(object_property(
            ctx,
            TAG,
            number_expr(ctx, resolved.index as f64),
        ));

        // 宣言順に並べる。名前つきで書かれていても並びは宣言順である。
        match &self.fields {
            VariantCtorFields::Unit => {}
            VariantCtorFields::Positional(args) => {
                for (name, arg) in resolved.field_names.iter().zip(args) {
                    properties.push(object_property(
                        ctx,
                        ctx.str_of(name),
                        arg.as_oxc_local(ctx, fctx),
                    ));
                }
            }
            VariantCtorFields::Named(args) => {
                for name in &resolved.field_names {
                    let arg = args
                        .iter()
                        .find(|(ident, _)| ident.id == *name)
                        .map(|(_, expr)| expr)
                        .expect("compiler bug: a variant field is missing after inference");
                    properties.push(object_property(
                        ctx,
                        ctx.str_of(name),
                        arg.as_oxc_local(ctx, fctx),
                    ));
                }
            }
        }

        oxc_ast::ast::Expression::ObjectExpression(oxc_allocator::Box::new_in(
            oxc_ast::ast::ObjectExpression {
                span: span(),
                properties,
            },
            ctx.allocator,
        ))
    }
}

fn object_property<'a>(
    ctx: &'a AstBuildCtx<'a>,
    name: &str,
    value: oxc_ast::ast::Expression<'a>,
) -> oxc_ast::ast::ObjectPropertyKind<'a> {
    oxc_ast::ast::ObjectPropertyKind::ObjectProperty(oxc_allocator::Box::new_in(
        oxc_ast::ast::ObjectProperty {
            span: span(),
            kind: oxc_ast::ast::PropertyKind::Init,
            key: oxc_ast::ast::PropertyKey::StaticIdentifier(oxc_allocator::Box::new_in(
                oxc_ast::ast::IdentifierName {
                    span: span(),
                    name: oxc_span::Ident::new_const(ctx.allocator.alloc_str(name)),
                },
                ctx.allocator,
            )),
            value,
            method: false,
            shorthand: false,
            computed: false,
        },
        ctx.allocator,
    ))
}

// ---------------------------------------------------------------
// match
// ---------------------------------------------------------------

impl<'a> AsOxcLocal<'a, oxc_ast::ast::Statement<'a>> for MatchStmt {
    fn as_oxc_local(
        &'a self,
        ctx: &'a AstBuildCtx<'a>,
        fctx: &mut FnAstBuildCtx<'a>,
    ) -> oxc_ast::ast::Statement<'a> {
        let scrutinee = self.scrutinee.as_oxc_local(ctx, fctx);
        let subject = fctx.alloc_temp();
        let subject_ty = scrutinee_ty(ctx, fctx, &self.scrutinee);
        fctx.stmts
            .push(let_stmt(ctx, &subject, Some(scrutinee), subject_ty));

        let arms: Vec<(&Pattern, Vec<oxc_ast::ast::Statement<'a>>)> = self
            .arms
            .iter()
            .map(|arm| {
                let body = arm_stmts(ctx, fctx, &subject, &arm.pattern, |ctx, fctx| {
                    arm.body
                        .stmts
                        .iter()
                        .map(|s| s.as_oxc_local(ctx, fctx))
                        .collect()
                });
                (&arm.pattern, body)
            })
            .collect();

        build_arm_chain(ctx, &subject, arms)
    }
}

/// 式形の `match`。
///
/// 値を受ける一時変数を先に宣言し、各アームでそこへ代入する。
/// 呼び出し側にはその変数を返す。
pub(super) fn match_expr_as_oxc<'a>(
    m: &'a MatchExpr,
    expr_id: biwac_hir::ExprId,
    ctx: &'a AstBuildCtx<'a>,
    fctx: &mut FnAstBuildCtx<'a>,
) -> oxc_ast::ast::Expression<'a> {
    let scrutinee = m.scrutinee.as_oxc_local(ctx, fctx);
    let subject = fctx.alloc_temp();
    let subject_ty = scrutinee_ty(ctx, fctx, &m.scrutinee);
    fctx.stmts
        .push(let_stmt(ctx, &subject, Some(scrutinee), subject_ty));

    // 結果の型も注釈する。無いと、アームが返す値から
    // `{ __tag: number }` のように広がった型が推論されてしまう。
    let result = fctx.alloc_temp();
    let result_ty = fctx.expr_tys.get(&expr_id).map(|ty| ty.kind.as_oxc(ctx));
    fctx.stmts.push(let_stmt(ctx, &result, None, result_ty));

    let arms: Vec<(&Pattern, Vec<oxc_ast::ast::Statement<'a>>)> = m
        .arms
        .iter()
        .map(|arm| {
            let body = arm_stmts(ctx, fctx, &subject, &arm.pattern, |ctx, fctx| {
                block_expr_into(ctx, fctx, &arm.body, &result)
            });
            (&arm.pattern, body)
        })
        .collect();

    let chain = build_arm_chain(ctx, &subject, arms);
    fctx.stmts.push(chain);

    ident_expr(ctx, &result)
}

/// 対象を受ける一時変数の型注釈。
///
/// 注釈が無いと、TypeScript は初期化式から絞り込んだ型を推論してしまう。
/// `let c: Color = { __tag: 1, .. }` を受けた変数が
/// `{ __tag: 1, .. }` になり、他のアームの比較が
/// 「重なりがない」と怒られる。enum 全体の型を明示して防ぐ。
fn scrutinee_ty<'a>(
    ctx: &'a AstBuildCtx<'a>,
    fctx: &FnAstBuildCtx<'a>,
    scrutinee: &'a biwac_hir::Expr,
) -> Option<oxc_ast::ast::TSType<'a>> {
    fctx.expr_tys
        .get(&scrutinee.id)
        .map(|ty| ty.kind.as_oxc(ctx))
}

/// ブロック式を「最後の値を `dest` に代入する文の並び」にする。
fn block_expr_into<'a>(
    ctx: &'a AstBuildCtx<'a>,
    fctx: &mut FnAstBuildCtx<'a>,
    block: &'a BlockExpr,
    dest: &str,
) -> Vec<oxc_ast::ast::Statement<'a>> {
    // ブロックの中で積まれた文を横取りするため、いったん退避する。
    let outer = std::mem::take(&mut fctx.stmts);

    let mut stmts: Vec<_> = block
        .stmts
        .iter()
        .map(|s| s.as_oxc_local(ctx, fctx))
        .collect();
    let value = block.expr.as_oxc_local(ctx, fctx);

    // 末尾の式を組む過程で積まれた文 (入れ子の match など) を先に置く。
    let inner = std::mem::replace(&mut fctx.stmts, outer);
    stmts.extend(inner);
    stmts.push(assign_stmt(ctx, dest, value));

    stmts
}

/// アーム 1 つ分の文の並び。先頭に束縛を置く。
fn arm_stmts<'a, F>(
    ctx: &'a AstBuildCtx<'a>,
    fctx: &mut FnAstBuildCtx<'a>,
    subject: &str,
    pattern: &'a Pattern,
    body: F,
) -> Vec<oxc_ast::ast::Statement<'a>>
where
    F: FnOnce(&'a AstBuildCtx<'a>, &mut FnAstBuildCtx<'a>) -> Vec<oxc_ast::ast::Statement<'a>>,
{
    let mut stmts = Vec::new();

    match pattern {
        Pattern::Wildcard(_) => {}
        Pattern::Binding(var_id, _) => {
            stmts.push(bind_stmt(ctx, fctx, *var_id, ident_expr(ctx, subject)));
        }
        Pattern::Variant(vp) => {
            let resolved = vp
                .resolved
                .get()
                .expect("compiler bug: variant pattern was not resolved by inference");

            for (name, var_id) in pattern_bindings(&vp.fields, resolved) {
                let subject_any = as_expr(ctx, ident_expr(ctx, subject), any_keyword(ctx));
                stmts.push(bind_stmt(
                    ctx,
                    fctx,
                    var_id,
                    member_expr(ctx, subject_any, ctx.str_of(&name)),
                ));
            }
        }
    }

    stmts.extend(body(ctx, fctx));
    stmts
}

/// (フィールド名, 束縛先) を宣言順に並べる。
fn pattern_bindings(
    fields: &PatternFields,
    resolved: &ResolvedVariant,
) -> Vec<(biwac_base::InternedIdent, VarId)> {
    match fields {
        PatternFields::Unit => Vec::new(),
        PatternFields::Positional(binds) => resolved
            .field_names
            .iter()
            .zip(binds)
            .filter_map(|(name, b)| Some((*name, b.var_id()?)))
            .collect(),
        PatternFields::Named(named) => named
            .iter()
            .filter_map(|(ident, b)| Some((ident.id, b.var_id()?)))
            .collect(),
    }
}

/// アームを if / else if の連鎖にする。
///
/// 必ず当たるアームは `else` に置く。
/// 網羅性は型推論が検査済みなので、当たるアームが必ずある。
fn build_arm_chain<'a>(
    ctx: &'a AstBuildCtx<'a>,
    subject: &str,
    arms: Vec<(&'a Pattern, Vec<oxc_ast::ast::Statement<'a>>)>,
) -> oxc_ast::ast::Statement<'a> {
    let catch_all = arms.iter().position(|(p, _)| p.is_irrefutable());
    let otherwise_arm = catch_all.unwrap_or(arms.len() - 1);

    let mut tested: Vec<(u32, Vec<oxc_ast::ast::Statement<'a>>)> = Vec::new();
    let mut fallback: Option<Vec<oxc_ast::ast::Statement<'a>>> = None;

    for (i, (pattern, stmts)) in arms.into_iter().enumerate() {
        if i == otherwise_arm {
            fallback = Some(stmts);
            continue;
        }
        let Pattern::Variant(vp) = pattern else {
            continue;
        };
        let index = vp
            .resolved
            .get()
            .expect("compiler bug: variant pattern was not resolved by inference")
            .index;
        tested.push((index, stmts));
    }

    // 後ろから畳んで if / else if にする。
    let mut current = block_stmt(ctx, fallback.unwrap_or_default());
    for (index, stmts) in tested.into_iter().rev() {
        current = oxc_ast::ast::Statement::IfStatement(oxc_allocator::Box::new_in(
            oxc_ast::ast::IfStatement {
                span: span(),
                test: tag_test(ctx, subject, index),
                consequent: block_stmt(ctx, stmts),
                alternate: Some(current),
            },
            ctx.allocator,
        ));
    }

    current
}

/// `(subject.__tag as number) === index`
///
/// `as number` で広げるのは、TypeScript の絞り込みを避けるためである。
/// `let c: Color = Color::Rgb(..)` を出すと、TypeScript は初期化式から
/// `c` を `{ __tag: 1, .. }` に絞り込む。そのまま `c.__tag === 0` と書くと
/// 「重なりがない」と怒られてしまう。
///
/// 型が正しいことは biwa 側で検査済みなので、
/// ここで TypeScript の絞り込みに頼る必要はない。
fn tag_test<'a>(
    ctx: &'a AstBuildCtx<'a>,
    subject: &str,
    index: u32,
) -> oxc_ast::ast::Expression<'a> {
    let tag = member_expr(ctx, ident_expr(ctx, subject), TAG);
    let widened = as_expr(ctx, tag, number_keyword(ctx));

    oxc_ast::ast::Expression::BinaryExpression(oxc_allocator::Box::new_in(
        oxc_ast::ast::BinaryExpression {
            span: span(),
            left: widened,
            operator: oxc_ast::ast::BinaryOperator::StrictEquality,
            right: number_expr(ctx, index as f64),
        },
        ctx.allocator,
    ))
}

/// `expr as ty`
fn as_expr<'a>(
    ctx: &'a AstBuildCtx<'a>,
    expression: oxc_ast::ast::Expression<'a>,
    ty: oxc_ast::ast::TSType<'a>,
) -> oxc_ast::ast::Expression<'a> {
    oxc_ast::ast::Expression::TSAsExpression(oxc_allocator::Box::new_in(
        oxc_ast::ast::TSAsExpression {
            span: span(),
            expression,
            type_annotation: ty,
        },
        ctx.allocator,
    ))
}

fn number_keyword<'a>(ctx: &'a AstBuildCtx<'a>) -> oxc_ast::ast::TSType<'a> {
    oxc_ast::ast::TSType::TSNumberKeyword(oxc_allocator::Box::new_in(
        oxc_ast::ast::TSNumberKeyword { span: span() },
        ctx.allocator,
    ))
}

fn any_keyword<'a>(ctx: &'a AstBuildCtx<'a>) -> oxc_ast::ast::TSType<'a> {
    oxc_ast::ast::TSType::TSAnyKeyword(oxc_allocator::Box::new_in(
        oxc_ast::ast::TSAnyKeyword { span: span() },
        ctx.allocator,
    ))
}

// ---------------------------------------------------------------
// 小さな組み立て
// ---------------------------------------------------------------

pub(super) fn ident_expr<'a>(ctx: &'a AstBuildCtx<'a>, name: &str) -> oxc_ast::ast::Expression<'a> {
    oxc_ast::ast::Expression::Identifier(oxc_allocator::Box::new_in(
        oxc_ast::ast::IdentifierReference {
            span: span(),
            name: oxc_span::Ident::new_const(ctx.allocator.alloc_str(name)),
            reference_id: Cell::new(None),
        },
        ctx.allocator,
    ))
}

fn member_expr<'a>(
    ctx: &'a AstBuildCtx<'a>,
    object: oxc_ast::ast::Expression<'a>,
    property: &str,
) -> oxc_ast::ast::Expression<'a> {
    oxc_ast::ast::Expression::StaticMemberExpression(oxc_allocator::Box::new_in(
        oxc_ast::ast::StaticMemberExpression {
            span: span(),
            object,
            property: oxc_ast::ast::IdentifierName {
                span: span(),
                name: oxc_span::Ident::new_const(ctx.allocator.alloc_str(property)),
            },
            optional: false,
        },
        ctx.allocator,
    ))
}

fn number_expr<'a>(ctx: &'a AstBuildCtx<'a>, value: f64) -> oxc_ast::ast::Expression<'a> {
    oxc_ast::ast::Expression::NumericLiteral(oxc_allocator::Box::new_in(
        oxc_ast::ast::NumericLiteral {
            span: span(),
            value,
            raw: None,
            base: oxc_ast::ast::NumberBase::Decimal,
        },
        ctx.allocator,
    ))
}

pub(super) fn let_stmt<'a>(
    ctx: &'a AstBuildCtx<'a>,
    name: &str,
    init: Option<oxc_ast::ast::Expression<'a>>,
    type_annotation: Option<oxc_ast::ast::TSType<'a>>,
) -> oxc_ast::ast::Statement<'a> {
    oxc_ast::ast::Statement::VariableDeclaration(oxc_allocator::Box::new_in(
        oxc_ast::ast::VariableDeclaration {
            span: span(),
            kind: oxc_ast::ast::VariableDeclarationKind::Let,
            declarations: oxc_allocator::Vec::from_iter_in(
                [oxc_ast::ast::VariableDeclarator {
                    span: span(),
                    kind: oxc_ast::ast::VariableDeclarationKind::Let,
                    id: oxc_ast::ast::BindingPattern::BindingIdentifier(
                        oxc_allocator::Box::new_in(
                            oxc_ast::ast::BindingIdentifier {
                                span: span(),
                                name: oxc_span::Ident::new_const(ctx.allocator.alloc_str(name)),
                                symbol_id: Cell::new(None),
                            },
                            ctx.allocator,
                        ),
                    ),
                    type_annotation: type_annotation.map(|ty| {
                        oxc_allocator::Box::new_in(
                            oxc_ast::ast::TSTypeAnnotation {
                                span: span(),
                                type_annotation: ty,
                            },
                            ctx.allocator,
                        )
                    }),
                    init,
                    definite: false,
                }],
                ctx.allocator,
            ),
            declare: false,
        },
        ctx.allocator,
    ))
}

/// パターンが束縛する変数の宣言。
///
/// 型は biwa の推論結果をそのまま注釈にする。
/// 読み出し側は `as any` を通すので、TypeScript の絞り込みには頼らない。
/// `any` が漏れるのはこの 1 回の読み出しだけで、
/// 束縛された変数自体は正しい型を持つ。
fn bind_stmt<'a>(
    ctx: &'a AstBuildCtx<'a>,
    fctx: &FnAstBuildCtx<'a>,
    var_id: VarId,
    init: oxc_ast::ast::Expression<'a>,
) -> oxc_ast::ast::Statement<'a> {
    let ty = fctx.var_tys.get(&var_id).map(|ty| ty.kind.as_oxc(ctx));
    let_stmt(ctx, &var_id.mangled(ctx), Some(init), ty)
}

fn assign_stmt<'a>(
    ctx: &'a AstBuildCtx<'a>,
    name: &str,
    value: oxc_ast::ast::Expression<'a>,
) -> oxc_ast::ast::Statement<'a> {
    oxc_ast::ast::Statement::ExpressionStatement(oxc_allocator::Box::new_in(
        oxc_ast::ast::ExpressionStatement {
            span: span(),
            expression: oxc_ast::ast::Expression::AssignmentExpression(oxc_allocator::Box::new_in(
                oxc_ast::ast::AssignmentExpression {
                    span: span(),
                    operator: oxc_ast::ast::AssignmentOperator::Assign,
                    left: oxc_ast::ast::AssignmentTarget::AssignmentTargetIdentifier(
                        oxc_allocator::Box::new_in(
                            oxc_ast::ast::IdentifierReference {
                                span: span(),
                                name: oxc_span::Ident::new_const(ctx.allocator.alloc_str(name)),
                                reference_id: Cell::new(None),
                            },
                            ctx.allocator,
                        ),
                    ),
                    right: value,
                },
                ctx.allocator,
            )),
        },
        ctx.allocator,
    ))
}

pub(super) fn block_stmt<'a>(
    ctx: &'a AstBuildCtx<'a>,
    stmts: Vec<oxc_ast::ast::Statement<'a>>,
) -> oxc_ast::ast::Statement<'a> {
    oxc_ast::ast::Statement::BlockStatement(oxc_allocator::Box::new_in(
        oxc_ast::ast::BlockStatement {
            span: span(),
            body: oxc_allocator::Vec::from_iter_in(stmts, ctx.allocator),
            scope_id: Cell::new(None),
        },
        ctx.allocator,
    ))
}
