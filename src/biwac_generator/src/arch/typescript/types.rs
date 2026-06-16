use std::cell::Cell;

use biwac_hir::TyKind;
use biwac_span::{GenDefId, LocalGenDefId};

use crate::arch::typescript::{AsOxc, Mangled, span};

impl Mangled for GenDefId {
    fn mangled(&self, _ctx: &super::AstBuildCtx) -> String {
        format!("T{}", self.value())
    }
}

impl Mangled for LocalGenDefId {
    fn mangled(&self, _ctx: &super::AstBuildCtx) -> String {
        format!("T{}", self.value())
    }
}

impl<'a> AsOxc<'a, oxc_ast::ast::TSType<'a>> for TyKind {
    fn as_oxc(&'a self, ctx: &'a super::AstBuildCtx<'a>) -> oxc_ast::ast::TSType<'a> {
        match self {
            Self::Infer(_) => panic!("compiler bug, type inferrence failed for type variable"),
            Self::Int => oxc_ast::ast::TSType::TSNumberKeyword(oxc_allocator::Box::new_in(
                oxc_ast::ast::TSNumberKeyword { span: span() },
                &ctx.allocator,
            )),
            Self::Float => oxc_ast::ast::TSType::TSNumberKeyword(oxc_allocator::Box::new_in(
                oxc_ast::ast::TSNumberKeyword { span: span() },
                &ctx.allocator,
            )),
            Self::Bool => oxc_ast::ast::TSType::TSBooleanKeyword(oxc_allocator::Box::new_in(
                oxc_ast::ast::TSBooleanKeyword { span: span() },
                &ctx.allocator,
            )),
            Self::Void => oxc_ast::ast::TSType::TSVoidKeyword(oxc_allocator::Box::new_in(
                oxc_ast::ast::TSVoidKeyword { span: span() },
                &ctx.allocator,
            )),
            Self::Defined(defined_ty) => {
                oxc_ast::ast::TSType::TSTypeReference(oxc_allocator::Box::new_in(
                    oxc_ast::ast::TSTypeReference {
                        span: span(),
                        type_name: oxc_ast::ast::TSTypeName::IdentifierReference(
                            oxc_allocator::Box::new_in(
                                oxc_ast::ast::IdentifierReference {
                                    span: span(),
                                    name: oxc_span::Ident::new_const(
                                        &ctx.allocator
                                            .alloc_str(&ctx.get_type_mangled(&defined_ty.def_id)),
                                    ),
                                    reference_id: Cell::new(None),
                                },
                                &ctx.allocator,
                            ),
                        ),
                        type_arguments: if !defined_ty.genargs.is_empty() {
                            Some(oxc_allocator::Box::new_in(
                                oxc_ast::ast::TSTypeParameterInstantiation {
                                    span: span(),
                                    params: oxc_allocator::Vec::from_iter_in(
                                        defined_ty.genargs.iter().map(|ty| ty.kind.as_oxc(ctx)),
                                        &ctx.allocator,
                                    ),
                                },
                                &ctx.allocator,
                            ))
                        } else {
                            None
                        },
                    },
                    &ctx.allocator,
                ))
            }
            Self::Fn(_) => todo!(),
            Self::Gen(gid) => oxc_ast::ast::TSType::TSTypeReference(oxc_allocator::Box::new_in(
                oxc_ast::ast::TSTypeReference {
                    span: span(),
                    type_name: oxc_ast::ast::TSTypeName::IdentifierReference(
                        oxc_allocator::Box::new_in(
                            oxc_ast::ast::IdentifierReference {
                                span: span(),

                                name: oxc_span::Ident::new_const(
                                    &ctx.allocator.alloc_str(&gid.mangled(ctx)),
                                ),
                                reference_id: Cell::new(None),
                            },
                            &ctx.allocator,
                        ),
                    ),
                    type_arguments: None,
                },
                &ctx.allocator,
            )),
            Self::LocGen(lgid) => {
                oxc_ast::ast::TSType::TSTypeReference(oxc_allocator::Box::new_in(
                    oxc_ast::ast::TSTypeReference {
                        span: span(),
                        type_name: oxc_ast::ast::TSTypeName::IdentifierReference(
                            oxc_allocator::Box::new_in(
                                oxc_ast::ast::IdentifierReference {
                                    span: span(),
                                    name: oxc_span::Ident::new_const(
                                        &ctx.allocator.alloc_str(&lgid.mangled(ctx)),
                                    ),
                                    reference_id: Cell::new(None),
                                },
                                &ctx.allocator,
                            ),
                        ),
                        type_arguments: None,
                    },
                    &ctx.allocator,
                ))
            }
        }
    }
}
