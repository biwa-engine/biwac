use std::cell::Cell;

use biwac_type_inferrer::Ty;

use crate::arch::typescript::{AsOxc, IntoOxc, Mangled, span};

impl<'a> AsOxc<'a, oxc_ast::ast::TSType<'a>> for Ty {
    fn as_oxc(
        &'a self,
        _env: &mut super::FnAstBuildEnv<'a>,
        allocator: &'a oxc_allocator::Allocator,
    ) -> oxc_ast::ast::TSType<'a> {
        match self {
            Self::Var(_) => panic!("compiler bug, type inferrence failed for type variable"),
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
            Self::Struct(s) => oxc_ast::ast::TSType::TSTypeReference(oxc_allocator::Box::new_in(
                oxc_ast::ast::TSTypeReference {
                    span: span(),
                    type_name: oxc_ast::ast::TSTypeName::IdentifierReference(
                        oxc_allocator::Box::new_in(
                            oxc_ast::ast::IdentifierReference {
                                span: span(),
                                name: oxc_span::Ident::new_const(allocator.alloc_str(&s.mangled())),
                                reference_id: Cell::new(None),
                            },
                            allocator,
                        ),
                    ),
                    type_arguments: None,
                },
                allocator,
            )),
            Self::Fn(_) => todo!(),
        }
    }
}

impl<'a> IntoOxc<'a, oxc_ast::ast::TSType<'a>> for Ty {
    fn into_oxc(self, allocator: &'a oxc_allocator::Allocator) -> oxc_ast::ast::TSType<'a> {
        match self {
            Self::Var(_) => panic!("compiler bug, type inferrence failed for type variable"),
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
            Self::Struct(s) => oxc_ast::ast::TSType::TSTypeReference(oxc_allocator::Box::new_in(
                oxc_ast::ast::TSTypeReference {
                    span: span(),
                    type_name: oxc_ast::ast::TSTypeName::IdentifierReference(
                        oxc_allocator::Box::new_in(
                            oxc_ast::ast::IdentifierReference {
                                span: span(),
                                name: oxc_span::Ident::new_const(allocator.alloc_str(&s.mangled())),
                                reference_id: Cell::new(None),
                            },
                            allocator,
                        ),
                    ),
                    type_arguments: None,
                },
                allocator,
            )),
            Self::Fn(_) => todo!(),
        }
    }
}
