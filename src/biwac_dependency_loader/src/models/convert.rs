use biwac_hir::{FnSignature, StructDef, Ty};
use biwac_span::Span;

use crate::{DepsFunction, DepsStruct, DepsTy};

impl DepsFunction {
    pub fn as_fn_signature(&self, span: Span) -> FnSignature {
        todo!()
        // FnDefSignature {
        //     args: self
        //         .args
        //         .iter()
        //         .map(|a| {
        //             (
        //                 Ident {
        //                     id: a.id.clone(),
        //                     span: span.clone(),
        //                 },
        //                 a.ty.as_ty(span.clone()),
        //             )
        //         })
        //         .collect(),
        //     rty: self.ret.as_ty(span.clone()),
        //     genargs: self
        //         .genargs
        //         .iter()
        //         .enumerate()
        //         .map(|(i, g)| {
        //             (
        //                 Ident {
        //                     id: g.clone(),
        //                     span: span.clone(),
        //                 },
        //                 LocGenTyId::new(i),
        //             )
        //         })
        //         .collect(),
        //     span,
        // }
    }
}

impl DepsStruct {
    pub fn as_struct_def(&self, span: Span) -> StructDef {
        todo!()
        // StructDef {
        //     // TODO: 重複チェック
        //     members: self
        //         .members
        //         .iter()
        //         .map(|m| (m.id.clone(), m.ty.as_ty(span.clone())))
        //         .collect(),
        //
        //     genargs: self
        //         .genargs
        //         .iter()
        //         .enumerate()
        //         .map(|(i, _)| GenTyId::new(i))
        //         .collect(),
        //     struct_name_span: span,
        // }
    }
}

impl DepsTy {
    pub fn as_ty(&self, span: Span) -> Ty {
        todo!()
        // let kind = match self {
        //     Self::Int => TyKind::Int,
        //     Self::Float => TyKind::Float,
        //     Self::Bool => TyKind::Bool,
        //     Self::Void => TyKind::Void,
        //     Self::Defined { id, genargs } => TyKind::Defined(DefinedTy {
        //         tid: TyId::new(
        //             PkgId::new(id.pkg.clone()),
        //             id.modu.clone().into(),
        //             id.id.clone(),
        //         ),
        //         genargs: genargs.iter().map(|g| g.as_ty(span.clone())).collect(),
        //     }),
        // };
        //
        // Ty::new(kind, span)
    }
}
