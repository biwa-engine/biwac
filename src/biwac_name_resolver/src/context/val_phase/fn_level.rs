use std::collections::{HashMap, hash_map::Entry};

use biwac_hir::{DecledVar, ExprId, Hir, LocGenTyId, LocVarId, Ty, TyKind, VarIdKind};
use biwac_parser::{DefTyp, Ident, PrimTyp, QualifiedId, TypRepr, TypReprVal};

use crate::{
    ResolveError, RsvResult,
    context::val_phase::{ResolvedValue, impl_level::ImplLevelResolveCtx},
};

#[derive(Debug)]
pub(crate) struct FnLevelResolveCtx<'ictx> {
    ictx: &'ictx ImplLevelResolveCtx<'ictx>,
    fn_def_genargs: HashMap<String, LocGenTyId>,
    // scope based variable name pool
    // ローカル変数に関数ローカルに一意なid
    // LocVarIdをつける
    // Stmt::VarDecl はLocVarIdも持つようにし、
    // 他の変数の参照をするExprはすべてLocVarIdだけもたせる
    // これにより、以降の型検査などでスコープのネストによる
    // 名前空間を考えなくて良くなる。フラットに考えられる
    scopes: Vec<HashMap<String, (DecledVar, LocVarId)>>,
    next_var_id: usize,
    next_expr_id: usize,
}

impl<'ictx> FnLevelResolveCtx<'ictx> {
    pub(crate) fn new(
        ictx: &'ictx ImplLevelResolveCtx<'ictx>,
        fn_def_genarg_vec: &[(Ident, LocGenTyId)],
    ) -> RsvResult<Self> {
        Ok(Self {
            ictx,
            fn_def_genargs: fn_def_genarg_vec
                .iter()
                .map(|(ident, lgid)| (ident.id.clone(), *lgid))
                .collect(),
            scopes: vec![HashMap::new()],
            next_var_id: 0,
            next_expr_id: 0,
        })
    }

    pub(crate) fn try_resolve_ty(&self, typ: &TypRepr, hir: &Hir) -> RsvResult<Ty> {
        match &typ.val {
            TypReprVal::Primitive(p) => match p {
                PrimTyp::Int => Ok(Ty::new(TyKind::Int, typ.span.clone())),
                // TODO: Uint
                PrimTyp::Uint => Ok(Ty::new(TyKind::Int, typ.span.clone())),
                PrimTyp::Float => Ok(Ty::new(TyKind::Float, typ.span.clone())),
                PrimTyp::Bool => Ok(Ty::new(TyKind::Bool, typ.span.clone())),
            },
            TypReprVal::Defined(deftyp) => {
                // deftypがidのみ(ex: `T`)の場合、
                // 内側から名前解決する
                //
                //  ```
                //  import foo::bar::T;
                //                   ^
                //                   | (3)さらに次に解決が試みられる
                //
                //  type T = Bar[Int];
                //       ^
                //       | (2)次に解決が試みられる
                //
                //  struct Foo[T, U] {
                //            ^^^^^^
                //            | (1)まず解決が試みられる
                //      x: T,
                //      y: U,
                //      z: Int,
                //  }
                //  ```
                if let Some(id) = deftyp.qualid.only_id()
                    && let Some(gid) = self.fn_def_genargs.get(id)
                {
                    Ok(Ty::new(TyKind::LocGen(*gid), typ.span.clone()))
                } else {
                    self.ictx.try_resolve_defined_ty(deftyp, hir)
                }
            }
        }
    }

    #[inline]
    pub(crate) fn try_resolve_defined_ty(&self, deftyp: &DefTyp, hir: &Hir) -> RsvResult<Ty> {
        self.ictx.try_resolve_defined_ty(deftyp, hir)
    }

    pub(crate) fn new_expr_id(&mut self) -> ExprId {
        let id = ExprId::new(self.next_expr_id);

        self.next_expr_id += 1;

        id
    }

    pub(crate) fn enter_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub(crate) fn exit_scope(&mut self) {
        // popping when empty is compiler bug
        self.scopes.pop().expect("compiler bug: scope underflowed");
    }

    pub(crate) fn declare_variable(&mut self, var: &Ident, ty: Ty) -> RsvResult<LocVarId> {
        match self.scopes.last_mut().unwrap().entry(var.id.clone()) {
            Entry::Vacant(e) => {
                let var_id = LocVarId::new(self.next_var_id);
                e.insert((
                    DecledVar {
                        id: var.clone(),
                        ty,
                    },
                    var_id,
                ));
                self.next_var_id += 1;

                Ok(var_id)
            }
            Entry::Occupied(e) => Err(ResolveError::DuplicatedVarName {
                vid1: Box::new(var.clone()),
                vid2: Box::new(e.get().0.id.clone()),
            }),
        }
    }

    // 変数名を解決する
    pub(crate) fn try_resolve_variable(&self, ident: &Ident, hir: &Hir) -> RsvResult<VarIdKind> {
        // 先に関数ローカルで、内側のスコープから、解決を試みる
        for scope in self.scopes.iter().rev() {
            if let Some((_, var_id)) = scope.get(&ident.id) {
                return Ok(VarIdKind::Local(*var_id));
            }
        }

        let val = self.ictx.try_resolve_value(
            &QualifiedId {
                is_from_root: false,
                quals: vec![],
                id: ident.id.clone(),
                span: ident.span.clone(),
            },
            hir,
        )?;

        match val {
            ResolvedValue::Local(var_id) => Ok(VarIdKind::Local(var_id)),
            ResolvedValue::Global(vid) => Ok(VarIdKind::Global(vid)),
            ResolvedValue::Assoc(_) => {
                // error
                todo!()
            }
        }
    }

    #[inline]
    pub(crate) fn try_resolve_value(
        &self,
        qualid: &QualifiedId,
        hir: &Hir,
    ) -> RsvResult<ResolvedValue> {
        self.ictx.try_resolve_value(qualid, hir)
    }

    #[inline]
    pub(crate) fn vars(&self) -> HashMap<LocVarId, DecledVar> {
        self.scopes
            .iter()
            .flat_map(|scope| scope.iter())
            .map(|(_, (ident, var_id))| (*var_id, ident.clone()))
            .collect()
    }
}
