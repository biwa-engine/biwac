//! trait 宣言の中での名前解決。
//!
//! ```biwa
//! trait Conv[T] {
//! //         ^ genargs
//!   fn conv(self) -> T;
//!   fn from(t: T) -> Self;
//! //                 ^^^^ self_gen
//! }
//! ```
//!
//! `Self` は宣言の時点ではまだ何の型でもないので、
//! 暗黙のジェネリック引数として扱う。
//! impl 側の検査で実装対象の型に置き換える
//! (`Ty::embody_by_gen_ty_id` がそのまま使える)。

use std::collections::{HashMap, hash_map::Entry};

use biwac_ast::{AbsolutePathHeader, TraitDef, symbols::globals::GenArgsDecl};
use biwac_base::InternedIdent;
use biwac_hir::TyKind;
use biwac_span::{DefIdKind, GenDefId, Span};

use crate::{
    ResolveError,
    resolving::{context::ResolveCtx, def_collector::DefCollector},
};

#[derive(Debug)]
pub struct TraitDefResolveCtx<'ctx, C: ResolveCtx> {
    ctx: &'ctx C,
    genargs: HashMap<InternedIdent, GenDefId>,
    self_gen: GenDefId,
}

impl<C: ResolveCtx> ResolveCtx for TraitDefResolveCtx<'_, C> {
    fn resolve_path(&self, path: &biwac_ast::Path) -> Result<(), ResolveError> {
        if path.abs_header.is_none()
            && path.segments.len() == 1
            && let Some(def_id) = self.genargs.get(&path.segments[0].ident.id)
        {
            if path.segments[0].resolved_id.get().is_none() {
                path.segments[0]
                    .resolved_id
                    .set(biwac_ast::PathSegmentResolution::Ok(DefIdKind::Gen(
                        *def_id,
                    )))
                    .unwrap();
            }
            Ok(())
        } else if let Some(AbsolutePathHeader::SelfTyp(self_typ)) = &path.abs_header {
            // `Self::foo()` は trait の宣言の中では書けない。
            // `Self` がまだ型ではないので、関連アイテムを引く先が無い。
            Err(ResolveError::UnexpectedSelfType {
                span: self_typ.span.clone(),
            })
        } else {
            self.ctx.resolve_path(path)
        }
    }

    fn opt_self_ty(&self) -> Option<TyKind> {
        Some(TyKind::Gen(self.self_gen))
    }
}

impl<'ctx, C: ResolveCtx> TraitDefResolveCtx<'ctx, C> {
    pub(crate) fn new(
        ctx: &'ctx C,
        trait_def: &TraitDef,
        def_collector: &mut DefCollector,
    ) -> Result<Self, Vec<ResolveError>> {
        let self_gen = match trait_def.self_gen.get() {
            Some(def_id) => *def_id,
            None => {
                let def_id = GenDefId::new(def_collector.alloc_def_id());
                trait_def.self_gen.set(def_id).unwrap();
                def_id
            }
        };

        let genargs = collect_genargs(&trait_def.genargs, def_collector)?;

        Ok(Self {
            ctx,
            genargs,
            self_gen,
        })
    }
}

/// `TyDefResolveCtx::new` と同じ採番と重複検査。
/// あちらは `Self` を型として持つので共通化していない。
fn collect_genargs(
    genargs_decl: &Option<GenArgsDecl<GenDefId>>,
    def_collector: &mut DefCollector,
) -> Result<HashMap<InternedIdent, GenDefId>, Vec<ResolveError>> {
    let mut genargs = HashMap::new();
    let mut spans: HashMap<InternedIdent, Span> = HashMap::new();
    let mut errors = Vec::new();

    if let Some(genargs_decl) = genargs_decl {
        for item in &genargs_decl.genargs {
            let def_id = match item.def_id.get() {
                Some(def_id) => *def_id,
                None => {
                    let def_id = GenDefId::new(def_collector.alloc_def_id());
                    item.def_id.set(def_id).unwrap();
                    def_id
                }
            };

            match genargs.entry(item.id.id) {
                Entry::Vacant(e) => {
                    e.insert(def_id);
                    spans.insert(item.id.id, item.id.span.clone());
                }
                Entry::Occupied(_) => {
                    errors.push(ResolveError::DuplicatedGenName {
                        name: item.id.id,
                        span1: spans[&item.id.id].clone(),
                        span2: item.id.span.clone(),
                    });
                }
            }
        }
    }

    if errors.is_empty() {
        Ok(genargs)
    } else {
        Err(errors)
    }
}
