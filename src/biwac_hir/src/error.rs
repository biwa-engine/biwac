use biwac_span::Span;

pub use crate::hir::{
    TyExistence,
    symbols::{
        Ident,
        expressions::AssocCallee,
        globals::{ImplValDefContentKind, TyId, ValId},
    },
    types::{DefinedTy, TyKind},
};

#[derive(Debug, Clone)]
pub enum HirError {
    DuplicatedValueName {
        vid: Box<ValId>,
        defined_position1: Box<Span>,
        defined_position2: Box<Span>,
    },
    DuplicatedTypeName {
        tid: Box<TyId>,
        defined_position1: Box<Span>,
        defined_position2: Box<Span>,
    },
    GenericArgLengthMismatched {
        defined_ty: Box<DefinedTy>,
        ty_existence: Box<TyExistence>,
    },
    DuplicatedImplementationForType {
        defined_ty: Box<DefinedTy>,
        ty_existence: Box<TyExistence>,
        val_content1: Box<ImplValDefContentKind>,
        val_content2: Box<ImplValDefContentKind>,
    },
    DuplicatedImplementationForSpecialType {
        ty: Box<TyKind>,
        val_content1: Box<ImplValDefContentKind>,
        val_content2: Box<ImplValDefContentKind>,
    },
    ImplementedValueIsNotMethod {
        ty: Box<TyKind>,
        method: Box<Ident>,                      // caller のspanを含む
        val_content: Box<ImplValDefContentKind>, // 取得された実装
    },
    ImplementedValueIsNotAssoc {
        assoc_callee: Box<AssocCallee>,
        caller_span: Box<Span>,
        val_content: Box<ImplValDefContentKind>, // 取得された実装
    },
}
