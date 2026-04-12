pub(crate) mod context;
mod symbols;
mod types;

#[cfg(test)]
mod tests;

use std::collections::HashMap;

use biwac_ast::{DefTyp, ImportDecl, QualifiedId, TypeDef};
use biwac_base::{ModPath, PackageName, PackageNameError, Span};
use biwac_hir::{
    Hir, HirError, Ident, ImplValDefContentKind, NativeTypeAliasDefContent, PkgId,
    StructDefContent, Ty, TyDefContentKind, TyExistence, TyId, ValDefContentKind, ValId,
};
use biwac_package_loader::Pkg;

use crate::context::{
    ty_phase::{
        fn_level::FnLevelTyResolveCtx, impl_level::ImplLevelTyResolveCtx,
        module_level::ModuleLevelTyResolveCtx, ty_alias_level::TyAliasResolveCtx,
    },
    val_phase::{
        fn_level::FnLevelResolveCtx, impl_level::ImplLevelResolveCtx,
        module_level::ModuleLevelResolveCtx,
    },
};

// このcrate biwac_name_resolver は、
// package内のあらゆる名前の解決をすることを目指す。
// - 関数や型といったグローバルなシンボルはもちろん、
// - 関数ローカルな変数の名前、
// - ~~構造体のメンバの名前に至るまで、~~
//
// - 同一の名前空間で名前の重複がないこと、
// - 名前空間の優先度に従って、その名前のシンボルの存在を確認し、絶対的な参照名に変換すること
// - モジュールレベルの公開設定に違反せずに、名前解決が出来ること
//   - ただし、構造体のメンバは型推論して始めて構造体の種類が特定できるなどの理由から
//     このパスでは行わないこととする
//     型の内容のチェックは次のパスに任せる
// のすべてを満たしつつ解決を目指す。

#[derive(Debug)]
pub enum ResolveError {
    HirError(HirError),
    TypeNotFound {
        qualid: Box<QualifiedId>,
        tid: Box<TyId>,
    },
    TypeNotFoundModuleFound {
        qualid: Box<QualifiedId>,
        import_decl: Box<ImportDecl>,
        modpath: Box<ModPath>,
    },
    TypeNotFoundValueFound {
        qualid: Box<QualifiedId>,
        import_decl: Box<ImportDecl>,
        vid: Box<ValId>,
    },
    ValueNotFound {
        qualid: Box<QualifiedId>,
        vid: Box<ValId>,
    },
    ValueNotFoundModuleFound {
        qualid: Box<QualifiedId>,
        import_decl: Box<ImportDecl>,
        modpath: Box<ModPath>,
    },
    ValueNotFoundTypeFound {
        qualid: Box<QualifiedId>,
        import_decl: Box<ImportDecl>,
        tid: Box<TyId>,
    },
    ImplementedValueNotFound {
        ty: Box<Ty>,
        val: String,
    },
    IdentifierNotFound {
        qualid: QualifiedId,
    },
    DuplicatedImportedName {
        name: String,
        imp1: Box<ImportDecl>,
        imp2: Box<ImportDecl>,
    },
    DuplicatedFnName {
        fid1: Box<Ident>,
        fid2: Box<Ident>,
    },
    DuplicatedTypeName {
        tid1: Box<biwac_ast::Ident>,
        tid2: Box<biwac_ast::Ident>,
    },
    DuplicatedVarName {
        vid1: Box<Ident>,
        vid2: Box<Ident>,
    },
    DuplicatedValueName {
        tid1: Box<Ident>,
        tid2: Box<Ident>,
    },
    DuplicatedGenericTypeDeclaration {
        tid1: Box<Ident>,
        tid2: Box<Ident>,
    },
    GenericArgLengthMismatched {
        // TODO: エラーメッセージを正確に出しやすく
        deftyp: Box<DefTyp>,
        tid: Box<TyId>,
        ty_existence: Box<TyExistence>,
    },
    CyclingTypeAlias {
        tid: Box<TyId>,
        detected_position: Box<Span>,
    },
    InsufficientDependencyPackageData {
        pkg: PackageName,
    },
    DependsOnSamePackageName {
        pkg: PackageName,
    },
    PackageNameError(PackageNameError), // CanNotBeImplementedForType {
                                        //     typ: Typ,
                                        // },
}

pub type RsvResult<T> = Result<T, ResolveError>;

// <----                           function id                                      ---->
// <----                               type id                                      ---->
// <package-name> :: <module-name>::<module-name>                         :: <identifier> :: [  ]
//
//
// <self-type>                                                            :: <identifier> :: [  ]
//
// <package-name> :: <module-name>::<module-name> :: <identifier> :: [  ]
// <----                      type id                       ---->
//
//

trait TryResolve<T>: Sized {
    fn try_resolve<'mctx>(
        value: T,
        fctx: &mut FnLevelResolveCtx<'mctx>,
        hir: &Hir,
    ) -> RsvResult<Self>;
}

// trait ImplLevelTryResolve<T>: Sized {
//     fn try_resolve_in_impl<'mctx>(
//         value: T,
//         ictx: &ImplLevelResolveCtx<'mctx>,
//         hir: &Hir,
//     ) -> RsvResult<Self>;
// }

// trait ModuleLevelTryResolve<T>: Sized {
//     fn try_resolve_in_module<'pctx>(
//         value: T,
//         mctx: &ModuleLevelResolveCtx,
//         hir: &Hir,
//     ) -> RsvResult<Self>;
// }

trait ModuleLevelTryResolveTy<T>: Sized {
    fn try_resolve_in_module(
        value: T,
        mctx: &ModuleLevelTyResolveCtx,
        hir: &Hir,
    ) -> RsvResult<Self>;
}

trait TryResolveTy<T>: Sized {
    fn try_resolve<'mctx>(
        value: T,
        fctx: &FnLevelTyResolveCtx<'mctx>,
        hir: &Hir,
    ) -> RsvResult<Self>;
}

impl From<HirError> for ResolveError {
    fn from(value: HirError) -> Self {
        Self::HirError(value)
    }
}

pub struct ResolveCtx {
    hir: Hir,
    pkg_name: PackageName,
}

impl ResolveCtx {
    pub fn new(
        metadata: &biwac_base::PackageMetadata,
        deps: &biwac_dependency_loader::Deps,
    ) -> Result<Self, ResolveError> {
        // dependency list file にすべての依存パッケージが記述されていることを検査
        let deps_pkgs: HashMap<_, _> = deps
            .deps_pkgs
            .iter()
            .map(|pkg| (pkg.name.value(), &pkg.symbols))
            .collect();

        for pkg in &metadata.dependencies {
            if !deps_pkgs.contains_key(pkg.name.value()) {
                return Err(ResolveError::InsufficientDependencyPackageData {
                    pkg: pkg.name.clone(),
                });
            }
        }

        if deps_pkgs.contains_key(metadata.name.value()) {
            return Err(ResolveError::DependsOnSamePackageName {
                pkg: metadata.name.clone(),
            });
        }

        Ok(Self {
            hir: Hir::new(metadata.name.clone()),
            pkg_name: metadata.name.clone(),
        })
    }

    pub fn try_resolve(mut self, pkg: Pkg) -> RsvResult<Hir> {
        // 型の存在を記録する
        let mut alias_defs = HashMap::new();
        for (modpath, modu) in &pkg.modules {
            // モジュールの存在も記録
            self.hir.register_module_existence(modpath.clone())?;

            for g in &modu.globals {
                if let biwac_ast::Globals::TypeDef(t) = g {
                    match t {
                        TypeDef::Struct(struct_) => {
                            let tid =
                                TyId::from_modpath(PkgId::Internal, modpath, struct_.id.id.clone());
                            self.hir.register_type_existence(
                                tid,
                                TyExistence {
                                    ty_name_span: struct_.id.span.clone().into(),
                                    genarg_len: struct_.genargs.len(),
                                },
                            )?;
                        }
                        TypeDef::TypeAlias(alias) => {
                            let tid = TyId::from_modpath(
                                PkgId::Internal,
                                modpath,
                                alias.ident.id.clone(),
                            );
                            self.hir.register_type_existence(
                                tid.clone(),
                                TyExistence {
                                    ty_name_span: alias.ident.span.clone().into(),
                                    genarg_len: alias.genargs.len(),
                                },
                            )?;

                            alias_defs.insert(tid, alias);
                        }
                        TypeDef::NativeTypeAlias(native) => {
                            let tid = TyId::from_modpath(
                                PkgId::Internal,
                                modpath,
                                native.ident.id.clone(),
                            );
                            self.hir.register_type_existence(
                                tid.clone(),
                                TyExistence {
                                    ty_name_span: native.ident.span.clone().into(),
                                    genarg_len: native.genargs.len(),
                                },
                            )?;
                        }
                    }
                }
            }
        }
        // hir から型の存在とジェネリック引数の長さを取得できるようになる
        // hir からモジュールの存在を取得できるようになる

        let mctxes = pkg
            .modules
            .iter()
            .map(|(modpath, modu)| {
                let mctx = ModuleLevelTyResolveCtx::new(modpath.clone(), modu, &self.hir)?;

                Ok((modpath.clone(), mctx))
            })
            .collect::<RsvResult<HashMap<ModPath, ModuleLevelTyResolveCtx>>>()?;

        // 型エイリアス type alias を正規化し記録する
        let aliases = TyAliasResolveCtx::new(&mctxes, alias_defs).try_resolve(&self.hir)?;
        for (tid, alias) in aliases {
            self.hir
                .register_type_content(&tid, TyDefContentKind::TypeAlias(Box::new(alias)))?;
        }

        // 型の実体(シグニチャ)を記録する
        for (modpath, modu) in &pkg.modules {
            let mctx = mctxes.get(modpath).unwrap();

            for g in &modu.globals {
                if let biwac_ast::Globals::TypeDef(type_def) = g {
                    match type_def {
                        TypeDef::Struct(struct_) => {
                            let tid =
                                TyId::from_modpath(PkgId::Internal, modpath, struct_.id.id.clone());

                            self.hir.register_type_content(
                                &tid,
                                TyDefContentKind::Struct(Box::new(
                                    StructDefContent::try_resolve_in_module(
                                        struct_, mctx, &self.hir,
                                    )?,
                                )),
                            )?;
                        }
                        TypeDef::TypeAlias(_) => {
                            // すでに解決済み
                        }
                        TypeDef::NativeTypeAlias(native) => {
                            let tid = TyId::from_modpath(
                                PkgId::Internal,
                                modpath,
                                native.ident.id.clone(),
                            );

                            self.hir.register_type_content(
                                &tid,
                                TyDefContentKind::NativeTypeAlias(Box::new(
                                    NativeTypeAliasDefContent {
                                        alias_name_span: native.ident.span.clone(),
                                        genargs: native
                                            .genargs
                                            .clone()
                                            .into_iter()
                                            .map(|g| g.into())
                                            .collect(),
                                        native: native.native.clone(),
                                        native_span: native.native_span.clone(),
                                    },
                                )),
                            )?;
                        }
                    }
                }
            }
        }
        // hir から型の実体を取得できるようになる

        // 値(fn, const)の存在(シグニチャ)を記録する
        for (modpath, modu) in pkg.modules {
            let mctx = mctxes.get(&modpath).unwrap();

            for g in modu.globals {
                match g {
                    biwac_ast::Globals::FnDef(fn_def) => {
                        // 関連関数のとき
                        if let Some(impl_ctx) = &fn_def.impl_ctx {
                            let ictx = ImplLevelTyResolveCtx::new(mctx, &impl_ctx.genargs)?;
                            let self_ty = ictx.try_resolve_ty(&impl_ctx.self_typ, &self.hir)?;

                            let fctx = FnLevelTyResolveCtx::new(&ictx, &fn_def.genargs)?;

                            let signature = biwac_hir::FnDefContentSignature::try_resolve(
                                (&fn_def.args, &fn_def.rtype),
                                &fctx,
                                &self.hir,
                            )?;

                            self.hir.register_impl_value_existence(
                                self_ty.kind,
                                ictx.impl_block_genargs,
                                &fn_def.id.clone(),
                                ImplValDefContentKind::Fn(Box::new(biwac_hir::FnDefContent::new(
                                    signature,
                                    fn_def,
                                    ictx.impl_block_genarg_vec,
                                ))),
                            )?;
                        } else {
                            // 通常の関数のとき
                            let vid = ValId::from_modpath(
                                PkgId::Internal,
                                &modpath,
                                fn_def.id.id.clone(),
                            );
                            let ictx = ImplLevelTyResolveCtx::new_empty(mctx);
                            let fctx = FnLevelTyResolveCtx::new(&ictx, &fn_def.genargs)?;

                            let signature = biwac_hir::FnDefContentSignature::try_resolve(
                                (&fn_def.args, &fn_def.rtype),
                                &fctx,
                                &self.hir,
                            )?;

                            self.hir.register_value_existence(
                                vid,
                                ValDefContentKind::Fn(Box::new(biwac_hir::FnDefContent::new(
                                    signature,
                                    fn_def,
                                    ictx.impl_block_genarg_vec,
                                ))),
                            )?;
                        }
                    }
                    biwac_ast::Globals::NativeFnDef(fn_def) => {
                        // 関連関数のとき
                        if let Some(impl_ctx) = &fn_def.impl_ctx {
                            let ictx = ImplLevelTyResolveCtx::new(mctx, &impl_ctx.genargs)?;
                            let self_ty = ictx.try_resolve_ty(&impl_ctx.self_typ, &self.hir)?;

                            let fctx = FnLevelTyResolveCtx::new(&ictx, &fn_def.genargs)?;

                            let signature = biwac_hir::FnDefContentSignature::try_resolve(
                                (&fn_def.args, &fn_def.rtype),
                                &fctx,
                                &self.hir,
                            )?;

                            self.hir.register_impl_value_existence(
                                self_ty.kind,
                                ictx.impl_block_genargs,
                                &fn_def.id.clone(),
                                ImplValDefContentKind::NativeFn(Box::new(
                                    biwac_hir::NativeFnDefContent::new(
                                        signature,
                                        fn_def,
                                        ictx.impl_block_genarg_vec,
                                    ),
                                )),
                            )?;
                        } else {
                            // 通常の関数のとき
                            let vid = ValId::from_modpath(
                                PkgId::Internal,
                                &modpath,
                                fn_def.id.id.clone(),
                            );
                            let ictx = ImplLevelTyResolveCtx::new_empty(mctx);
                            let fctx = FnLevelTyResolveCtx::new(&ictx, &fn_def.genargs)?;

                            let signature = biwac_hir::FnDefContentSignature::try_resolve(
                                (&fn_def.args, &fn_def.rtype),
                                &fctx,
                                &self.hir,
                            )?;

                            self.hir.register_value_existence(
                                vid,
                                ValDefContentKind::Native(Box::new(
                                    biwac_hir::NativeFnDefContent::new(
                                        signature,
                                        fn_def,
                                        ictx.impl_block_genarg_vec,
                                    ),
                                )),
                            )?;
                        }
                    }
                    biwac_ast::Globals::MethodDef(method_def) => {
                        let ictx = ImplLevelTyResolveCtx::new(mctx, &method_def.impl_genargs)?;
                        let self_ty = ictx.try_resolve_ty(&method_def.self_typ, &self.hir)?;

                        let fctx = FnLevelTyResolveCtx::new(&ictx, &method_def.genargs)?;

                        // 第一引数 self は含まない
                        let signature = biwac_hir::FnDefContentSignature::try_resolve(
                            (&method_def.args, &method_def.rtype),
                            &fctx,
                            &self.hir,
                        )?;

                        // メソッドとして登録
                        self.hir.register_impl_value_existence(
                            self_ty.kind,
                            ictx.impl_block_genargs,
                            &method_def.id.clone(),
                            ImplValDefContentKind::Method(Box::new(
                                biwac_hir::MethodDefContent::new(
                                    signature,
                                    method_def,
                                    ictx.impl_block_genarg_vec,
                                ),
                            )),
                        )?;
                    }
                    biwac_ast::Globals::NativeMethodDef(method_def) => {
                        let ictx = ImplLevelTyResolveCtx::new(mctx, &method_def.impl_genargs)?;
                        let self_ty = ictx.try_resolve_ty(&method_def.self_typ, &self.hir)?;

                        let fctx = FnLevelTyResolveCtx::new(&ictx, &method_def.genargs)?;

                        // 第一引数 self は含まない
                        let signature = biwac_hir::FnDefContentSignature::try_resolve(
                            (&method_def.args, &method_def.rtype),
                            &fctx,
                            &self.hir,
                        )?;

                        // メソッドとして登録
                        self.hir.register_impl_value_existence(
                            self_ty.kind.clone(),
                            ictx.impl_block_genargs,
                            &method_def.id.clone(),
                            ImplValDefContentKind::NativeMethod(Box::new(
                                biwac_hir::NativeMethodDefContent::new(
                                    signature,
                                    method_def,
                                    self_ty,
                                    ictx.impl_block_genarg_vec,
                                ),
                            )),
                        )?;
                    }
                    biwac_ast::Globals::VarDecl(_var_decl) => {
                        todo!()
                    }
                    biwac_ast::Globals::Import(_) => {
                        // nothing to do
                    }
                    biwac_ast::Globals::TypeDef(_) => {
                        // nothing to do
                    }
                    biwac_ast::Globals::NativeCode(native) => {
                        self.hir
                            .register_module_native_code(modpath.clone(), &native);
                    }
                    biwac_ast::Globals::NovelScene(scene_def) => {
                        let vid =
                            ValId::from_modpath(PkgId::Internal, &modpath, scene_def.id.id.clone());
                        let ictx = ImplLevelTyResolveCtx::new_empty(mctx);
                        let fctx = FnLevelTyResolveCtx::new(&ictx, &Vec::new())?; // ジェネリック引数列は必ず空

                        let signature = biwac_hir::FnDefContentSignature::try_resolve(
                            (&scene_def.args, &scene_def.rtype),
                            &fctx,
                            &self.hir,
                        )?;

                        self.hir.register_value_existence(
                            vid,
                            ValDefContentKind::NovelScene(Box::new(
                                biwac_hir::NovelSceneDefContent::new(signature, scene_def),
                            )),
                        )?;
                    }
                }
            }
        }
        // hir から値の存在(シグニチャ)を取得できるようになる

        // 関数内の名前解決を行う
        let mctxes = mctxes
            .into_iter()
            .map(|(modpath, mctx)| Ok((modpath, ModuleLevelResolveCtx::new(mctx, &self.hir)?)))
            .collect::<RsvResult<HashMap<ModPath, ModuleLevelResolveCtx>>>()?;

        // 通常の関数に対し
        // 解決を行う
        let mut fn_bodies = vec![];
        for (vid, val) in &self.hir.vals {
            match val {
                ValDefContentKind::Fn(f) => {
                    match &f.body {
                        biwac_hir::Progressive::NotYet(fn_def) => {
                            let mctx = mctxes
                                .get(fn_def.id.span.module())
                                .expect("compiler bug: module not found");
                            let ictx = ImplLevelResolveCtx::new_empty(mctx);
                            let mut fctx = FnLevelResolveCtx::new(&ictx, &f.signature.genargs)?;

                            let fn_body = biwac_hir::FnDefContentBody::try_resolve(
                                (fn_def, &f.signature),
                                &mut fctx,
                                &self.hir,
                            )?;

                            fn_bodies.push((vid.clone(), fn_body));
                        }
                        biwac_hir::Progressive::Completed(_) => {
                            // nothing to do
                        }
                    }
                }
                ValDefContentKind::NovelScene(scene) => {
                    match &scene.body {
                        biwac_hir::Progressive::NotYet(scene_def) => {
                            let mctx = mctxes
                                .get(scene_def.id.span.module())
                                .expect("compiler bug: module not found");
                            let ictx = ImplLevelResolveCtx::new_empty(mctx);
                            let mut fctx = FnLevelResolveCtx::new(&ictx, &scene.signature.genargs)?;

                            let fn_body = biwac_hir::FnDefContentBody::try_resolve(
                                (scene_def, &scene.signature),
                                &mut fctx,
                                &self.hir,
                            )?;

                            fn_bodies.push((vid.clone(), fn_body));
                        }
                        biwac_hir::Progressive::Completed(_) => {
                            // nothing to do
                        }
                    }
                }
                ValDefContentKind::Native(_) => {
                    // nothing to do
                }
            }
        }

        // 解決済みの関数のボディ情報を登録する
        for (vid, fn_body) in fn_bodies {
            self.hir.register_value_definition(&vid, fn_body)?;
        }

        // ユーザ定義型の関連関数、メソッドに対し
        // 解決を行う
        let mut impl_fn_bodies = vec![];
        for (tid, defined_ty_impl) in &self.hir.tys {
            for (val_name, impl_list) in &defined_ty_impl.vals {
                for (impl_vid, val) in &impl_list.vals {
                    match &val.val_content {
                        ImplValDefContentKind::Fn(f) => {
                            match &f.body {
                                biwac_hir::Progressive::NotYet(fn_body) => {
                                    let mctx = mctxes
                                        .get(f.fn_name_span.module())
                                        .expect("compiler bug: module not found");
                                    let ictx = ImplLevelResolveCtx::new(
                                        mctx,
                                        val.impl_block_genargs.clone(),
                                    )?;
                                    let mut fctx =
                                        FnLevelResolveCtx::new(&ictx, &f.signature.genargs)?;

                                    let fn_body = biwac_hir::FnDefContentBody::try_resolve(
                                        (fn_body, &f.signature),
                                        &mut fctx,
                                        &self.hir,
                                    )?;

                                    impl_fn_bodies.push((
                                        tid.clone(),
                                        val_name.clone(),
                                        *impl_vid,
                                        fn_body,
                                    ));
                                }
                                biwac_hir::Progressive::Completed(_) => {
                                    // nothing to do
                                }
                            }
                        }
                        ImplValDefContentKind::Method(m) => {
                            match &m.body {
                                biwac_hir::Progressive::NotYet(fn_body) => {
                                    let mctx = mctxes
                                        .get(m.fn_name_span.module())
                                        .expect("compiler bug: module not found");
                                    let ictx = ImplLevelResolveCtx::new(
                                        mctx,
                                        val.impl_block_genargs.clone(),
                                    )?;
                                    let mut fctx =
                                        FnLevelResolveCtx::new(&ictx, &m.signature.genargs)?;

                                    let fn_body = biwac_hir::FnDefContentBody::try_resolve(
                                        (fn_body, &m.signature),
                                        &mut fctx,
                                        &self.hir,
                                    )?;

                                    impl_fn_bodies.push((
                                        tid.clone(),
                                        val_name.clone(),
                                        *impl_vid,
                                        fn_body,
                                    ));
                                }
                                biwac_hir::Progressive::Completed(_) => {
                                    // nothing to do
                                }
                            }
                        }
                        ImplValDefContentKind::NativeFn(_)
                        | ImplValDefContentKind::NativeMethod(_) => {
                            // nothing to do
                        }
                    }
                }
            }
        }

        // 解決済みの関数のボディ情報を登録する
        for (tid, val_name, impl_vid, fn_body) in impl_fn_bodies {
            self.hir
                .register_impl_value_definition(&tid, &val_name, &impl_vid, fn_body)?;
        }

        // プリミティブ型などの関連関数、メソッドに対し
        // 解決を行う
        let mut special_impl_fn_bodies = vec![];
        for (ty, special_ty_impl) in &self.hir.special_ty_impls {
            for (val_name, val) in &special_ty_impl.vals {
                match &val {
                    ImplValDefContentKind::Fn(f) => {
                        match &f.body {
                            biwac_hir::Progressive::NotYet(fn_body) => {
                                let mctx = mctxes
                                    .get(f.fn_name_span.module())
                                    .expect("compiler bug: module not found");
                                // プリミティブ型などに対して impl block レベルでジェネリック型宣言はないはずなので空
                                let ictx = ImplLevelResolveCtx::new(mctx, [].into())?;
                                let mut fctx = FnLevelResolveCtx::new(&ictx, &f.signature.genargs)?;

                                let fn_body = biwac_hir::FnDefContentBody::try_resolve(
                                    (fn_body, &f.signature),
                                    &mut fctx,
                                    &self.hir,
                                )?;

                                special_impl_fn_bodies.push((
                                    ty.clone(),
                                    val_name.clone(),
                                    fn_body,
                                ));
                            }
                            biwac_hir::Progressive::Completed(_) => {
                                // nothing to do
                            }
                        }
                    }
                    ImplValDefContentKind::Method(m) => {
                        match &m.body {
                            biwac_hir::Progressive::NotYet(fn_body) => {
                                let mctx = mctxes
                                    .get(m.fn_name_span.module())
                                    .expect("compiler bug: module not found");
                                // プリミティブ型などに対して impl block レベルでジェネリック型宣言はないはずなので空
                                let ictx = ImplLevelResolveCtx::new(mctx, [].into())?;
                                let mut fctx = FnLevelResolveCtx::new(&ictx, &m.signature.genargs)?;

                                let fn_body = biwac_hir::FnDefContentBody::try_resolve(
                                    (fn_body, &m.signature),
                                    &mut fctx,
                                    &self.hir,
                                )?;

                                special_impl_fn_bodies.push((
                                    ty.clone(),
                                    val_name.clone(),
                                    fn_body,
                                ));
                            }
                            biwac_hir::Progressive::Completed(_) => {
                                // nothing to do
                            }
                        }
                    }
                    ImplValDefContentKind::NativeFn(_) | ImplValDefContentKind::NativeMethod(_) => {
                        // nothing to do
                    }
                }
            }
        }

        // 解決済みの関数のボディ情報を登録する
        for (ty, val_name, fn_body) in special_impl_fn_bodies {
            self.hir
                .register_special_impl_value_definition(&ty, &val_name, fn_body)?;
        }

        Ok(self.hir)
    }
}
