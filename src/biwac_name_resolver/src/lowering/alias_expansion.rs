use std::collections::{HashMap, HashSet};

use biwac_hir::{
    AssocValDefKind, BlockExpr, BlockStmt, Callee, DefinedTy, Expr, ExprVal, FnBody, FnDef,
    FnSignature, FnTy, Hir, Literal, NativeFnDef, NovelSceneDef, Primary, Stmt, Ty, TyDefKind,
    TyKind, TypeAliasDef, ValDefKind, VariantCtorFields,
};
use biwac_span::{GenDefId, TyDefId};

use crate::ResolveError;

pub(super) fn expand_aliases(hir: &mut Hir, errors: &mut Vec<ResolveError>) {
    if let Err(errs) = detect_alias_cycles(&hir.ty_aliases) {
        errors.extend(errs);
        return;
    }

    let aliases = hir.ty_aliases.clone();

    for defined_ty_impl in hir.tys.values_mut() {
        if let Some(TyDefKind::Struct(struct_def)) = &mut defined_ty_impl.ty_content {
            for member_ty in struct_def.members.values_mut() {
                *member_ty = expand_ty(member_ty.clone(), &aliases);
            }
        }
        for impl_list in defined_ty_impl.vals.values_mut() {
            for pair in impl_list.vals.values_mut() {
                pair.genargs = pair
                    .genargs
                    .iter()
                    .cloned()
                    .map(|t| expand_ty(t, &aliases))
                    .collect();
                expand_assoc_val_def_kind(&mut pair.val_content, &aliases);
            }
        }
    }

    for val in hir.vals.values_mut() {
        expand_val_def_kind(val, &aliases);
    }
}

fn detect_alias_cycles(aliases: &HashMap<TyDefId, TypeAliasDef>) -> Result<(), Vec<ResolveError>> {
    let mut errors = Vec::new();
    let mut visited: HashSet<TyDefId> = HashSet::new();
    let mut in_progress: HashSet<TyDefId> = HashSet::new();

    for &def_id in aliases.keys() {
        if !visited.contains(&def_id) {
            dfs_detect(def_id, aliases, &mut visited, &mut in_progress, &mut errors);
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn dfs_detect(
    current: TyDefId,
    aliases: &HashMap<TyDefId, TypeAliasDef>,
    visited: &mut HashSet<TyDefId>,
    in_progress: &mut HashSet<TyDefId>,
    errors: &mut Vec<ResolveError>,
) {
    in_progress.insert(current);

    let refs = collect_alias_refs(&aliases[&current].right.kind, aliases);

    for next in refs {
        if in_progress.contains(&next) {
            errors.push(ResolveError::CyclingTypeAlias {
                def_id: Box::new(next),
                detected_position: Box::new(aliases[&next].name.span.clone()),
            });
        } else if !visited.contains(&next) {
            dfs_detect(next, aliases, visited, in_progress, errors);
        }
    }

    in_progress.remove(&current);
    visited.insert(current);
}

fn collect_alias_refs(kind: &TyKind, aliases: &HashMap<TyDefId, TypeAliasDef>) -> Vec<TyDefId> {
    let mut refs = Vec::new();
    collect_alias_refs_inner(kind, aliases, &mut refs);
    refs
}

fn collect_alias_refs_inner(
    kind: &TyKind,
    aliases: &HashMap<TyDefId, TypeAliasDef>,
    refs: &mut Vec<TyDefId>,
) {
    match kind {
        TyKind::Defined(DefinedTy { def_id, genargs }) => {
            if aliases.contains_key(def_id) {
                refs.push(*def_id);
            }
            for g in genargs {
                collect_alias_refs_inner(&g.kind, aliases, refs);
            }
        }
        TyKind::Fn(FnTy { args, rty, .. }) => {
            for a in args {
                collect_alias_refs_inner(&a.kind, aliases, refs);
            }
            collect_alias_refs_inner(&rty.kind, aliases, refs);
        }
        _ => {}
    }
}

pub(super) fn expand_ty(ty: Ty, aliases: &HashMap<TyDefId, TypeAliasDef>) -> Ty {
    let span = ty.span;
    match ty.kind {
        TyKind::Defined(DefinedTy { def_id, genargs }) => {
            let expanded_genargs: Vec<Ty> =
                genargs.into_iter().map(|g| expand_ty(g, aliases)).collect();
            if let Some(alias) = aliases.get(&def_id) {
                let assigns: HashMap<GenDefId, TyKind> = alias
                    .genargs
                    .iter()
                    .copied()
                    .zip(expanded_genargs.iter().map(|t| t.kind.clone()))
                    .collect();
                let substituted = alias.right.clone().embody_by_gen_ty_id(&assigns);
                expand_ty(substituted, aliases)
            } else {
                Ty::new(
                    TyKind::Defined(DefinedTy {
                        def_id,
                        genargs: expanded_genargs,
                    }),
                    span,
                )
            }
        }
        TyKind::Fn(FnTy { args, rty, genargs }) => Ty::new(
            TyKind::Fn(FnTy {
                args: args.into_iter().map(|a| expand_ty(a, aliases)).collect(),
                rty: Box::new(expand_ty(*rty, aliases)),
                genargs,
            }),
            span,
        ),
        kind => Ty::new(kind, span),
    }
}

fn expand_fn_signature(sig: &mut FnSignature, aliases: &HashMap<TyDefId, TypeAliasDef>) {
    for arg in &mut sig.args {
        arg.ty = expand_ty(arg.ty.clone(), aliases);
    }
    if let Some(self_ty) = &mut sig.self_ty {
        *self_ty = expand_ty(self_ty.clone(), aliases);
    }
    if let Some(impl_self_ty) = &mut sig.impl_self_ty {
        *impl_self_ty = expand_ty(impl_self_ty.clone(), aliases);
    }
    sig.rty = expand_ty(sig.rty.clone(), aliases);
}

fn expand_fn_body(body: &mut FnBody, aliases: &HashMap<TyDefId, TypeAliasDef>) {
    for decled_var in body.vars.values_mut() {
        decled_var.ty = expand_ty(decled_var.ty.clone(), aliases);
    }
    for stmt in &mut body.stmts {
        expand_stmt(stmt, aliases);
    }
    if let Some(expr) = &mut body.expr {
        expand_expr(expr, aliases);
    }
}

// 式の中にも型が現れる。
//
// `Callee::AssocFn` の `self_ty` がそれで、`CharacterBiwa::new(..)` の
// `CharacterBiwa` をここで `Character[BiwaCharacterProps]` に置き換える。
// これをやらないと推論がエイリアスの型引数を受け取れない。

fn expand_stmt(stmt: &mut Stmt, aliases: &HashMap<TyDefId, TypeAliasDef>) {
    match stmt {
        Stmt::Block(b) => expand_block_stmt(b, aliases),
        Stmt::Expr(e) => expand_expr(&mut e.expr, aliases),
        Stmt::Return(r) => expand_expr(&mut r.expr, aliases),
        Stmt::If(i) => {
            expand_expr(&mut i.cond, aliases);
            expand_block_stmt(&mut i.then, aliases);
            if let Some(els) = &mut i.els {
                expand_block_stmt(els, aliases);
            }
        }
        Stmt::While(w) => {
            expand_expr(&mut w.cond, aliases);
            expand_block_stmt(&mut w.stmts, aliases);
        }
        Stmt::Match(m) => {
            expand_expr(&mut m.scrutinee, aliases);
            for arm in &mut m.arms {
                expand_block_stmt(&mut arm.body, aliases);
            }
        }
        Stmt::VarDecl(v) => expand_expr(&mut v.init, aliases),
        Stmt::Assign(a) => {
            expand_primary(&mut a.dst, aliases);
            expand_expr(&mut a.src, aliases);
        }
        Stmt::NovelWrite(_) | Stmt::NovelWait(_) => {}
    }
}

fn expand_block_stmt(block: &mut BlockStmt, aliases: &HashMap<TyDefId, TypeAliasDef>) {
    for stmt in &mut block.stmts {
        expand_stmt(stmt, aliases);
    }
}

fn expand_block_expr(block: &mut BlockExpr, aliases: &HashMap<TyDefId, TypeAliasDef>) {
    for stmt in &mut block.stmts {
        expand_stmt(stmt, aliases);
    }
    expand_expr(&mut block.expr, aliases);
}

fn expand_expr(expr: &mut Expr, aliases: &HashMap<TyDefId, TypeAliasDef>) {
    match &mut expr.expr {
        ExprVal::Primary(p) => expand_primary(p, aliases),
        ExprVal::Unary(u) => expand_expr(&mut u.right, aliases),
        ExprVal::Binary(b) => {
            expand_expr(&mut b.left, aliases);
            expand_expr(&mut b.right, aliases);
        }
    }
}

fn expand_primary(primary: &mut Primary, aliases: &HashMap<TyDefId, TypeAliasDef>) {
    match primary {
        Primary::FnCall(call) => {
            if let Callee::AssocFn { self_ty, .. } = &mut call.callee {
                *self_ty = expand_ty(self_ty.clone(), aliases);
            }
            for arg in &mut call.args {
                expand_expr(arg, aliases);
            }
        }
        Primary::MethodCall(m) => {
            expand_expr(&mut m.left, aliases);
            for arg in &mut m.args {
                expand_expr(arg, aliases);
            }
        }
        Primary::Match(m) => {
            expand_expr(&mut m.scrutinee, aliases);
            for arm in &mut m.arms {
                expand_block_expr(&mut arm.body, aliases);
            }
        }
        // バリアント自体は型ではないので展開の対象にならない。実引数だけ歩く。
        Primary::VariantCtor(v) => match &mut v.fields {
            VariantCtorFields::Unit => {}
            VariantCtorFields::Positional(exprs) => {
                for expr in exprs {
                    expand_expr(expr, aliases);
                }
            }
            VariantCtorFields::Named(fields) => {
                for (_, expr) in fields {
                    expand_expr(expr, aliases);
                }
            }
        },
        Primary::MemberAccess(m) => expand_expr(&mut m.left, aliases),
        Primary::IfExpr(i) => {
            expand_expr(&mut i.cond, aliases);
            expand_block_expr(&mut i.then, aliases);
            expand_block_expr(&mut i.els, aliases);
        }
        Primary::Block(b) => expand_block_expr(b, aliases),
        Primary::Literal(Literal::Struct(s)) => {
            for (_, member) in &mut s.members {
                expand_expr(member, aliases);
            }
        }
        Primary::Literal(_) | Primary::Variable(_) => {}
    }
}

fn expand_fn_def(fn_def: &mut FnDef, aliases: &HashMap<TyDefId, TypeAliasDef>) {
    expand_fn_signature(&mut fn_def.signature, aliases);
    expand_fn_body(&mut fn_def.body, aliases);
}

fn expand_native_fn_def(fn_def: &mut NativeFnDef, aliases: &HashMap<TyDefId, TypeAliasDef>) {
    expand_fn_signature(&mut fn_def.signature, aliases);
}

fn expand_novel_scene_def(scene: &mut NovelSceneDef, aliases: &HashMap<TyDefId, TypeAliasDef>) {
    expand_fn_signature(&mut scene.signature, aliases);
    expand_fn_body(&mut scene.body, aliases);
}

fn expand_assoc_val_def_kind(val: &mut AssocValDefKind, aliases: &HashMap<TyDefId, TypeAliasDef>) {
    match val {
        AssocValDefKind::Fn(fn_def) => expand_fn_def(fn_def, aliases),
        AssocValDefKind::NativeFn(fn_def) => expand_native_fn_def(fn_def, aliases),
    }
}

fn expand_val_def_kind(val: &mut ValDefKind, aliases: &HashMap<TyDefId, TypeAliasDef>) {
    match val {
        ValDefKind::Fn(fn_def) => expand_fn_def(fn_def, aliases),
        ValDefKind::Native(fn_def) => expand_native_fn_def(fn_def, aliases),
        ValDefKind::NovelScene(scene) => expand_novel_scene_def(scene, aliases),
    }
}
