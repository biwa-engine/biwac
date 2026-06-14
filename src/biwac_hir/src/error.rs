use biwac_span::{Span, TyDefId, ValDefId};

pub use crate::hir::{TyExistence, types::DefinedTy};

#[derive(Debug, Clone)]
pub enum HirError {
    DuplicatedValueName {
        vid: Box<ValDefId>,
        defined_position1: Box<Span>,
        defined_position2: Box<Span>,
    },
    DuplicatedTypeName {
        tid: Box<TyDefId>,
        defined_position1: Box<Span>,
        defined_position2: Box<Span>,
    },
    GenericArgLengthMismatched {
        defined_ty: Box<DefinedTy>,
        ty_existence: Box<TyExistence>,
    },
}
