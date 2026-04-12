use biwac_base::SSpan;
use biwac_hir::{
    DefinedTy, FnDefContentSignature, GenTyId, Ident, LocGenTyId, PkgId, StructDefContent, Ty,
    TyId, TyKind,
};

use crate::{DepsFunction, DepsStruct, DepsTy};

impl DepsFunction {
    pub fn as_fn_signature(&self, span: SSpan) -> FnDefContentSignature {
        FnDefContentSignature {
            args: self
                .args
                .iter()
                .map(|a| {
                    (
                        Ident {
                            id: a.id.clone(),
                            span: span.clone(),
                        },
                        a.ty.as_ty(span.clone()),
                    )
                })
                .collect(),
            rty: self.ret.as_ty(span.clone()),
            genargs: self
                .genargs
                .iter()
                .enumerate()
                .map(|(i, g)| {
                    (
                        Ident {
                            id: g.clone(),
                            span: span.clone(),
                        },
                        LocGenTyId::new(i),
                    )
                })
                .collect(),
            span,
        }
    }
}

impl DepsStruct {
    pub fn as_struct_def(&self, span: SSpan) -> StructDefContent {
        StructDefContent {
            // TODO: 重複チェック
            members: self
                .members
                .iter()
                .map(|m| (m.id.clone(), m.ty.as_ty(span.clone())))
                .collect(),

            genargs: self
                .genargs
                .iter()
                .enumerate()
                .map(|(i, _)| GenTyId::new(i))
                .collect(),
            struct_name_span: span,
        }
    }
}

impl DepsTy {
    pub fn as_ty(&self, span: SSpan) -> Ty {
        let kind = match self {
            Self::Int => TyKind::Int,
            Self::Float => TyKind::Float,
            Self::Bool => TyKind::Bool,
            Self::Void => TyKind::Void,
            Self::Defined { id, genargs } => TyKind::Defined(DefinedTy {
                tid: TyId::new(
                    PkgId::new(id.pkg.clone()),
                    id.modu.clone().into(),
                    id.id.clone(),
                ),
                genargs: genargs.iter().map(|g| g.as_ty(span.clone())).collect(),
            }),
        };

        Ty::new(kind, span)
    }
}
