pub(crate) mod ty_phase;
pub(crate) mod val_phase;

use biwac_ast::PrimTyp;
use biwac_base::Span;
use biwac_hir::{Ty, TyKind};

fn ty_from_primitive(ptyp: &PrimTyp, span: Span) -> Ty {
    match ptyp {
        PrimTyp::Int => Ty::new(TyKind::Int, span.into()),
        // TODO: Uint
        PrimTyp::Uint => Ty::new(TyKind::Int, span.into()),
        PrimTyp::Float => Ty::new(TyKind::Float, span.into()),
        PrimTyp::Bool => Ty::new(TyKind::Bool, span.into()),
    }
}
