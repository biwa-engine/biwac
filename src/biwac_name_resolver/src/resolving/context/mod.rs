pub(crate) mod fn_level;
pub(crate) mod impl_level;
pub(crate) mod module_level;
pub(crate) mod trait_def_level;
pub(crate) mod ty_def_level;

use biwac_ast::{Path, PathSegment, PrimTyp, TypRepr, TypReprVal, symbols::globals::GenArgsDecl};
use biwac_hir::TyKind;
use biwac_span::{Span, TraitAssocDefId, TraitDefId, VarId};

use crate::{ResolveError, ResolveErrorHandler};

pub(crate) trait ResolveCtx {
    fn resolve_path(&self, path: &Path) -> Result<(), ResolveError>;

    fn resolve_typ(&self, typ: &TypRepr) -> Result<(), Vec<ResolveError>> {
        match &typ.val {
            TypReprVal::Primitive(PrimTyp::Int) => Ok(()),
            TypReprVal::Primitive(PrimTyp::Uint) => Ok(()), // TODO:
            TypReprVal::Primitive(PrimTyp::Float) => Ok(()),
            TypReprVal::Primitive(PrimTyp::Bool) => Ok(()),
            TypReprVal::Defined(def_typ) => {
                let mut errors = Vec::new();

                match self.resolve_path(&def_typ.path) {
                    Ok(()) => {
                        // trait はパスとしては解決できるが型ではない。
                        // ここで弾かないと lowering が
                        // 「解決できなかった型」として `Infer` に潰してしまい、
                        // 診断がずっと後ろの段まで流れてしまう。
                        if let Ok(biwac_span::DefIdKind::Trait(def_id)) =
                            crate::lowering::def_id_kind_from_path(&def_typ.path)
                        {
                            errors.push(ResolveError::TypeNotFoundTraitFound {
                                path: Box::new(def_typ.path.clone()),
                                def_id,
                            });
                        }
                    }
                    Err(e) => errors.push(e),
                }

                if let Some(genargs) = &def_typ.genargs {
                    for typ in genargs {
                        self.resolve_typ(typ).handle(&mut errors);
                    }
                }

                if errors.is_empty() {
                    Ok(())
                } else {
                    Err(errors)
                }
            }
            TypReprVal::SelfTyp => match self.opt_self_ty() {
                Some(_) => Ok(()),
                None => Err(vec![ResolveError::UnexpectedSelfType {
                    span: typ.span.clone(),
                }]),
            },
        }
    }

    /// `impl Foo: Bar[Int]` の `Bar[Int]` を解決する。
    ///
    /// [`Self::resolve_typ`] と違い、trait であることを期待する位置なので
    /// 「trait は型ではない」の検査を行わない。
    /// ジェネリック引数の側は普通の型なので、そちらは `resolve_typ` に回す。
    fn resolve_trait_typ(&self, typ: &TypRepr) -> Result<(), Vec<ResolveError>> {
        let TypReprVal::Defined(def_typ) = &typ.val else {
            return Err(vec![ResolveError::TraitExpected {
                path: Box::new(Path::new(None, vec![])),
            }]);
        };

        let mut errors = Vec::new();
        self.resolve_path(&def_typ.path).handle(&mut errors);

        if let Some(genargs) = &def_typ.genargs {
            for typ in genargs {
                self.resolve_typ(typ).handle(&mut errors);
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// ジェネリック引数の宣言に付いた制限を解決する。
    ///
    /// 制限は trait でなければならないので `resolve_trait_typ` に回す
    /// (`resolve_typ` は「trait は型ではない」で弾いてしまう)。
    fn resolve_genarg_bounds<I>(
        &self,
        decl: &Option<GenArgsDecl<I>>,
    ) -> Result<(), Vec<ResolveError>> {
        let Some(decl) = decl else {
            return Ok(());
        };

        let mut errors = Vec::new();
        for item in &decl.genargs {
            for bound in &item.bounds {
                self.resolve_trait_typ(bound).handle(&mut errors);
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// ジェネリック引数に付いた制限から関連アイテムを引く (`T::guee`)。
    ///
    /// 実体を持つのは [`module_level::ModuleResolveCtx`] だけで、
    /// 内側の文脈はそこへ委譲する。
    fn lookup_trait_assoc(
        &self,
        _bounds: &[TraitDefId],
        segment: &PathSegment,
    ) -> Result<TraitAssocDefId, ResolveError> {
        Err(ResolveError::TraitAssocNotFound {
            segment: segment.clone(),
        })
    }

    fn opt_self_ty(&self) -> Option<TyKind> {
        None
    }
}

/// ジェネリック引数の宣言に書かれた制限のうち、trait に解決できたものの id。
///
/// 解決に失敗したものは既にエラーになっているので、ここでは落とす。
pub(crate) fn resolved_bound_traits<I>(
    item: &biwac_ast::symbols::globals::GenArgDeclItem<I>,
) -> Vec<TraitDefId> {
    item.bounds
        .iter()
        .filter_map(|b| {
            let TypReprVal::Defined(def_typ) = &b.val else {
                return None;
            };
            match crate::lowering::def_id_kind_from_path(&def_typ.path) {
                Ok(biwac_span::DefIdKind::Trait(def_id)) => Some(def_id),
                _ => None,
            }
        })
        .collect()
}

pub(crate) trait LocalResolveCtx: ResolveCtx {
    fn declare_variable(&mut self, ident: &biwac_ast::Ident) -> Result<VarId, ResolveError>;
    fn inner_scope<F: FnOnce(&mut Self) -> Result<(), Vec<ResolveError>>>(
        &mut self,
        f: F,
    ) -> Result<(), Vec<ResolveError>>;
    fn resolve_self_var(&self, span: &Span) -> Result<VarId, ResolveError>;
}
