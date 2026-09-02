mod body;
mod codec;
mod format;
mod lang_item;
pub mod module_view;
mod table;

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::error::DepMetadataError;
use body::SymbolBody;
use codec::{DiskDecode, DiskEncode};
pub use format::BIWAC_DEPENDENCY_METADATA_FORMAT_VERSION;
use format::{
    BIWAC_DEPENDENCY_METADATA_MAGIC, DiskBodyOffset, DiskDepSvh, DiskExternalSymbol, DiskFileIndex,
    DiskSourceInfo, DiskSpan, DiskSymbolHeader, DiskSymbolIndex, DiskSymbolKind, DiskTy,
    DiskTyHeader, DiskTyKind, DiskVisibility,
};
use lang_item::DiskLangItem;
use table::{LazyDiskVec, SourceFileTable, StringTable};

/// ロード済みの依存パッケージメタデータ。
/// sym_hdr_table と source_file_table は常にフルロード (固定長・小サイズ)。
/// sym_body_table は LazyDiskVec による遅延デコード。
/// string_table は常にフルロード (参照が頻繁なため)。
pub struct DepMetadata {
    pub sym_hdrs: Vec<DiskSymbolHeader>,
    pub sym_bodies: LazyDiskVec,
    pub source_files: SourceFileTable,
    pub strings: StringTable,
    /// ルートモジュール (lib モジュール) のシンボルインデックス
    pub root_sym_idx: u32,
    /// このパッケージが定義した lang item。
    /// 依存側はこれを使って lang item テーブルを復元する。
    pub lang_items: Vec<DiskLangItem>,
    /// このパッケージのインタフェースのハッシュ (Strict Version Hash)。
    ///
    /// 差分ビルドの中核。これが前回と一致していれば、
    /// このパッケージに依存しているパッケージは再ビルドしなくてよい。
    /// span やコメントは含まないので、実装だけの変更は伝播しない。
    /// 詳細は [`DepMetadata::compute_svh`]。
    pub svh: biwac_hash::Hash64,
    /// ビルド時の依存グラフ (推移閉包) の各パッケージの SVH。
    /// 鮮度判定と整合性検査に使う。
    pub dep_svhs: Vec<DiskDepSvh>,
    /// 依存パッケージのシンボルへの参照の一覧。
    /// [`DiskTyKind::ExternalDefined`] の sym_id がここを指す。
    pub ext_syms: Vec<DiskExternalSymbol>,
}

impl DepMetadata {
    /// HIR から `.biwameta` を組み立てる。
    ///
    /// `dep_svhs` はビルド時の依存グラフ (推移閉包) の各パッケージの SVH。
    /// 差分ビルドの鮮度判定に使うため、そのままファイルに記録する。
    /// `.biwameta` を組み立てる。
    ///
    /// 併せて [`SymbolIndexMap`] を返す。ここで決まったシンボルの採番が
    /// 「下流から見たこのパッケージの DefId」そのものであり、
    /// `.biwamir` を書くときに同じ採番が要るためである。
    pub fn new(
        hir: &biwac_hir::Hir,
        source_holder: &biwac_base::SourceHolder,
        interner: &biwac_base::IdentInterner,
        lang_item_table: &biwac_lang_item::LangItemTable,
        dep_svhs: &[(biwac_base::PackageId, biwac_hash::Hash64)],
    ) -> (Self, crate::SymbolIndexMap) {
        let mut symbol_index = crate::SymbolIndexMap::new();

        use biwac_base::ModPath;
        use biwac_hir::{AssocValDefKind, TyDefKind, ValDefKind};
        use biwac_span::{GenDefId, LocalGenDefId, TyDefId, ValDefId};
        use codec::DiskVec;
        use format::{
            DiskGenArg, DiskModData, DiskNativeTypeAliasData, DiskStructData, DiskStructMember,
        };

        // ====================================================
        // Phase 1: source file table (自パッケージのモジュール)
        // ====================================================
        let mut strings = StringTable::empty();
        let mut source_file_entries: Vec<DiskSourceInfo> = Vec::new();
        let mut mod_to_file_idx: HashMap<biwac_base::ModId, DiskFileIndex> = HashMap::new();

        let mut self_mods: Vec<(biwac_base::ModId, &biwac_base::ModSource)> = source_holder
            .mods
            .iter()
            .filter(|(_, ms)| ms.pkg_id.is_self())
            .map(|(mid, ms)| (*mid, ms))
            .collect();
        // ファイル名でソートして決定論的な順序にする
        self_mods.sort_by_key(|(_, ms)| ms.modu.file_name());

        for (mod_id, mod_src) in &self_mods {
            let path_str = strings.push(&mod_src.modu.file_name());
            let file_idx = DiskFileIndex(source_file_entries.len() as u32);
            source_file_entries.push(DiskSourceInfo {
                file_path: path_str,
            });
            mod_to_file_idx.insert(*mod_id, file_idx);
        }
        let source_files = SourceFileTable::new(source_file_entries);

        // ====================================================
        // Phase 2: シンボルインデックスの割り当て
        //   [struct類][native type alias類][assoc fn類][top-level fn類][mod類]
        // ====================================================

        // --- struct ---
        struct StructItem<'h> {
            def_id: TyDefId,
            def: &'h biwac_hir::StructDef,
        }
        let mut struct_items: Vec<StructItem> = hir
            .tys
            .iter()
            .filter(|(def_id, _)| def_id.pkg().is_self())
            .filter_map(|(def_id, def_impl)| {
                if let Some(TyDefKind::Struct(s)) = &def_impl.ty_content {
                    Some(StructItem {
                        def_id: *def_id,
                        def: s.as_ref(),
                    })
                } else {
                    None
                }
            })
            .collect();
        struct_items.sort_by_key(|i| i.def_id.value());

        // --- native type alias ---
        //
        // struct と同じくパッケージ外から参照される型定義である。
        // これを書き出さないと、依存側で `fn write(msg: String)` の引数型や
        // `struct Game { name: String }` のメンバ型が
        // ty_to_sym の引きに失敗してシンボル 0 番に退避してしまう。
        struct AliasItem<'h> {
            def_id: TyDefId,
            def: &'h biwac_hir::NativeTypeAliasDef,
        }
        let mut alias_items: Vec<AliasItem> = hir
            .tys
            .iter()
            .filter(|(def_id, _)| def_id.pkg().is_self())
            .filter_map(|(def_id, def_impl)| {
                if let Some(TyDefKind::NativeTypeAlias(a)) = &def_impl.ty_content {
                    Some(AliasItem {
                        def_id: *def_id,
                        def: a.as_ref(),
                    })
                } else {
                    None
                }
            })
            .collect();
        alias_items.sort_by_key(|i| i.def_id.value());

        let mut ty_to_sym: HashMap<TyDefId, DiskSymbolIndex> = HashMap::new();
        let mut next_idx = 0u32;
        for item in &struct_items {
            ty_to_sym.insert(item.def_id, DiskSymbolIndex(next_idx));
            symbol_index.insert_ty(item.def_id, next_idx);
            next_idx += 1;
        }
        for item in &alias_items {
            ty_to_sym.insert(item.def_id, DiskSymbolIndex(next_idx));
            symbol_index.insert_ty(item.def_id, next_idx);
            next_idx += 1;
        }

        // --- assoc fn (struct の impl に紐づく関数) ---
        struct AssocFnItem<'h> {
            val_def_id: ValDefId,
            name: &'h biwac_hir::Ident,
            signature: &'h biwac_hir::FnSignature,
            impl_genargs: &'h [(biwac_hir::Ident, LocalGenDefId)],
            parent_ty_def_id: TyDefId,
            /// impl の self 型。所属する型と、その impl 対象ジェネリック引数から組む。
            /// マングリングとメソッド解決の両方で「どの impl か」を決めるのに使う。
            impl_self_ty: biwac_hir::Ty,
        }
        let mut assoc_fn_items: Vec<AssocFnItem> = Vec::new();
        // 所属する型ではなく、**関連関数自身の ValDefId** でこのパッケージのものかを決める。
        //
        // struct と native type alias のどちらも impl block を持てる
        // (`impl String { fn concat(..) }`) が、それだけでは足りない。
        // `impl Int { fn sqrt(self) }` のようにプリミティブ型への実装もあり、
        // その場合 所属する型は組み込みパッケージのものになるが、
        // 関数自身はこのパッケージが定義している。
        // これを表に載せないと、`.biwamir` から参照できるシンボル索引が無くなる。
        //
        // (プリミティブ型への実装は将来 std に限定される予定で、
        //  そうなればこの経路は std のビルドでしか通らなくなる)
        let assoc_parents: Vec<(TyDefId, &biwac_hir::DefinedTyImpl)> =
            hir.tys.iter().map(|(id, i)| (*id, i)).collect();
        for (parent_ty_def_id, parent_impl) in &assoc_parents {
            for impl_list in parent_impl.vals.values() {
                for (val_def_id, pair) in &impl_list.vals {
                    if !val_def_id.pkg().is_self() {
                        continue;
                    }
                    let (name, sig, ig) = match &pair.val_content {
                        AssocValDefKind::Fn(f) => {
                            (&f.name, &f.signature, f.impl_genargs.as_slice())
                        }
                        AssocValDefKind::NativeFn(f) => {
                            (&f.name, &f.signature, f.impl_genargs.as_slice())
                        }
                    };
                    assoc_fn_items.push(AssocFnItem {
                        val_def_id: *val_def_id,
                        name,
                        signature: sig,
                        impl_genargs: ig,
                        parent_ty_def_id: *parent_ty_def_id,
                        impl_self_ty: impl_self_ty_of(*parent_ty_def_id, &pair.genargs, &name.span),
                    });
                }
            }
        }
        assoc_fn_items.sort_by_key(|i| i.val_def_id.value());

        let mut val_to_sym: HashMap<ValDefId, DiskSymbolIndex> = HashMap::new();
        for item in &assoc_fn_items {
            val_to_sym.insert(item.val_def_id, DiskSymbolIndex(next_idx));
            symbol_index.insert_val(item.val_def_id, next_idx);
            next_idx += 1;
        }

        // --- top-level fn ---
        struct TopFnItem<'h> {
            val_def_id: ValDefId,
            name: &'h biwac_hir::Ident,
            signature: &'h biwac_hir::FnSignature,
            impl_genargs: &'h [(biwac_hir::Ident, LocalGenDefId)],
        }
        let mut top_fn_items: Vec<TopFnItem> = hir
            .vals
            .iter()
            .filter(|(def_id, _)| def_id.pkg().is_self())
            .filter_map(|(def_id, val_kind)| {
                let (name, sig, ig) = match val_kind {
                    ValDefKind::Fn(f) => (&f.name, &f.signature, f.impl_genargs.as_slice()),
                    ValDefKind::Native(f) => (&f.name, &f.signature, f.impl_genargs.as_slice()),
                    ValDefKind::NovelScene(ns) => (&ns.name, &ns.signature, [].as_slice()),
                };
                Some(TopFnItem {
                    val_def_id: *def_id,
                    name,
                    signature: sig,
                    impl_genargs: ig,
                })
            })
            .collect();
        top_fn_items.sort_by_key(|i| i.val_def_id.value());

        for item in &top_fn_items {
            val_to_sym.insert(item.val_def_id, DiskSymbolIndex(next_idx));
            symbol_index.insert_val(item.val_def_id, next_idx);
            next_idx += 1;
        }

        // --- modules ---
        let mut mod_paths_sorted: Vec<ModPath> =
            self_mods.iter().map(|(_, ms)| ms.modu.clone()).collect();
        mod_paths_sorted.sort_by_key(|mp| mp.file_name());

        let mut mod_path_to_sym: HashMap<ModPath, DiskSymbolIndex> = HashMap::new();
        for mp in &mod_paths_sorted {
            mod_path_to_sym.insert(mp.clone(), DiskSymbolIndex(next_idx));
            symbol_index.insert_module(mp.clone(), next_idx);
            next_idx += 1;
        }

        let total_syms = next_idx as usize;

        // ルートモジュールのシンボルインデックス
        let root_path = if mod_path_to_sym.contains_key(&ModPath::Lib) {
            ModPath::Lib
        } else {
            ModPath::Main
        };
        let root_sym_idx = mod_path_to_sym.get(&root_path).map(|s| s.0).unwrap_or(0);

        // ====================================================
        // Phase 3: モジュール → 子シンボルリストの構築
        // ====================================================
        let mut mod_children: HashMap<ModPath, Vec<DiskSymbolIndex>> = HashMap::new();
        for mp in &mod_paths_sorted {
            mod_children.entry(mp.clone()).or_default();
        }

        // struct を所属モジュールに登録
        for item in &struct_items {
            let mod_id = item.def.name.span.module();
            if let Some(ms) = source_holder.mods.get(&mod_id)
                && let Some(&sym) = ty_to_sym.get(&item.def_id)
            {
                mod_children.entry(ms.modu.clone()).or_default().push(sym);
            }
        }

        // native type alias を所属モジュールに登録
        for item in &alias_items {
            let mod_id = item.def.name.span.module();
            if let Some(ms) = source_holder.mods.get(&mod_id)
                && let Some(&sym) = ty_to_sym.get(&item.def_id)
            {
                mod_children.entry(ms.modu.clone()).or_default().push(sym);
            }
        }

        // top-level fn を所属モジュールに登録
        for item in &top_fn_items {
            let mod_id = item.name.span.module();
            if let Some(ms) = source_holder.mods.get(&mod_id)
                && let Some(&sym) = val_to_sym.get(&item.val_def_id)
            {
                mod_children.entry(ms.modu.clone()).or_default().push(sym);
            }
        }

        // 子モジュールを親モジュールに登録
        for mp in &mod_paths_sorted {
            let parent = match mp {
                ModPath::Main | ModPath::Lib => None,
                ModPath::Mod(segs) if segs.len() == 1 => Some(root_path.clone()),
                ModPath::Mod(segs) => Some(ModPath::Mod(segs[..segs.len() - 1].to_vec())),
            };
            if let Some(parent_path) = parent
                && let Some(&child_sym) = mod_path_to_sym.get(mp)
            {
                mod_children.entry(parent_path).or_default().push(child_sym);
            }
        }

        // ====================================================
        // Phase 4: ボディのエンコード
        // ====================================================

        // シグニチャやメンバ型に依存パッケージの型が現れたら、
        // ここに登録して ExternalDefined として書き出す。
        let mut ext_syms = ExtSymBuilder::new();

        let mut body_builder = BodyBuilder::new();
        let mut sym_hdrs: Vec<DiskSymbolHeader> = Vec::with_capacity(total_syms);
        let mut cache: Vec<OnceLock<SymbolBody>> = Vec::with_capacity(total_syms);

        let push_body = |builder: &mut BodyBuilder,
                         hdrs: &mut Vec<DiskSymbolHeader>,
                         cache: &mut Vec<OnceLock<SymbolBody>>,
                         kind: DiskSymbolKind,
                         body: SymbolBody| {
            let offset = builder.push(&body);
            hdrs.push(DiskSymbolHeader {
                kind: kind as u32,
                vis: DiskVisibility::Public as u32,
                offset,
            });
            let lock = OnceLock::new();
            let _ = lock.set(body);
            cache.push(lock);
        };

        // --- struct ボディ ---
        for item in &struct_items {
            // GenDefId → 序数マップ (struct ジェネリクス)
            let gen_ord: HashMap<GenDefId, u32> = item
                .def
                .genargs
                .iter()
                .enumerate()
                .map(|(i, gid)| (*gid, i as u32))
                .collect();
            // 消費側は ext_gen_id(所属シンボル, 序数) で id を合成するので、
            // .biwamir も同じ組を書けるように記録しておく。
            let struct_sym = ty_to_sym.get(&item.def_id).map(|s| s.0).unwrap_or(0);
            for (gid, ord) in &gen_ord {
                symbol_index.insert_ty_genarg(*gid, struct_sym, *ord);
            }
            let empty_loc_gen: HashMap<LocalGenDefId, u32> = HashMap::new();

            let name_str = interner.get_str(&item.def.name.id).unwrap_or("");
            let disk_name = strings.push(name_str);
            let name_span = impl_to_disk_span(&item.def.name.span, &mod_to_file_idx);

            // def_raw_code: 名前 span のソーステキスト (HIR は struct 全体の span を持たない)
            let raw_code_text = source_holder
                .mods
                .get(&item.def.name.span.module())
                .map(|ms| &ms.src[item.def.name.span.begin()..item.def.name.span.end()])
                .unwrap_or("");
            let def_raw_code = strings.push(raw_code_text);

            // genargs (HIR は struct genarg の名前を持たないので空文字列)
            let empty_name = strings.push("");
            let disk_genargs = DiskVec(
                item.def
                    .genargs
                    .iter()
                    .map(|_| DiskGenArg {
                        name: empty_name,
                        name_span: DiskSpan {
                            file: DiskFileIndex(0),
                            begin: 0,
                            end: 0,
                        },
                    })
                    .collect(),
            );

            // members
            //
            // HIR 側は HashMap なので、**名前でソートしてから** 変換する。
            // 文字列テーブルへの push もこの順に起きるため、
            // .biwameta がビルドごとにバイト一致するようになる。
            // (以前は変換してから DiskStringOffset でソートしていたが、
            //  そのオフセット自体が HashMap の走査順で決まるので効いていなかった。
            //  差分ビルドはメタデータのハッシュを土台にするので、ここは崩せない)
            let mut members_sorted: Vec<(&str, &biwac_hir::Ty)> = item
                .def
                .members
                .iter()
                .map(|(ident, ty)| (interner.get_str(ident).unwrap_or(""), ty))
                .collect();
            members_sorted.sort_by_key(|(name, _)| *name);

            let members_vec: Vec<DiskStructMember> = members_sorted
                .into_iter()
                .map(|(mem_name, ty)| {
                    let mem_name_off = strings.push(mem_name);
                    // メンバ名 span は HIR に存在しないので型の span で代替
                    let mem_name_span = impl_to_disk_span(&ty.span, &mod_to_file_idx);
                    let disk_ty = impl_encode_ty(
                        ty,
                        &ty_to_sym,
                        &gen_ord,
                        &empty_loc_gen,
                        &mod_to_file_idx,
                        &mut ext_syms,
                    );
                    DiskStructMember {
                        name: mem_name_off,
                        name_span: mem_name_span,
                        ty: disk_ty,
                    }
                })
                .collect();

            // assoc symbols
            let assoc_syms: Vec<DiskSymbolIndex> = assoc_fn_items
                .iter()
                .filter(|af| af.parent_ty_def_id == item.def_id)
                .filter_map(|af| val_to_sym.get(&af.val_def_id).copied())
                .collect();

            let body = SymbolBody::Struct(DiskStructData {
                name: disk_name,
                name_span,
                def_raw_code,
                def_span: name_span,
                genargs: disk_genargs,
                members: DiskVec(members_vec),
                assoc_symbols: DiskVec(assoc_syms),
            });
            push_body(
                &mut body_builder,
                &mut sym_hdrs,
                &mut cache,
                DiskSymbolKind::Struct,
                body,
            );
        }

        // --- native type alias ボディ ---
        for item in &alias_items {
            let name_str = interner.get_str(&item.def.name.id).unwrap_or("");
            let disk_name = strings.push(name_str);
            let name_span = impl_to_disk_span(&item.def.name.span, &mod_to_file_idx);

            let disk_native = strings.push(&item.def.native);
            let native_span = impl_to_disk_span(&item.def.native_span, &mod_to_file_idx);

            // native type alias は genarg の名前を HIR に保持している
            let disk_genargs = DiskVec(
                item.def
                    .genargs
                    .iter()
                    .map(|g| DiskGenArg {
                        name: strings.push(interner.get_str(&g.id).unwrap_or("")),
                        name_span: impl_to_disk_span(&g.span, &mod_to_file_idx),
                    })
                    .collect(),
            );

            let assoc_syms: Vec<DiskSymbolIndex> = assoc_fn_items
                .iter()
                .filter(|af| af.parent_ty_def_id == item.def_id)
                .filter_map(|af| val_to_sym.get(&af.val_def_id).copied())
                .collect();

            let body = SymbolBody::NativeTypeAlias(DiskNativeTypeAliasData {
                name: disk_name,
                name_span,
                native: disk_native,
                native_span,
                genargs: disk_genargs,
                assoc_symbols: DiskVec(assoc_syms),
            });
            push_body(
                &mut body_builder,
                &mut sym_hdrs,
                &mut cache,
                DiskSymbolKind::NativeTypeAlias,
                body,
            );
        }

        // --- assoc fn ボディ ---
        for item in &assoc_fn_items {
            let fn_sym = val_to_sym.get(&item.val_def_id).map(|s| s.0).unwrap_or(0);
            let fn_data = impl_encode_fn_data(
                item.name,
                item.signature,
                item.impl_genargs,
                Some(&item.impl_self_ty),
                &ty_to_sym,
                &mod_to_file_idx,
                source_holder,
                &mut strings,
                interner,
                &mut ext_syms,
                fn_sym,
                &mut symbol_index,
            );
            let body = SymbolBody::Fn(fn_data);
            push_body(
                &mut body_builder,
                &mut sym_hdrs,
                &mut cache,
                DiskSymbolKind::Fn,
                body,
            );
        }

        // --- top-level fn ボディ ---
        for item in &top_fn_items {
            let fn_sym = val_to_sym.get(&item.val_def_id).map(|s| s.0).unwrap_or(0);
            let fn_data = impl_encode_fn_data(
                item.name,
                item.signature,
                item.impl_genargs,
                None,
                &ty_to_sym,
                &mod_to_file_idx,
                source_holder,
                &mut strings,
                interner,
                &mut ext_syms,
                fn_sym,
                &mut symbol_index,
            );
            let body = SymbolBody::Fn(fn_data);
            push_body(
                &mut body_builder,
                &mut sym_hdrs,
                &mut cache,
                DiskSymbolKind::Fn,
                body,
            );
        }

        // --- mod ボディ ---
        for mp in &mod_paths_sorted {
            let mod_name_str = match mp {
                ModPath::Lib => "lib",
                ModPath::Main => "main",
                ModPath::Mod(segs) => segs.last().map(|s| s.as_str()).unwrap_or(""),
            };
            let disk_mod_name = strings.push(mod_name_str);
            let children_vec = mod_children.get(mp).cloned().unwrap_or_default();
            let body = SymbolBody::Mod(DiskModData {
                name: disk_mod_name,
                name_span: DiskSpan {
                    file: DiskFileIndex(0),
                    begin: 0,
                    end: 0,
                },
                children: DiskVec(children_vec),
            });
            push_body(
                &mut body_builder,
                &mut sym_hdrs,
                &mut cache,
                DiskSymbolKind::Mod,
                body,
            );
        }

        // ====================================================
        // Phase 5: 組み立て
        // ====================================================
        let sym_body_bytes = body_builder.finish();
        let sym_bodies = LazyDiskVec::from_cache(sym_body_bytes, cache);

        // lang item: DefId をこのファイル内のシンボルインデックスに変換する。
        //
        // DepMetadata::new はシンボルを 0 から振り直すため、
        // 自パッケージでの PackageLocalDefId とは一致しない。
        // 一方で依存側がこのファイルを読むときは
        // sym_idx がそのまま PackageLocalDefId になる
        // (module_view::ExternalChildRef::as_ty_def_id を参照)。
        let mut lang_items: Vec<DiskLangItem> = lang_item_table
            .iter()
            .filter(|(_, def_id)| def_id.pkg().is_self())
            .filter_map(|(item, def_id)| {
                let sym_idx = match item.kind() {
                    biwac_lang_item::LangItemKind::Ty => {
                        ty_to_sym.get(&TyDefId::new(def_id)).copied()
                    }
                    biwac_lang_item::LangItemKind::Fn => {
                        val_to_sym.get(&ValDefId::new(def_id)).copied()
                    }
                }?;
                Some(DiskLangItem::new(item, sym_idx))
            })
            .collect();
        // 決定論的な順序にする
        lang_items.sort_by_key(|li| li.lang_item);

        // 依存の SVH は id 順に並べる (集合として比較するので順序を正準化しておく)。
        let mut dep_svh_entries: Vec<DiskDepSvh> = dep_svhs
            .iter()
            .map(|(pkg_id, svh)| DiskDepSvh {
                pkg: pkg_id.value(),
                svh: svh.as_u64(),
            })
            .collect();
        dep_svh_entries.sort_by_key(|d| d.pkg);

        let mut me = Self {
            sym_hdrs,
            sym_bodies,
            source_files,
            strings,
            root_sym_idx,
            lang_items,
            svh: biwac_hash::Hash64::ZERO,
            dep_svhs: dep_svh_entries,
            ext_syms: ext_syms.finish(),
        };
        // SVH は組み上がったシンボル表から計算する。
        // デコード側でも同じ関数で再計算できるので、必要なら検証もできる。
        me.svh = me.compute_svh();
        (me, symbol_index)
    }

    // --- Decode ---

    /// `.biwameta` を読む。
    ///
    /// 外部シンボル参照は `PackageId` の生値を持っており、
    /// その id は (name, version) から導出される安定ハッシュなので、
    /// ロード時に何かへ束縛し直す必要はない。
    pub fn decode_file(data: &[u8]) -> Result<Self, DepMetadataError> {
        let mut pos = 0;

        // ヘッダ (magic + version)
        if data.len() < 8 {
            return Err(DepMetadataError::UnexpectedEnd {
                needed: 8,
                available: data.len(),
            });
        }
        if &data[0..4] != BIWAC_DEPENDENCY_METADATA_MAGIC.as_ref() {
            return Err(DepMetadataError::InvalidMagic);
        }
        let version = u32::from_le_bytes(data[4..8].try_into().unwrap());
        if version != BIWAC_DEPENDENCY_METADATA_FORMAT_VERSION {
            return Err(DepMetadataError::InvalidVersion { got: version });
        }
        pos += 8;

        // sym_hdr_table: [count: u32][DiskSymbolHeader; count]
        let (sym_hdr_count, n) = u32::decode(&data[pos..])?;
        pos += n;
        let mut sym_hdrs = Vec::with_capacity(sym_hdr_count as usize);
        for _ in 0..sym_hdr_count {
            let (hdr, n) = DiskSymbolHeader::decode(&data[pos..])?;
            pos += n;
            sym_hdrs.push(hdr);
        }

        // root_sym_idx
        let (root_sym_idx, n) = u32::decode(&data[pos..])?;
        pos += n;

        // sym_body_table: [total_bytes: u32][body_data: total_bytes B]
        let (sym_body_total, n) = u32::decode(&data[pos..])?;
        pos += n;
        let body_end = pos + sym_body_total as usize;
        if body_end > data.len() {
            return Err(DepMetadataError::UnexpectedEnd {
                needed: body_end,
                available: data.len(),
            });
        }
        let sym_body_bytes = data[pos..body_end].to_vec();
        pos = body_end;
        let sym_bodies = LazyDiskVec::new(sym_body_bytes, sym_hdrs.len());

        // source_file_table: [count: u32][DiskSourceInfo; count]
        let (file_count, n) = u32::decode(&data[pos..])?;
        pos += n;
        let mut source_file_entries = Vec::with_capacity(file_count as usize);
        for _ in 0..file_count {
            let (info, n) = DiskSourceInfo::decode(&data[pos..])?;
            pos += n;
            source_file_entries.push(info);
        }
        let source_files = SourceFileTable::new(source_file_entries);

        // string_table: [total_bytes: u32][data: total_bytes B]
        let (str_total, n) = u32::decode(&data[pos..])?;
        pos += n;
        let str_end = pos + str_total as usize;
        if str_end > data.len() {
            return Err(DepMetadataError::UnexpectedEnd {
                needed: str_end,
                available: data.len(),
            });
        }
        let strings = StringTable::new(data[pos..str_end].to_vec());
        pos = str_end;

        // lang_item_table: [count: u32][DiskLangItem; count]
        let (lang_item_count, n) = u32::decode(&data[pos..])?;
        pos += n;
        let mut lang_items = Vec::with_capacity(lang_item_count as usize);
        for _ in 0..lang_item_count {
            let (li, n) = DiskLangItem::decode(&data[pos..])?;
            pos += n;
            lang_items.push(li);
        }

        // svh
        let (svh, n) = u64::decode(&data[pos..])?;
        pos += n;

        // dep_svh_table: [count: u32][DiskDepSvh; count]
        let (dep_svh_count, n) = u32::decode(&data[pos..])?;
        pos += n;
        let mut dep_svhs = Vec::with_capacity(dep_svh_count as usize);
        for _ in 0..dep_svh_count {
            let (dep, n) = DiskDepSvh::decode(&data[pos..])?;
            pos += n;
            dep_svhs.push(dep);
        }

        // ext_sym_table: [count: u32][DiskExternalSymbol; count]
        let (ext_sym_count, n) = u32::decode(&data[pos..])?;
        pos += n;
        let mut ext_syms = Vec::with_capacity(ext_sym_count as usize);
        for _ in 0..ext_sym_count {
            let (e, n) = DiskExternalSymbol::decode(&data[pos..])?;
            pos += n;
            ext_syms.push(e);
        }

        Ok(Self {
            sym_hdrs,
            sym_bodies,
            source_files,
            strings,
            root_sym_idx,
            lang_items,
            svh: biwac_hash::Hash64::from_u64(svh),
            dep_svhs,
            ext_syms,
        })
    }

    // --- Encode ---

    pub fn encode_file(&self) -> Vec<u8> {
        let mut buf = Vec::new();

        // ヘッダ
        buf.extend_from_slice(BIWAC_DEPENDENCY_METADATA_MAGIC.as_ref());
        BIWAC_DEPENDENCY_METADATA_FORMAT_VERSION.encode(&mut buf);

        // sym_hdr_table
        (self.sym_hdrs.len() as u32).encode(&mut buf);
        for hdr in &self.sym_hdrs {
            hdr.encode(&mut buf);
        }

        // root_sym_idx
        self.root_sym_idx.encode(&mut buf);

        // sym
        let mut builder = BodyBuilder::new();
        for b in &self.sym_bodies.cache {
            builder.push(b.get().unwrap());
        }
        let sym_body_bytes = builder.finish();

        // sym_body_table
        (sym_body_bytes.len() as u32).encode(&mut buf);
        buf.extend_from_slice(&sym_body_bytes);

        // source_file_table
        (self.source_files.len() as u32).encode(&mut buf);
        for info in self.source_files.entries() {
            info.encode(&mut buf);
        }

        // string_table
        let str_data = self.strings.data();
        (str_data.len() as u32).encode(&mut buf);
        buf.extend_from_slice(str_data);

        // lang_item_table
        (self.lang_items.len() as u32).encode(&mut buf);
        for li in &self.lang_items {
            li.encode(&mut buf);
        }

        // svh
        self.svh.as_u64().encode(&mut buf);

        // dep_svh_table
        (self.dep_svhs.len() as u32).encode(&mut buf);
        for dep in &self.dep_svhs {
            dep.encode(&mut buf);
        }

        // ext_sym_table
        (self.ext_syms.len() as u32).encode(&mut buf);
        for e in &self.ext_syms {
            e.encode(&mut buf);
        }

        buf
    }

    /// 外部シンボル表のエントリ 1 件を `(PackageId, シンボルインデックス)` に解決する。
    fn resolve_ext_sym(&self, ext_sym_idx: u32) -> Option<(biwac_base::PackageId, u32)> {
        let entry = self.ext_syms.get(ext_sym_idx as usize)?;
        Some((biwac_base::PackageId::new(entry.pkg), entry.sym.0))
    }

    /// ビルド時に見た依存パッケージの `(PackageId, Svh)` を列挙する。
    pub fn dep_svhs(
        &self,
    ) -> impl Iterator<Item = (biwac_base::PackageId, biwac_hash::Hash64)> + '_ {
        self.dep_svhs.iter().map(|d| {
            (
                biwac_base::PackageId::new(d.pkg),
                biwac_hash::Hash64::from_u64(d.svh),
            )
        })
    }

    // ============================================================
    // SVH (Strict Version Hash)
    // ============================================================

    /// このパッケージの **インタフェース** のハッシュを計算する。
    ///
    /// 差分ビルドの中核。あるパッケージの SVH が前回と一致していれば、
    /// そのパッケージに依存しているパッケージを再ビルドする必要はない。
    /// rustc の `Svh` (`rustc_data_structures/src/svh.rs`) に相当する。
    ///
    /// **含めるもの**: シンボルの並びと種別・名前・所属モジュール・シグニチャ・
    /// メンバ・native 本体・lang item、そして **シンボルインデックス**。
    ///
    /// インデックスを含めるのが要点である。
    /// 外部パッケージからの参照は `(PackageId, シンボルインデックス)` の形で行われるので、
    /// 採番がずれたら参照側は作り直さなければならない。
    /// 逆に、採番も名前もシグニチャも同一なら、参照側は一切影響を受けない。
    ///
    /// **含めないもの**:
    /// - span のバイトオフセットと `def_raw_code`
    ///   (コメントや空行を足しただけで依存先に再ビルドが波及しないように)
    /// - 文字列テーブルのオフセット (文字列そのものを混ぜる)
    /// - 依存の SVH 表
    ///   (含めると伝播が打ち切れなくなる。std を再ビルドしても、
    ///    このパッケージが参照している std のシンボルが動いていなければ
    ///    ここは不変であってほしい)
    ///
    /// 所属モジュールのファイル名は含める。マングル名の一部になるからである
    /// ([`Self::symbol_mangling_info`] を参照)。
    pub fn compute_svh(&self) -> biwac_hash::Hash64 {
        use biwac_hash::StableHasher64;

        let mut h = StableHasher64::new();
        h.write_str("biwameta-svh");
        h.write_u32(BIWAC_DEPENDENCY_METADATA_FORMAT_VERSION);
        h.write_u32(self.root_sym_idx);

        h.write_usize(self.sym_hdrs.len());
        for sym_idx in 0..self.sym_hdrs.len() {
            h.write_usize(sym_idx);

            let Ok(body) = self.get_symbol_body(sym_idx) else {
                // ヘッダはあるのにボディが壊れている。
                // 「読めなかった」という事実自体を混ぜて、正常なものと区別する。
                h.write_str("<undecodable>");
                continue;
            };

            match body {
                SymbolBody::Mod(d) => {
                    h.write_str("mod");
                    self.svh_name(&mut h, d.name, &d.name_span);
                    h.write_usize(d.children.0.len());
                    for c in &d.children.0 {
                        h.write_u32(c.0);
                    }
                }
                SymbolBody::Struct(d) => {
                    h.write_str("struct");
                    self.svh_name(&mut h, d.name, &d.name_span);
                    self.svh_genargs(&mut h, &d.genargs.0);

                    // メンバは名前順に正準化する。
                    // .biwameta 側も名前順に書いているので実際には既に整列しているが、
                    // SVH は表現ではなく意味のハッシュなので、ここでも保証しておく。
                    let mut members: Vec<(&str, &DiskTy)> = d
                        .members
                        .0
                        .iter()
                        .map(|m| (self.get_str(m.name).unwrap_or(""), &m.ty))
                        .collect();
                    members.sort_by_key(|(name, _)| *name);
                    h.write_usize(members.len());
                    for (name, ty) in members {
                        h.write_str(name);
                        self.svh_ty(&mut h, ty);
                    }

                    h.write_usize(d.assoc_symbols.0.len());
                    for a in &d.assoc_symbols.0 {
                        h.write_u32(a.0);
                    }
                }
                SymbolBody::NativeTypeAlias(d) => {
                    h.write_str("native-type-alias");
                    self.svh_name(&mut h, d.name, &d.name_span);
                    // native 本体はターゲット言語にそのまま出るのでインタフェースである。
                    h.write_str(self.get_str(d.native).unwrap_or(""));
                    self.svh_genargs(&mut h, &d.genargs.0);
                    h.write_usize(d.assoc_symbols.0.len());
                    for a in &d.assoc_symbols.0 {
                        h.write_u32(a.0);
                    }
                }
                SymbolBody::Fn(d) => {
                    h.write_str("fn");
                    self.svh_name(&mut h, d.name, &d.name_span);
                    self.svh_genargs(&mut h, &d.genargs.0);
                    h.write_usize(d.args.0.len());
                    for a in &d.args.0 {
                        h.write_str(self.get_str(a.name).unwrap_or(""));
                        self.svh_ty(&mut h, &a.ty);
                    }
                    self.svh_ty(&mut h, &d.rty);
                    h.write_usize(d.impl_self_ty.0.len());
                    for t in &d.impl_self_ty.0 {
                        self.svh_ty(&mut h, t);
                    }
                }
            }
        }

        // lang item は依存側のコンパイラが直接引くのでインタフェースである。
        // DepMetadata::new が discriminant 順にソート済み。
        h.write_str("lang-items");
        h.write_usize(self.lang_items.len());
        for li in &self.lang_items {
            h.write_u32(li.lang_item);
            h.write_u32(li.sym_idx.0);
        }

        h.finish()
    }

    /// シンボル名と、それが属するモジュールのファイル名を混ぜる。
    ///
    /// span のバイトオフセットは混ぜない。
    /// モジュールのファイル名だけはマングル名に出るので必要になる。
    fn svh_name(
        &self,
        h: &mut biwac_hash::StableHasher64,
        name: format::DiskStringOffset,
        span: &DiskSpan,
    ) {
        h.write_str(self.get_str(name).unwrap_or(""));
        let modu = self
            .source_files
            .get(span.file.0)
            .ok()
            .and_then(|f| self.get_str(f.file_path).ok())
            .unwrap_or("");
        h.write_str(modu);
    }

    fn svh_genargs(&self, h: &mut biwac_hash::StableHasher64, genargs: &[format::DiskGenArg]) {
        h.write_usize(genargs.len());
        for g in genargs {
            h.write_str(self.get_str(g.name).unwrap_or(""));
        }
    }

    fn svh_ty(&self, h: &mut biwac_hash::StableHasher64, ty: &DiskTy) {
        h.write_u32(ty.hdr.kind);
        match DiskTyKind::try_from(ty.hdr.kind) {
            // ExternalDefined の sym_id は ext_sym_table の索引なので、
            // 表の詰め順に左右されないよう解決してから混ぜる。
            Ok(DiskTyKind::ExternalDefined) => match self.resolve_ext_sym(ty.hdr.sym_id.0) {
                Some((pkg_id, sym_idx)) => {
                    h.write_u32(pkg_id.value());
                    h.write_u32(sym_idx);
                }
                None => h.write_str("<unresolved-external>"),
            },
            // Defined は自ファイルの索引、Gen/LocGen は序数、Fn は引数の数。
            // どれもそのまま混ぜてよい。
            _ => h.write_u32(ty.hdr.sym_id.0),
        }
        h.write_usize(ty.genargs.len());
        for g in &ty.genargs {
            self.svh_ty(h, g);
        }
    }

    pub fn get_symbol_body(&self, sym_idx: usize) -> Result<&SymbolBody, DepMetadataError> {
        if sym_idx >= self.sym_hdrs.len() {
            return Err(DepMetadataError::SymbolIndexOutOfBounds {
                index: sym_idx as u32,
                sym_count: self.sym_hdrs.len() as u32,
            });
        }
        self.sym_bodies.get(sym_idx, &self.sym_hdrs[sym_idx])
    }

    pub fn get_str(&self, offset: format::DiskStringOffset) -> Result<&str, DepMetadataError> {
        self.strings.get(offset)
    }

    /// 外部パッケージのシンボル 1 つについて、名前とモジュールパスを返す。
    ///
    /// codegen のシンボル名マングリングに使う。
    /// HIR 側に復元したシンボルの span はダミーであり
    /// (get_ext_ty_impl / get_ext_val_kind を参照)、
    /// モジュールを特定できないため、メタデータから直接引く必要がある。
    pub fn symbol_mangling_info(&self, sym_idx: u32) -> Option<(&str, biwac_base::ModPath)> {
        let body = self.get_symbol_body(sym_idx as usize).ok()?;

        let (name, name_span) = match body {
            SymbolBody::Struct(d) => (d.name, d.name_span),
            SymbolBody::Fn(d) => (d.name, d.name_span),
            SymbolBody::NativeTypeAlias(d) => (d.name, d.name_span),
            SymbolBody::Mod(d) => (d.name, d.name_span),
        };

        let name = self.get_str(name).ok()?;
        let file = self.source_files.get(name_span.file.0).ok()?;
        let modu = biwac_base::ModPath::from_file_name(self.get_str(file.file_path).ok()?)?;

        Some((name, modu))
    }

    /// このパッケージが定義した lang item を `(LangItem, DefId)` として列挙する。
    ///
    /// `pkg_id` は依存側が割り当てたパッケージ ID。
    /// メタデータ上の sym_idx は消費側から見た `PackageLocalDefId` と一致するため
    /// (module_view::ExternalChildRef::as_ty_def_id と同じ規約)、
    /// そのまま DefId を組み立てられる。
    ///
    /// フォーマットバージョンが一致していれば未知の discriminant は現れないが、
    /// 現れた場合はその項目を黙って飛ばす。
    pub fn lang_items(
        &self,
        pkg_id: biwac_base::PackageId,
    ) -> impl Iterator<Item = (biwac_lang_item::LangItem, biwac_span::DefId)> + '_ {
        self.lang_items.iter().filter_map(move |li| {
            let item = li.lang_item()?;
            let def_id =
                biwac_span::DefId::new(pkg_id, biwac_span::PackageLocalDefId::new(li.sym_idx.0));
            Some((item, def_id))
        })
    }

    /// 外部パッケージの fn シンボル1つを ValDefKind に変換する。
    /// TyCtx の遅延ロードから呼ばれる。sym_idx は DefId.local_idx() と一致する。
    pub fn get_ext_val_kind(
        &self,
        sym_idx: u32,
        pkg_id: biwac_base::PackageId,
        interner: &mut biwac_base::IdentInterner,
    ) -> Option<biwac_hir::FnSignature> {
        let body = self.get_symbol_body(sym_idx as usize).ok()?;
        let SymbolBody::Fn(fn_data) = body else {
            return None;
        };
        let sig = self.impl_disk_fn_to_signature(sym_idx, fn_data, pkg_id, interner);
        Some(sig)
    }

    /// 関連関数・メソッドの impl self 型を復元する。
    /// トップレベル関数なら `None`。
    fn impl_decode_impl_self_ty(
        &self,
        fn_data: &format::DiskFnData,
        fn_sym_idx: u32,
        pkg_id: biwac_base::PackageId,
    ) -> Option<biwac_hir::Ty> {
        let disk_ty = fn_data.impl_self_ty.0.first()?;

        // self 型に現れる LocGen は impl ブロックのジェネリック引数であり、
        // fn の combined genargs の先頭に並んでいるので、
        // その序数解決に fn 自身のシンボルインデックスを使う。
        Some(self.impl_disk_ty_to_ty(disk_ty, pkg_id, None, Some(fn_sym_idx)))
    }

    /// 外部パッケージの関連関数・メソッドの impl self 型を返す。
    /// codegen のシンボル名マングリングから呼ばれる。
    pub fn assoc_impl_self_ty(
        &self,
        sym_idx: u32,
        pkg_id: biwac_base::PackageId,
    ) -> Option<biwac_hir::Ty> {
        let SymbolBody::Fn(fn_data) = self.get_symbol_body(sym_idx as usize).ok()? else {
            return None;
        };

        self.impl_decode_impl_self_ty(fn_data, sym_idx, pkg_id)
    }

    /// 外部パッケージの型に紐づく assoc fn 群を vals マップに復元する。
    ///
    /// struct と native type alias で共通の処理。
    fn impl_load_ext_assoc_vals(
        &self,
        assoc_symbols: &[DiskSymbolIndex],
        pkg_id: biwac_base::PackageId,
        interner: &mut biwac_base::IdentInterner,
    ) -> std::collections::HashMap<biwac_base::InternedIdent, biwac_hir::TyValImplList> {
        use biwac_hir::{
            AssocValDefKind, Ident, NativeFnDef, TyValImplGenargsContentPair, TyValImplList,
        };
        use biwac_span::{DefId, LocalGenDefId, PackageLocalDefId, Span, ValDefId};

        let mut vals: std::collections::HashMap<biwac_base::InternedIdent, TyValImplList> =
            std::collections::HashMap::new();

        // assoc fns を vals に登録する。
        // impl の対象ジェネリック引数は DiskFnData::impl_self_ty に記録済みなので、
        // `impl Foo[Int] { ... }` のように特殊化された impl も正確に復元できる。
        for &assoc_sym_idx_disk in assoc_symbols {
            let assoc_sym_idx = assoc_sym_idx_disk.0;
            let Ok(assoc_body) = self.get_symbol_body(assoc_sym_idx as usize) else {
                continue;
            };
            let SymbolBody::Fn(fn_data) = assoc_body else {
                continue;
            };

            let val_def_id =
                ValDefId::new(DefId::new(pkg_id, PackageLocalDefId::new(assoc_sym_idx)));

            // impl 対象のジェネリック引数は self 型の genargs そのもの。
            let impl_self_ty = self.impl_decode_impl_self_ty(fn_data, assoc_sym_idx, pkg_id);
            let impl_genarg_pattern: Vec<biwac_hir::Ty> = impl_self_ty
                .as_ref()
                .map(|ty| match &ty.kind {
                    biwac_hir::TyKind::Defined(dt) => dt.genargs.clone(),
                    // プリミティブ型の impl はジェネリック引数を取らない
                    _ => Vec::new(),
                })
                .unwrap_or_default();
            let impl_genarg_count = impl_genarg_pattern.len();

            let impl_block_genargs: std::collections::HashMap<_, _> = (0..impl_genarg_count)
                .map(|i| {
                    let name_str = fn_data
                        .genargs
                        .0
                        .get(i)
                        .and_then(|g| self.get_str(g.name).ok())
                        .unwrap_or("");
                    let name_id = interner.get_or_insert(name_str);
                    let lgid = LocalGenDefId::new(DefId::new(
                        pkg_id,
                        PackageLocalDefId::new(ext_loc_gen_id(assoc_sym_idx, i as u32)),
                    ));
                    (name_id, (lgid, Span::dummy()))
                })
                .collect();

            // val_content は型推論中に参照されないが構造体に必要なためダミーで埋める
            let fn_name_str = self.get_str(fn_data.name).unwrap_or("");
            let fn_name_id = interner.get_or_insert(fn_name_str);
            let placeholder_sig =
                self.impl_disk_fn_to_signature(assoc_sym_idx, fn_data, pkg_id, interner);
            let placeholder = NativeFnDef {
                name: Ident {
                    id: fn_name_id,
                    span: Span::dummy(),
                },
                signature: placeholder_sig,
                native_body: String::new(),
                native_span: Span::dummy(),
                span: Span::dummy(),
                impl_genargs: vec![],
            };

            let pair = TyValImplGenargsContentPair {
                impl_block_genargs,
                genargs: impl_genarg_pattern,
                val_content: AssocValDefKind::NativeFn(Box::new(placeholder)),
            };

            vals.entry(fn_name_id)
                .or_insert_with(|| TyValImplList {
                    vals: std::collections::HashMap::new(),
                })
                .vals
                .insert(val_def_id, pair);
        }

        vals
    }

    /// 外部パッケージの型シンボル1つを DefinedTyImpl に変換する。
    /// TyCtx の遅延ロードから呼ばれる。
    /// struct はメンバ型を、native type alias は native 本体を、
    /// どちらも assoc fns を含む。
    pub fn get_ext_ty_impl(
        &self,
        ty_sym_idx: u32,
        pkg_id: biwac_base::PackageId,
        interner: &mut biwac_base::IdentInterner,
    ) -> Option<biwac_hir::DefinedTyImpl> {
        match self.get_symbol_body(ty_sym_idx as usize).ok()? {
            SymbolBody::Struct(_) => self.impl_get_ext_struct_ty_impl(ty_sym_idx, pkg_id, interner),
            SymbolBody::NativeTypeAlias(_) => {
                self.impl_get_ext_native_alias_ty_impl(ty_sym_idx, pkg_id, interner)
            }
            _ => None,
        }
    }

    /// native type alias (`type String = {{ string }};`) を復元する。
    /// メンバを持たないため、名前・genarg 名・native 本体と assoc fns だけを持つ。
    fn impl_get_ext_native_alias_ty_impl(
        &self,
        alias_sym_idx: u32,
        pkg_id: biwac_base::PackageId,
        interner: &mut biwac_base::IdentInterner,
    ) -> Option<biwac_hir::DefinedTyImpl> {
        use biwac_hir::{DefinedTyImpl, Ident, NativeTypeAliasDef, TyDefKind};
        use biwac_span::Span;

        let body = self.get_symbol_body(alias_sym_idx as usize).ok()?;
        let SymbolBody::NativeTypeAlias(alias_data) = body else {
            return None;
        };

        let name_id = interner.get_or_insert(self.get_str(alias_data.name).unwrap_or(""));
        let genargs: Vec<Ident> = alias_data
            .genargs
            .0
            .iter()
            .map(|g| Ident {
                id: interner.get_or_insert(self.get_str(g.name).unwrap_or("")),
                span: Span::dummy(),
            })
            .collect();

        let vals = self.impl_load_ext_assoc_vals(&alias_data.assoc_symbols.0, pkg_id, interner);

        let alias_def = NativeTypeAliasDef {
            name: Ident {
                id: name_id,
                span: Span::dummy(),
            },
            genargs,
            native: self.get_str(alias_data.native).unwrap_or("").to_string(),
            native_span: Span::dummy(),
        };

        Some(DefinedTyImpl {
            ty_content: Some(TyDefKind::NativeTypeAlias(Box::new(alias_def))),
            vals,
        })
    }

    fn impl_get_ext_struct_ty_impl(
        &self,
        struct_sym_idx: u32,
        pkg_id: biwac_base::PackageId,
        interner: &mut biwac_base::IdentInterner,
    ) -> Option<biwac_hir::DefinedTyImpl> {
        use biwac_hir::{DefinedTyImpl, Ident, StructDef, TyDefKind};
        use biwac_span::{DefId, GenDefId, PackageLocalDefId, Span};

        let body = self.get_symbol_body(struct_sym_idx as usize).ok()?;
        let SymbolBody::Struct(struct_data) = body else {
            return None;
        };

        // struct のジェネリクス引数 GenDefId を割り当て
        let genargs: Vec<GenDefId> = (0..struct_data.genargs.0.len())
            .map(|i| {
                GenDefId::new(DefId::new(
                    pkg_id,
                    PackageLocalDefId::new(ext_gen_id(struct_sym_idx, i as u32)),
                ))
            })
            .collect();

        // メンバを変換
        let mut members = std::collections::HashMap::new();
        for m in &struct_data.members.0 {
            let name_str = self.get_str(m.name).unwrap_or("");
            let name_id = interner.get_or_insert(name_str);
            let ty = self.impl_disk_ty_to_ty(&m.ty, pkg_id, Some(struct_sym_idx), None);
            members.insert(name_id, ty);
        }

        let struct_name_str = self.get_str(struct_data.name).unwrap_or("");
        let struct_name_id = interner.get_or_insert(struct_name_str);
        let struct_def = StructDef {
            name: Ident {
                id: struct_name_id,
                span: Span::dummy(),
            },
            members,
            genargs,
        };

        let vals = self.impl_load_ext_assoc_vals(&struct_data.assoc_symbols.0, pkg_id, interner);

        Some(DefinedTyImpl {
            ty_content: Some(TyDefKind::Struct(Box::new(struct_def))),
            vals,
        })
    }

    /// DiskFnData → FnSignature 変換。fn_sym_idx を LocGenDefId の名前空間として使用する。
    fn impl_disk_fn_to_signature(
        &self,
        fn_sym_idx: u32,
        fn_data: &format::DiskFnData,
        pkg_id: biwac_base::PackageId,
        interner: &mut biwac_base::IdentInterner,
    ) -> biwac_hir::FnSignature {
        use biwac_hir::{FnArgDecl, FnSignature, Ident};
        use biwac_span::{DefId, LocalGenDefId, PackageLocalDefId, Span, VarId};

        // disk のジェネリクス引数 → LocalGenDefId のマップ (ordinal → lgid)
        let genargs: Vec<(biwac_hir::Ident, biwac_span::LocalGenDefId)> = fn_data
            .genargs
            .0
            .iter()
            .enumerate()
            .map(|(i, g)| {
                let name_str = self.get_str(g.name).unwrap_or("");
                let name_id = interner.get_or_insert(name_str);
                let lgid = LocalGenDefId::new(DefId::new(
                    pkg_id,
                    PackageLocalDefId::new(ext_loc_gen_id(fn_sym_idx, i as u32)),
                ));
                (
                    Ident {
                        id: name_id,
                        span: Span::dummy(),
                    },
                    lgid,
                )
            })
            .collect();

        // 引数を変換 (self 引数は FnSignature.args には含まない)
        let args: Vec<FnArgDecl> = fn_data
            .args
            .0
            .iter()
            .enumerate()
            .map(|(i, a)| {
                let name_str = self.get_str(a.name).unwrap_or("");
                let name_id = interner.get_or_insert(name_str);
                let ty = self.impl_disk_ty_to_ty(&a.ty, pkg_id, None, Some(fn_sym_idx));
                FnArgDecl {
                    id: Ident {
                        id: name_id,
                        span: Span::dummy(),
                    },
                    ty,
                    // var_id は外部 fn の型推論では使われないためダミー値を使用
                    var_id: VarId::new(i as u32 + 1),
                }
            })
            .collect();

        let rty = self.impl_disk_ty_to_ty(&fn_data.rty, pkg_id, None, Some(fn_sym_idx));

        FnSignature {
            args,
            // メソッドなら impl の self 型を入れる。トップレベル関数なら None になる。
            //
            // メソッド呼び出しの単一化はレシーバを第 1 引数として含めるので、
            // ここが空だと impl ブロックのジェネリック引数がレシーバから決まらず、
            // 戻り値が `Self` のメソッドで型変数が解けないまま残る
            // (単相化に必要な割り当てが記録されず、MIR の符号化まで漏れる)。
            self_ty: self.impl_decode_impl_self_ty(fn_data, fn_sym_idx, pkg_id),
            rty,
            genargs,
            span: Span::dummy(),
        }
    }

    /// DiskTy → Ty 変換。
    /// struct_gen_sym_idx: struct メンバ型の Gen 解決に使うシンボルインデックス
    /// fn_loc_gen_sym_idx: fn シグニチャの LocGen 解決に使うシンボルインデックス
    fn impl_disk_ty_to_ty(
        &self,
        disk_ty: &format::DiskTy,
        pkg_id: biwac_base::PackageId,
        struct_gen_sym_idx: Option<u32>,
        fn_loc_gen_sym_idx: Option<u32>,
    ) -> biwac_hir::Ty {
        use biwac_hir::{DefinedTy, FnTy, TyKind};
        use biwac_span::{DefId, GenDefId, LocalGenDefId, PackageLocalDefId, Span, TyDefId};

        let kind = match DiskTyKind::try_from(disk_ty.hdr.kind) {
            Ok(k) => k,
            Err(_) => return biwac_hir::Ty::new(TyKind::Void, Span::dummy()),
        };

        let ty_kind = match kind {
            DiskTyKind::Int => TyKind::Int,
            DiskTyKind::Float => TyKind::Float,
            DiskTyKind::Bool => TyKind::Bool,
            DiskTyKind::Void => TyKind::Void,
            DiskTyKind::Defined => {
                let sym_idx = disk_ty.hdr.sym_id.0;
                let genargs = disk_ty
                    .genargs
                    .iter()
                    .map(|g| {
                        self.impl_disk_ty_to_ty(g, pkg_id, struct_gen_sym_idx, fn_loc_gen_sym_idx)
                    })
                    .collect();
                TyKind::Defined(DefinedTy {
                    def_id: TyDefId::new(DefId::new(pkg_id, PackageLocalDefId::new(sym_idx))),
                    genargs,
                })
            }
            DiskTyKind::ExternalDefined => {
                // sym_id は ext_sym_table の索引。
                // そこから (依存パッケージ, 相手のシンボルインデックス) を引く。
                //
                // ジェネリック引数は「このファイルの」型なので、
                // 序数解決のコンテキスト (struct_gen_sym_idx / fn_loc_gen_sym_idx) は
                // そのまま引き継ぐ。切り替えるのは Defined の所属パッケージだけである。
                let genargs = disk_ty
                    .genargs
                    .iter()
                    .map(|g| {
                        self.impl_disk_ty_to_ty(g, pkg_id, struct_gen_sym_idx, fn_loc_gen_sym_idx)
                    })
                    .collect();

                let Some((ext_pkg_id, ext_sym_idx)) = self.resolve_ext_sym(disk_ty.hdr.sym_id.0)
                else {
                    // decode 時に束縛済みなのでここは通らない。
                    debug_assert!(false, "compiler bug: unresolved external symbol reference");
                    return biwac_hir::Ty::new(TyKind::Void, Span::dummy());
                };

                TyKind::Defined(DefinedTy {
                    def_id: TyDefId::new(DefId::new(
                        ext_pkg_id,
                        PackageLocalDefId::new(ext_sym_idx),
                    )),
                    genargs,
                })
            }
            DiskTyKind::Gen => {
                // sym_id = struct genargs 内の ordinal (0-base)
                let ordinal = disk_ty.hdr.sym_id.0;
                let struct_sym_idx = struct_gen_sym_idx.unwrap_or(0);
                TyKind::Gen(GenDefId::new(DefId::new(
                    pkg_id,
                    PackageLocalDefId::new(ext_gen_id(struct_sym_idx, ordinal)),
                )))
            }
            DiskTyKind::LocGen => {
                // sym_id = fn の combined genargs 内の ordinal (0-base)
                let ordinal = disk_ty.hdr.sym_id.0;
                let fn_sym_idx = fn_loc_gen_sym_idx.unwrap_or(0);
                TyKind::LocGen(LocalGenDefId::new(DefId::new(
                    pkg_id,
                    PackageLocalDefId::new(ext_loc_gen_id(fn_sym_idx, ordinal)),
                )))
            }
            DiskTyKind::Fn => {
                // sym_id = 引数の数; genargs = [arg0, ..., argN, rty]
                let arg_count = disk_ty.hdr.sym_id.0 as usize;
                if arg_count + 1 > disk_ty.genargs.len() {
                    return biwac_hir::Ty::new(TyKind::Void, Span::dummy());
                }
                let args = disk_ty.genargs[..arg_count]
                    .iter()
                    .map(|g| {
                        self.impl_disk_ty_to_ty(g, pkg_id, struct_gen_sym_idx, fn_loc_gen_sym_idx)
                    })
                    .collect();
                let rty = self.impl_disk_ty_to_ty(
                    &disk_ty.genargs[arg_count],
                    pkg_id,
                    struct_gen_sym_idx,
                    fn_loc_gen_sym_idx,
                );
                TyKind::Fn(FnTy {
                    args,
                    rty: Box::new(rty),
                    genargs: vec![],
                })
            }
        };

        biwac_hir::Ty::new(ty_kind, Span::dummy())
    }
}

/// 外部パッケージの struct ジェネリクス引数 ID エンコード。
/// GenDefId/LocalGenDefId の PackageLocal 部分として使用する。
/// 1シンボルあたり最大 1024 genargs、最大 4M シンボルをサポート。
fn ext_gen_id(struct_sym_idx: u32, ordinal: u32) -> u32 {
    (struct_sym_idx << 10) | ordinal
}

/// [`ext_gen_id`] / [`ext_loc_gen_id`] の合成規則。
///
/// ジェネリック引数には `.biwameta` 上のシンボル索引が無く、
/// 「所属するシンボルと、その中での序数」から id を合成している。
/// `.biwamir` も同じ組でジェネリック引数を書くので、
/// 合成と分解をここに集める。
pub fn compose_genarg_local_idx(owner_sym_idx: u32, ordinal: u32) -> u32 {
    (owner_sym_idx << 10) | ordinal
}

/// [`compose_genarg_local_idx`] の逆。`(所属シンボル, 序数)` を返す。
pub fn decompose_genarg_local_idx(local_idx: u32) -> (u32, u32) {
    (local_idx >> 10, local_idx & 0x3FF)
}

/// 外部パッケージの fn ローカルジェネリクス引数 ID エンコード。
fn ext_loc_gen_id(fn_sym_idx: u32, ordinal: u32) -> u32 {
    (fn_sym_idx << 10) | ordinal
}

/// 外部シンボル表 (ext_sym_table) の組み立て。
///
/// エンコード中に依存パッケージのシンボルが現れるたびに [`Self::intern`] を呼び、
/// 返ってきた索引を `DiskTyKind::ExternalDefined` の sym_id に入れる。
/// 同じシンボルへの複数の参照は 1 エントリを共有する。
///
/// `PackageId` は (name, version) から導出される安定 id なので、
/// ファイルローカルな番号に置き換えて名前で解決し直す必要がない。
struct ExtSymBuilder {
    /// (PackageId の生値, 相手のシンボルインデックス) → ext_sym_table 索引
    entry_index: HashMap<(u32, u32), u32>,
    entries: Vec<DiskExternalSymbol>,
}

impl ExtSymBuilder {
    fn new() -> Self {
        Self {
            entry_index: HashMap::new(),
            entries: Vec::new(),
        }
    }

    /// 外部シンボルを登録し、ext_sym_table のインデックスを返す。
    fn intern(&mut self, pkg_id: biwac_base::PackageId, sym_idx: u32) -> u32 {
        let key = (pkg_id.value(), sym_idx);
        if let Some(i) = self.entry_index.get(&key) {
            return *i;
        }
        let i = self.entries.len() as u32;
        self.entries.push(DiskExternalSymbol {
            pkg: pkg_id.value(),
            sym: DiskSymbolIndex(sym_idx),
        });
        self.entry_index.insert(key, i);
        i
    }

    fn finish(self) -> Vec<DiskExternalSymbol> {
        self.entries
    }
}

// --- BodyBuilder: encode 時にシンボルボディを sym_body_bytes に追記するヘルパー ---
//
// 使い方:
//  ```
//  let mut builder = BodyBuilder::new();
//  let offset = builder.push(fn_data);  // DiskBodyOffset を返す
//  // ... 全シンボルを push した後
//  let body_bytes = builder.finish();
//  ```
struct BodyBuilder {
    buf: Vec<u8>,
}

impl BodyBuilder {
    fn new() -> Self {
        Self { buf: Vec::new() }
    }

    /// ボディをエンコードして追記し、そのオフセット (body_size プレフィックスの位置) を返す。
    fn push(&mut self, body: &impl DiskEncode) -> DiskBodyOffset {
        let offset = DiskBodyOffset(self.buf.len() as u32);

        // body bytes を一旦別バッファにエンコードしてサイズを確定させてから追記
        let mut body_buf = Vec::new();
        body.encode(&mut body_buf);

        (body_buf.len() as u32).encode(&mut self.buf); // body_size prefix
        self.buf.extend_from_slice(&body_buf);

        offset
    }

    fn finish(self) -> Vec<u8> {
        self.buf
    }
}

// ============================================================
// ヘルパー関数 (crate 内部専用)
// ============================================================

fn impl_to_disk_span(
    span: &biwac_span::Span,
    mod_to_file_idx: &HashMap<biwac_base::ModId, DiskFileIndex>,
) -> DiskSpan {
    let file = mod_to_file_idx
        .get(&span.module())
        .copied()
        .unwrap_or(DiskFileIndex(0));
    DiskSpan {
        file,
        begin: span.begin() as u32,
        end: span.end() as u32,
    }
}

fn impl_encode_ty(
    ty: &biwac_hir::Ty,
    ty_to_sym: &HashMap<biwac_span::TyDefId, DiskSymbolIndex>,
    gen_ord: &HashMap<biwac_span::GenDefId, u32>,
    loc_gen_ord: &HashMap<biwac_span::LocalGenDefId, u32>,
    mod_to_file_idx: &HashMap<biwac_base::ModId, DiskFileIndex>,
    ext: &mut ExtSymBuilder,
) -> DiskTy {
    use biwac_hir::TyKind;
    let span = impl_to_disk_span(&ty.span, mod_to_file_idx);
    match &ty.kind {
        TyKind::Int => DiskTy {
            hdr: DiskTyHeader {
                kind: DiskTyKind::Int as u32,
                sym_id: DiskSymbolIndex(0),
                span,
            },
            genargs: vec![],
        },
        TyKind::Float => DiskTy {
            hdr: DiskTyHeader {
                kind: DiskTyKind::Float as u32,
                sym_id: DiskSymbolIndex(0),
                span,
            },
            genargs: vec![],
        },
        TyKind::Bool => DiskTy {
            hdr: DiskTyHeader {
                kind: DiskTyKind::Bool as u32,
                sym_id: DiskSymbolIndex(0),
                span,
            },
            genargs: vec![],
        },
        TyKind::Void => DiskTy {
            hdr: DiskTyHeader {
                kind: DiskTyKind::Void as u32,
                sym_id: DiskSymbolIndex(0),
                span,
            },
            genargs: vec![],
        },
        TyKind::Defined(dt) => {
            let genargs: Vec<DiskTy> = dt
                .genargs
                .iter()
                .map(|t| impl_encode_ty(t, ty_to_sym, gen_ord, loc_gen_ord, mod_to_file_idx, ext))
                .collect();

            // 名前ツリー上のどこにいるかで書き分ける。
            //
            //   予約済み (Int/Float/Bool/Void) → プリミティブに正規化する
            //   自パッケージ                   → Defined + このファイルのシンボル索引
            //   依存パッケージ                 → ExternalDefined + 外部シンボル表の索引
            let pkg = dt.def_id.pkg();
            let (kind, sym_id) = if let Some(prim) = reserved_prim_disk_kind(dt.def_id) {
                // 型としてのプリミティブは通常 TyKind::Int 等になるので普段ここは通らないが、
                // 予約 TyDefId を持つ Defined が紛れ込んでも
                // シンボル索引として誤解釈しないようにしておく。
                (prim, DiskSymbolIndex(0))
            } else if pkg.is_self() {
                let sym_id = ty_to_sym.get(&dt.def_id).copied().unwrap_or_else(|| {
                    // 自パッケージの型はすべて Phase 2 で採番済みのはず。
                    debug_assert!(
                        false,
                        "compiler bug: self-package type {:?} is missing from the symbol table",
                        dt.def_id
                    );
                    DiskSymbolIndex(0)
                });
                (DiskTyKind::Defined, sym_id)
            } else {
                let idx = ext.intern(pkg, dt.def_id.local_idx());
                (DiskTyKind::ExternalDefined, DiskSymbolIndex(idx))
            };

            DiskTy {
                hdr: DiskTyHeader {
                    kind: kind as u32,
                    sym_id,
                    span,
                },
                genargs,
            }
        }
        TyKind::Gen(gid) => {
            let ord = gen_ord.get(gid).copied().unwrap_or(0);
            DiskTy {
                hdr: DiskTyHeader {
                    kind: DiskTyKind::Gen as u32,
                    sym_id: DiskSymbolIndex(ord),
                    span,
                },
                genargs: vec![],
            }
        }
        TyKind::LocGen(lgid) => {
            let ord = loc_gen_ord.get(lgid).copied().unwrap_or(0);
            DiskTy {
                hdr: DiskTyHeader {
                    kind: DiskTyKind::LocGen as u32,
                    sym_id: DiskSymbolIndex(ord),
                    span,
                },
                genargs: vec![],
            }
        }
        TyKind::Fn(ft) => {
            // sym_id = 引数の数。genargs = [arg_ty...] + [rty] (最後が戻り値型)
            let mut genargs: Vec<DiskTy> = ft
                .args
                .iter()
                .map(|a| impl_encode_ty(a, ty_to_sym, gen_ord, loc_gen_ord, mod_to_file_idx, ext))
                .collect();
            genargs.push(impl_encode_ty(
                &ft.rty,
                ty_to_sym,
                gen_ord,
                loc_gen_ord,
                mod_to_file_idx,
                ext,
            ));
            DiskTy {
                hdr: DiskTyHeader {
                    kind: DiskTyKind::Fn as u32,
                    sym_id: DiskSymbolIndex(ft.args.len() as u32),
                    span,
                },
                genargs,
            }
        }
        TyKind::Infer(_) => DiskTy {
            hdr: DiskTyHeader {
                kind: DiskTyKind::Void as u32,
                sym_id: DiskSymbolIndex(0),
                span,
            },
            genargs: vec![],
        },
    }
}

/// 予約済みの [`biwac_span::TyDefId`] に対応するプリミティブの [`DiskTyKind`]。
///
/// プリミティブ型は名前ツリーのどのパッケージにも属さないので、
/// シンボル索引ではなく専用の kind として書き出す。
fn reserved_prim_disk_kind(def_id: biwac_span::TyDefId) -> Option<DiskTyKind> {
    use biwac_span::TyDefId;

    if def_id == TyDefId::INT_TY_DEF_ID {
        Some(DiskTyKind::Int)
    } else if def_id == TyDefId::FLOAT_TY_DEF_ID {
        Some(DiskTyKind::Float)
    } else if def_id == TyDefId::BOOL_TY_DEF_ID {
        Some(DiskTyKind::Bool)
    } else if def_id == TyDefId::VOID_TY_DEF_ID {
        Some(DiskTyKind::Void)
    } else {
        None
    }
}

#[allow(clippy::too_many_arguments)]
/// 所属する型と impl 対象ジェネリック引数から impl の self 型を組み立てる。
///
/// 予約済みの [`TyDefId`] (Int/Float/Bool/Void) はプリミティブなので、
/// 対応する [`TyKind`] にそのまま戻す。
fn impl_self_ty_of(
    parent_ty_def_id: biwac_span::TyDefId,
    impl_genargs: &[biwac_hir::Ty],
    span: &biwac_span::Span,
) -> biwac_hir::Ty {
    use biwac_hir::{DefinedTy, TyKind};
    use biwac_span::TyDefId;

    let kind = if parent_ty_def_id == TyDefId::INT_TY_DEF_ID {
        TyKind::Int
    } else if parent_ty_def_id == TyDefId::FLOAT_TY_DEF_ID {
        TyKind::Float
    } else if parent_ty_def_id == TyDefId::BOOL_TY_DEF_ID {
        TyKind::Bool
    } else if parent_ty_def_id == TyDefId::VOID_TY_DEF_ID {
        TyKind::Void
    } else {
        TyKind::Defined(DefinedTy {
            def_id: parent_ty_def_id,
            genargs: impl_genargs.to_vec(),
        })
    };

    biwac_hir::Ty::new(kind, span.clone())
}

#[allow(clippy::too_many_arguments)]
fn impl_encode_fn_data(
    name: &biwac_hir::Ident,
    signature: &biwac_hir::FnSignature,
    impl_genargs: &[(biwac_hir::Ident, biwac_span::LocalGenDefId)],
    // 関連関数・メソッドなら impl の self 型。トップレベル関数なら None。
    impl_self_ty: Option<&biwac_hir::Ty>,
    ty_to_sym: &HashMap<biwac_span::TyDefId, DiskSymbolIndex>,
    mod_to_file_idx: &HashMap<biwac_base::ModId, DiskFileIndex>,
    source_holder: &biwac_base::SourceHolder,
    strings: &mut StringTable,
    interner: &biwac_base::IdentInterner,
    ext: &mut ExtSymBuilder,
    // この関数自身のシンボル索引と、ジェネリック引数の採番の記録先。
    // 消費側は ext_loc_gen_id(所属シンボル, 序数) で id を合成するので、
    // .biwamir も同じ組を書けるようにここで記録する。
    fn_sym: u32,
    symbol_index: &mut crate::SymbolIndexMap,
) -> format::DiskFnData {
    use biwac_span::LocalGenDefId;
    use codec::DiskVec;
    use format::{DiskArg, DiskGenArg};

    let name_str = interner.get_str(&name.id).unwrap_or("");
    let disk_name = strings.push(name_str);
    let name_span = impl_to_disk_span(&name.span, mod_to_file_idx);

    let def_raw_code = {
        let text = source_holder
            .mods
            .get(&signature.span.module())
            .map(|ms| &ms.src[signature.span.begin()..signature.span.end()])
            .unwrap_or("");
        strings.push(text)
    };
    let def_span = impl_to_disk_span(&signature.span, mod_to_file_idx);

    // combined genargs: impl_genargs 先頭, 次に signature.genargs
    let all_genargs: Vec<(&biwac_hir::Ident, LocalGenDefId)> = impl_genargs
        .iter()
        .chain(signature.genargs.iter())
        .map(|(ident, lgid)| (ident, *lgid))
        .collect();

    let loc_gen_ord: HashMap<LocalGenDefId, u32> = all_genargs
        .iter()
        .enumerate()
        .map(|(i, (_, lgid))| (*lgid, i as u32))
        .collect();

    for (lgid, ord) in &loc_gen_ord {
        symbol_index.insert_fn_genarg(fn_sym, *lgid, *ord);
    }

    let disk_genargs = DiskVec(
        all_genargs
            .iter()
            .map(|(ident, _)| {
                let gname = interner.get_str(&ident.id).unwrap_or("");
                let gname_off = strings.push(gname);
                let gname_span = impl_to_disk_span(&ident.span, mod_to_file_idx);
                DiskGenArg {
                    name: gname_off,
                    name_span: gname_span,
                }
            })
            .collect(),
    );

    let empty_gen_ord: HashMap<biwac_span::GenDefId, u32> = HashMap::new();

    let disk_args = DiskVec(
        signature
            .args
            .iter()
            .map(|arg| {
                let arg_name_str = interner.get_str(&arg.id.id).unwrap_or("");
                let arg_name_off = strings.push(arg_name_str);
                let arg_name_span = impl_to_disk_span(&arg.id.span, mod_to_file_idx);
                let arg_ty = impl_encode_ty(
                    &arg.ty,
                    ty_to_sym,
                    &empty_gen_ord,
                    &loc_gen_ord,
                    mod_to_file_idx,
                    ext,
                );
                DiskArg {
                    name: arg_name_off,
                    name_span: arg_name_span,
                    ty: arg_ty,
                }
            })
            .collect(),
    );

    let rty = impl_encode_ty(
        &signature.rty,
        ty_to_sym,
        &empty_gen_ord,
        &loc_gen_ord,
        mod_to_file_idx,
        ext,
    );

    let disk_impl_self_ty = DiskVec(
        impl_self_ty
            .map(|ty| {
                impl_encode_ty(
                    ty,
                    ty_to_sym,
                    &empty_gen_ord,
                    &loc_gen_ord,
                    mod_to_file_idx,
                    ext,
                )
            })
            .into_iter()
            .collect(),
    );

    format::DiskFnData {
        name: disk_name,
        name_span,
        def_raw_code,
        def_span,
        genargs: disk_genargs,
        args: disk_args,
        rty,
        impl_self_ty: disk_impl_self_ty,
    }
}
