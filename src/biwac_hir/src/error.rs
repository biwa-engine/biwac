use biwac_base::SSpan;

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
        defined_position1: Box<SSpan>,
        defined_position2: Box<SSpan>,
    },
    DuplicatedTypeName {
        tid: Box<TyId>,
        defined_position1: Box<SSpan>,
        defined_position2: Box<SSpan>,
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
        caller_span: Box<SSpan>,
        val_content: Box<ImplValDefContentKind>, // 取得された実装
    },
}
