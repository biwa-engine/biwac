mod alias_expansion;
mod expressions;
mod globals;
mod novel;
mod statements;

use std::collections::HashMap;

pub(crate) use expressions::ExprLowerCtx;

use biwac_ast::{Path, PathSegmentResolution, PrimTyp, TypRepr, TypReprVal};
use biwac_base::{InternedIdent, PackageId, PackageName};
use biwac_hir::{DefinedTy, DefinedTyImpl, Hir, Ty, TyKind, ValDefKind};
use biwac_package_loader::{LoadedModule, Pkg};
use biwac_span::{DefIdKind, GenDefId, LocalGenDefId, TyDefId, ValDefId};

use crate::{ResolveError, resolving::def_collector::ImplCollector};

pub(crate) fn lower(
    pkg_name: PackageName,
    pkg: Pkg,
    pkg_names: HashMap<PackageId, InternedIdent>,
    impl_collector: &ImplCollector,
) -> Result<Hir, Vec<ResolveError>> {
    let mut errors = Vec::new();

    // Pass 1: register all type definitions so impl blocks can reference them.
    let mut tys = lower_module_types(&pkg.root_module, &mut errors)
        .into_iter()
        .map(|(def_id, ty)| (def_id, ty))
        .collect();

    // NOTE: maybe unnecessary because alias expanded (and cycle detected) in resolving path.
    //
    // // Pass 2: expand type aliases recursively, detect cycles.
    // alias_expansion::expand_aliases(&mut hir, &mut errors);

    // Pass 2: lower impl blocks
    lower_impl_blocks(&mut tys, &pkg.root_module, impl_collector, &mut errors);

    // Pass 3: lower all values (fns, impls, novel scenes, native code).
    let vals = lower_module_vals(&pkg.root_module, &mut errors)
        .into_iter()
        .map(|(def_id, val)| (def_id, val))
        .collect();

    // Pass 4: lower native codes
    let native_codes = lower_native_codes(&pkg.root_module);

    if errors.is_empty() {
        Ok(Hir::new(pkg_name, pkg_names, tys, vals, native_codes))
    } else {
        Err(errors)
    }
}

fn lower_module_types(
    module: &LoadedModule,
    errors: &mut Vec<ResolveError>,
) -> Vec<(TyDefId, DefinedTyImpl)> {
    let mut tys = Vec::new();

    for g in &module.ast.globals {
        if let biwac_ast::Globals::TypeDef(type_def) = g {
            tys.extend(globals::lower_type_def(type_def, errors));
        }
    }
    for child in module.children.values() {
        tys.extend(lower_module_types(child, errors));
    }

    tys
}

fn lower_impl_blocks(
    tys: &mut HashMap<TyDefId, DefinedTyImpl>,
    module: &LoadedModule,
    impl_collector: &ImplCollector,
    errors: &mut Vec<ResolveError>,
) {
    for g in &module.ast.globals {
        if let biwac_ast::Globals::ImplBlock(impl_block) = g {
            globals::lower_impl_block(tys, impl_block, impl_collector, errors);
        }
    }
    for child in module.children.values() {
        lower_impl_blocks(tys, child, impl_collector, errors);
    }
}

fn lower_module_vals(
    module: &LoadedModule,
    errors: &mut Vec<ResolveError>,
) -> Vec<(ValDefId, ValDefKind)> {
    let mut vals = Vec::new();

    for g in &module.ast.globals {
        match g {
            biwac_ast::Globals::FnDef(fn_def) => {
                vals.push(globals::lower_fn_def(fn_def, vec![], errors));
            }
            biwac_ast::Globals::NativeFnDef(fn_def) => {
                vals.push(globals::lower_native_fn_def(fn_def, vec![], errors));
            }
            biwac_ast::Globals::NovelScene(scene_def) => {
                vals.push(novel::lower_novel_scene(scene_def, errors));
            }
            biwac_ast::Globals::TypeDef(_)
            | biwac_ast::Globals::Import(_)
            | biwac_ast::Globals::VarDecl(_)
            | biwac_ast::Globals::ImplBlock(_)
            | biwac_ast::Globals::NativeCode(_) => {}
        }
    }
    for child in module.children.values() {
        vals.extend(lower_module_vals(child, errors));
    }

    vals
}

fn lower_native_codes(module: &LoadedModule) -> Vec<biwac_hir::NativeCode> {
    let mut codes = Vec::new();

    for g in &module.ast.globals {
        if let biwac_ast::Globals::NativeCode(native) = g {
            codes.push(globals::lower_native_code(native));
        }
    }
    for child in module.children.values() {
        codes.extend(lower_native_codes(child));
    }

    codes
}

/// Converts a resolved TypRepr to TyKind.
/// `self_typ` is Some only inside impl blocks (for `Self` type references).
///
/// NOTE: `typ` path segments must have been resolved before calling this.
pub(crate) fn ty_kind_from_typ_repr(typ: &TypRepr, self_typ: Option<&TyKind>) -> TyKind {
    match &typ.val {
        TypReprVal::Primitive(p) => match p {
            PrimTyp::Int => TyKind::Int,
            PrimTyp::Uint => TyKind::Int, // TODO: proper Uint type
            PrimTyp::Float => TyKind::Float,
            PrimTyp::Bool => TyKind::Bool,
        },
        TypReprVal::Defined(deftyp) => match ty_def_id_kind_from_path(&deftyp.path) {
            Ok(TyDefIdKind::Ty(def_id)) => TyKind::Defined(DefinedTy {
                def_id,
                genargs: deftyp
                    .genargs
                    .iter()
                    .flat_map(|genargs| {
                        genargs
                            .iter()
                            .map(|t| Ty::new(ty_kind_from_typ_repr(t, self_typ), t.span.clone()))
                    })
                    .collect(),
            }),
            Ok(TyDefIdKind::Gen(gid)) => TyKind::Gen(gid),
            Ok(TyDefIdKind::LocalGen(lgid)) => TyKind::LocGen(lgid),
            Err(_) => TyKind::Infer(biwac_hir::InferTy::Unknown),
        },
        TypReprVal::SelfTyp => self_typ
            .unwrap_or_else(|| panic!("compiler bug: SelfTyp outside impl context: {typ:?}"))
            .clone(),
    }
}

pub(crate) fn ty_from_typ_repr(typ: &TypRepr, self_typ: Option<&TyKind>) -> Ty {
    Ty::new(ty_kind_from_typ_repr(typ, self_typ), typ.span.clone())
}

/// Returns the resolved DefIdKind from the final segment of a resolved path.
///
/// NOTE: panics if the path was not resolved — call only after successful name resolution.
pub(crate) fn def_id_kind_from_path(path: &Path) -> Result<DefIdKind, ResolveError> {
    if path.segments.is_empty() {
        match path
            .abs_header
            .as_ref()
            .expect("compiler bug: completely empty Path")
        {
            biwac_ast::AbsolutePathHeader::Package(_) => todo!(),
            biwac_ast::AbsolutePathHeader::SelfTyp(self_typ) => {
                return Ok(DefIdKind::Ty(
                    *self_typ
                        .resolved_id
                        .get()
                        .expect("compiler bug: path was not resolved before lowering"),
                ));
            }
        }
    }

    for (i, segment) in path.segments.iter().enumerate() {
        match segment.resolved_id.get() {
            Some(PathSegmentResolution::Ok(def_id_kind)) => {
                if i + 1 == path.segments.len() {
                    return Ok(def_id_kind.clone());
                }
                // continue to next segment
            }
            Some(PathSegmentResolution::Err) => {
                return Err(ResolveError::IdentNotFound {
                    ident: segment.ident.clone(),
                });
            }
            None => break,
        }
    }
    panic!("compiler bug: path was not resolved before lowering: {path:?}")
}

pub(crate) enum TyDefIdKind {
    Ty(TyDefId),
    Gen(GenDefId),
    LocalGen(LocalGenDefId),
}

pub(crate) fn ty_def_id_kind_from_path(path: &Path) -> Result<TyDefIdKind, ResolveError> {
    match def_id_kind_from_path(path)? {
        DefIdKind::Ty(id) => Ok(TyDefIdKind::Ty(id)),
        DefIdKind::Gen(id) => Ok(TyDefIdKind::Gen(id)),
        DefIdKind::LocalGen(id) => Ok(TyDefIdKind::LocalGen(id)),
        DefIdKind::Package(pkg_id) => Err(ResolveError::TypeNotFoundPackageFound {
            path: Box::new(path.clone()),
            pkg_id,
        }),
        DefIdKind::Mod(mod_id) => Err(ResolveError::TypeNotFoundModuleFound {
            path: Box::new(path.clone()),
            mod_id,
        }),
        DefIdKind::Val(def_id) => Err(ResolveError::TypeNotFoundValueFound {
            path: Box::new(path.clone()),
            def_id,
        }),
        DefIdKind::Var(var_id) => Err(ResolveError::TypeNotFoundVariableFound {
            path: Box::new(path.clone()),
            var_id,
        }),
    }
}
