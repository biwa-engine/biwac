use std::cell::Cell;

use biwac_hir::{GenTyId, Hir, LocGenTyId, TyKind};

use crate::arch::typescript::{AsOxc, IntoOxc, Mangled, span};

impl Mangled for GenTyId {
    fn mangled(&self) -> String {
        format!("T{}", self.value())
    }
}

impl Mangled for LocGenTyId {
    fn mangled(&self) -> String {
        format!("T{}", self.value())
    }
}

impl<'a> AsOxc<'a, oxc_ast::ast::TSType<'a>> for TyKind {
    fn as_oxc(
        &'a self,
        _env: &mut super::FnAstBuildEnv<'a>,
        allocator: &'a oxc_allocator::Allocator,
        hir: &Hir,
    ) -> oxc_ast::ast::TSType<'a> {
        match self {
            Self::Infer(_) => panic!("compiler bug, type inferrence failed for type variable"),
            Self::Int => oxc_ast::ast::TSType::TSNumberKeyword(oxc_allocator::Box::new_in(
                oxc_ast::ast::TSNumberKeyword { span: span() },
                allocator,
            )),
            Self::Float => oxc_ast::ast::TSType::TSNumberKeyword(oxc_allocator::Box::new_in(
                oxc_ast::ast::TSNumberKeyword { span: span() },
                allocator,
            )),
            Self::Bool => oxc_ast::ast::TSType::TSBooleanKeyword(oxc_allocator::Box::new_in(
                oxc_ast::ast::TSBooleanKeyword { span: span() },
                allocator,
            )),
            Self::Void => oxc_ast::ast::TSType::TSVoidKeyword(oxc_allocator::Box::new_in(
                oxc_ast::ast::TSVoidKeyword { span: span() },
                allocator,
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
                                        allocator
                                            .alloc_str(&(&hir.pkg_name, &defined_ty.tid).mangled()),
                                    ),
                                    reference_id: Cell::new(None),
                                },
                                allocator,
                            ),
                        ),
                        type_arguments: if !defined_ty.genargs.is_empty() {
                            Some(oxc_allocator::Box::new_in(
                                oxc_ast::ast::TSTypeParameterInstantiation {
                                    span: span(),
                                    params: oxc_allocator::Vec::from_iter_in(
                                        defined_ty
                                            .genargs
                                            .iter()
                                            .map(|ty| ty.kind.clone().into_oxc(allocator, hir)),
                                        allocator,
                                    ),
                                },
                                allocator,
                            ))
                        } else {
                            None
                        },
                    },
                    allocator,
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
                                    allocator.alloc_str(&gid.mangled()),
                                ),
                                reference_id: Cell::new(None),
                            },
                            allocator,
                        ),
                    ),
                    type_arguments: None,
                },
                allocator,
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
                                        allocator.alloc_str(&lgid.mangled()),
                                    ),
                                    reference_id: Cell::new(None),
                                },
                                allocator,
                            ),
                        ),
                        type_arguments: None,
                    },
                    allocator,
                ))
            }
        }
    }
}

impl<'a> IntoOxc<'a, oxc_ast::ast::TSType<'a>> for TyKind {
    fn into_oxc(
        self,
        allocator: &'a oxc_allocator::Allocator,
        hir: &Hir,
    ) -> oxc_ast::ast::TSType<'a> {
        match self {
            Self::Infer(_) => panic!("compiler bug, type inferrence failed for type variable"),
            Self::Int => oxc_ast::ast::TSType::TSNumberKeyword(oxc_allocator::Box::new_in(
                oxc_ast::ast::TSNumberKeyword { span: span() },
                allocator,
            )),
            Self::Float => oxc_ast::ast::TSType::TSNumberKeyword(oxc_allocator::Box::new_in(
                oxc_ast::ast::TSNumberKeyword { span: span() },
                allocator,
            )),
            Self::Bool => oxc_ast::ast::TSType::TSBooleanKeyword(oxc_allocator::Box::new_in(
                oxc_ast::ast::TSBooleanKeyword { span: span() },
                allocator,
            )),
            Self::Void => oxc_ast::ast::TSType::TSVoidKeyword(oxc_allocator::Box::new_in(
                oxc_ast::ast::TSVoidKeyword { span: span() },
                allocator,
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
                                        allocator
                                            .alloc_str(&(&hir.pkg_name, &defined_ty.tid).mangled()),
                                    ),
                                    reference_id: Cell::new(None),
                                },
                                allocator,
                            ),
                        ),
                        type_arguments: if !defined_ty.genargs.is_empty() {
                            Some(oxc_allocator::Box::new_in(
                                oxc_ast::ast::TSTypeParameterInstantiation {
                                    span: span(),
                                    params: oxc_allocator::Vec::from_iter_in(
                                        defined_ty
                                            .genargs
                                            .into_iter()
                                            .map(|ty| ty.kind.into_oxc(allocator, hir)),
                                        allocator,
                                    ),
                                },
                                allocator,
                            ))
                        } else {
                            None
                        },
                    },
                    allocator,
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
                                    allocator.alloc_str(&gid.mangled()),
                                ),
                                reference_id: Cell::new(None),
                            },
                            allocator,
                        ),
                    ),
                    type_arguments: None,
                },
                allocator,
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
                                        allocator.alloc_str(&lgid.mangled()),
                                    ),
                                    reference_id: Cell::new(None),
                                },
                                allocator,
                            ),
                        ),
                        type_arguments: None,
                    },
                    allocator,
                ))
            }
        }
    }
}
