mod expressions;
mod globals;
mod novel;
mod statements;

pub(crate) use expressions::ExprLowerCtx;

use biwac_ast::{Path, PathSegmentResolution, PrimTyp, TypRepr, TypReprVal};
use biwac_base::PackageName;
use biwac_hir::{DefinedTy, Hir, Ty, TyKind};
use biwac_package_loader::{LoadedModule, Pkg};
use biwac_span::{DefIdKind, GenDefId, LocalGenDefId, TyDefId};

use crate::ResolveError;

pub(crate) fn lower(pkg_name: PackageName, pkg: Pkg) -> Result<Hir, Vec<ResolveError>> {
    let mut hir = Hir::new(pkg_name);
    let mut errors = Vec::new();

    // Pass 1: register all type definitions so impl blocks can reference them.
    lower_module_types(&mut hir, &pkg.root_module, &mut errors);

    // Pass 2: lower all values (fns, impls, novel scenes, native code).
    lower_module_vals(&mut hir, &pkg.root_module, &mut errors);

    if errors.is_empty() {
        Ok(hir)
    } else {
        Err(errors)
    }
}

fn lower_module_types(hir: &mut Hir, module: &LoadedModule, errors: &mut Vec<ResolveError>) {
    for g in &module.ast.globals {
        if let biwac_ast::Globals::TypeDef(type_def) = g {
            globals::lower_type_def(hir, type_def, errors);
        }
    }
    for child in module.children.values() {
        lower_module_types(hir, child, errors);
    }
}

fn lower_module_vals(hir: &mut Hir, module: &LoadedModule, errors: &mut Vec<ResolveError>) {
    for g in &module.ast.globals {
        match g {
            biwac_ast::Globals::FnDef(fn_def) => {
                globals::lower_fn_def(hir, fn_def, vec![], errors);
            }
            biwac_ast::Globals::NativeFnDef(fn_def) => {
                globals::lower_native_fn_def(hir, fn_def, vec![], errors);
            }
            biwac_ast::Globals::ImplBlock(impl_block) => {
                globals::lower_impl_block(hir, impl_block, errors);
            }
            biwac_ast::Globals::NovelScene(scene_def) => {
                novel::lower_novel_scene(hir, scene_def, errors);
            }
            biwac_ast::Globals::NativeCode(code) => {
                globals::lower_native_code(hir, &module.ast.modpath, code);
            }
            biwac_ast::Globals::TypeDef(_)
            | biwac_ast::Globals::Import(_)
            | biwac_ast::Globals::VarDecl(_) => {}
        }
    }
    for child in module.children.values() {
        lower_module_vals(hir, child, errors);
    }
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
            .expect("compiler bug: SelfTyp outside impl context")
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
    panic!("compiler bug: path was not resolved before lowering")
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
