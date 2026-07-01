mod body;
mod codec;
mod format;
pub mod module_view;
mod table;

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::error::DepMetadataError;
use body::SymbolBody;
use codec::{DiskDecode, DiskEncode};
use format::{
    BIWAC_DEPENDENCY_METADATA_FORMAT_VERSION, BIWAC_DEPENDENCY_METADATA_MAGIC, DiskBodyOffset,
    DiskFileIndex, DiskSourceInfo, DiskSpan, DiskSymbolHeader, DiskSymbolIndex, DiskSymbolKind,
    DiskTy, DiskTyHeader, DiskTyKind, DiskVisibility,
};
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
}

impl DepMetadata {
    pub fn new(
        hir: &biwac_hir::Hir,
        source_holder: &biwac_base::SourceHolder,
        interner: &biwac_base::IdentInterner,
    ) -> Self {
        use biwac_base::ModPath;
        use biwac_hir::{AssocValDefKind, TyDefKind, ValDefKind};
        use biwac_span::{GenDefId, LocalGenDefId, TyDefId, ValDefId};
        use codec::DiskVec;
        use format::{DiskGenArg, DiskModData, DiskStructData, DiskStructMember};

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
        //   [struct類][assoc fn類][top-level fn類][mod類]
        // ====================================================

        // --- struct ---
        struct StructItem<'h> {
            def_id: TyDefId,
            def: &'h biwac_hir::StructDef,
            def_impl: &'h biwac_hir::DefinedTyImpl,
        }
        let mut struct_items: Vec<StructItem> = hir
            .tys
            .iter()
            .filter(|(def_id, _)| def_id.pkg().is_self())
            .filter_map(|(def_id, def_impl)| {
                if let TyDefKind::Struct(s) = &def_impl.ty_content {
                    Some(StructItem {
                        def_id: *def_id,
                        def: s.as_ref(),
                        def_impl,
                    })
                } else {
                    None
                }
            })
            .collect();
        struct_items.sort_by_key(|i| i.def_id.value());

        let mut ty_to_sym: HashMap<TyDefId, DiskSymbolIndex> = HashMap::new();
        let mut next_idx = 0u32;
        for item in &struct_items {
            ty_to_sym.insert(item.def_id, DiskSymbolIndex(next_idx));
            next_idx += 1;
        }

        // --- assoc fn (struct の impl に紐づく関数) ---
        struct AssocFnItem<'h> {
            val_def_id: ValDefId,
            name: &'h biwac_hir::Ident,
            signature: &'h biwac_hir::FnSignature,
            impl_genargs: &'h [(biwac_hir::Ident, LocalGenDefId)],
            parent_ty_def_id: TyDefId,
        }
        let mut assoc_fn_items: Vec<AssocFnItem> = Vec::new();
        for item in &struct_items {
            for impl_list in item.def_impl.vals.values() {
                for (val_def_id, pair) in &impl_list.vals {
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
                        parent_ty_def_id: item.def_id,
                    });
                }
            }
        }
        assoc_fn_items.sort_by_key(|i| i.val_def_id.value());

        let mut val_to_sym: HashMap<ValDefId, DiskSymbolIndex> = HashMap::new();
        for item in &assoc_fn_items {
            val_to_sym.insert(item.val_def_id, DiskSymbolIndex(next_idx));
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
                    ValDefKind::ExternalFn(_) => return None,
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
            next_idx += 1;
        }

        // --- modules ---
        let mut mod_paths_sorted: Vec<ModPath> =
            self_mods.iter().map(|(_, ms)| ms.modu.clone()).collect();
        mod_paths_sorted.sort_by_key(|mp| mp.file_name());

        let mut mod_path_to_sym: HashMap<ModPath, DiskSymbolIndex> = HashMap::new();
        for mp in &mod_paths_sorted {
            mod_path_to_sym.insert(mp.clone(), DiskSymbolIndex(next_idx));
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

            // members (HashMap なのでソートして順序を安定させる)
            let mut members_vec: Vec<DiskStructMember> = item
                .def
                .members
                .iter()
                .map(|(ident, ty)| {
                    let mem_name = interner.get_str(ident).unwrap_or("");
                    let mem_name_off = strings.push(mem_name);
                    // メンバ名 span は HIR に存在しないので型の span で代替
                    let mem_name_span = impl_to_disk_span(&ty.span, &mod_to_file_idx);
                    let disk_ty =
                        impl_encode_ty(ty, &ty_to_sym, &gen_ord, &empty_loc_gen, &mod_to_file_idx);
                    DiskStructMember {
                        name: mem_name_off,
                        name_span: mem_name_span,
                        ty: disk_ty,
                    }
                })
                .collect();
            members_vec.sort_by_key(|m| m.name.0);

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

        // --- assoc fn ボディ ---
        for item in &assoc_fn_items {
            let fn_data = impl_encode_fn_data(
                item.name,
                item.signature,
                item.impl_genargs,
                &ty_to_sym,
                &mod_to_file_idx,
                source_holder,
                &mut strings,
                interner,
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
            let fn_data = impl_encode_fn_data(
                item.name,
                item.signature,
                item.impl_genargs,
                &ty_to_sym,
                &mod_to_file_idx,
                source_holder,
                &mut strings,
                interner,
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

        Self {
            sym_hdrs,
            sym_bodies,
            source_files,
            strings,
            root_sym_idx,
        }
    }

    // --- Decode ---

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

        Ok(Self {
            sym_hdrs,
            sym_bodies,
            source_files,
            strings,
            root_sym_idx,
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

        buf
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

    /// 外部パッケージの fn シンボル1つを ValDefKind に変換する。
    /// TyCtx の遅延ロードから呼ばれる。sym_idx は DefId.local_idx() と一致する。
    pub fn get_ext_val_kind(
        &self,
        sym_idx: u32,
        pkg_id: biwac_base::PackageId,
        interner: &mut biwac_base::IdentInterner,
    ) -> Option<biwac_hir::ValDefKind> {
        let body = self.get_symbol_body(sym_idx as usize).ok()?;
        let SymbolBody::Fn(fn_data) = body else {
            return None;
        };
        let sig = self.impl_disk_fn_to_signature(sym_idx, fn_data, pkg_id, interner);
        Some(biwac_hir::ValDefKind::ExternalFn(Box::new(sig)))
    }

    /// 外部パッケージの struct シンボル1つを DefinedTyImpl に変換する。
    /// TyCtx の遅延ロードから呼ばれる。struct のメンバ型と assoc fns を含む。
    pub fn get_ext_ty_impl(
        &self,
        struct_sym_idx: u32,
        pkg_id: biwac_base::PackageId,
        interner: &mut biwac_base::IdentInterner,
    ) -> Option<biwac_hir::DefinedTyImpl> {
        use biwac_hir::{
            AssocValDefKind, DefinedTyImpl, Ident, NativeFnDef, StructDef, TyDefKind,
            TyValImplGenargsContentPair, TyValImplList,
        };
        use biwac_span::{DefId, GenDefId, LocalGenDefId, PackageLocalDefId, Span, ValDefId};

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

        let struct_genarg_count = struct_data.genargs.0.len();
        let mut vals: std::collections::HashMap<biwac_base::InternedIdent, TyValImplList> =
            std::collections::HashMap::new();

        // assoc fns を vals に登録する。
        // NOTE: disk format にはimpl genargs とfn genargs の境界が記録されていないため、
        //       struct のジェネリクス数を上限として先頭から impl genargs とみなす。
        //       impl[T] Foo[T] のような標準的なパターンは正しく再現できる。
        //       TODO: impl Foo[Int] { ... } のように特殊化されたimplの場合は
        //             disk format に impl_target_genargs を追加することで解決する。
        for &assoc_sym_idx_disk in &struct_data.assoc_symbols.0 {
            let assoc_sym_idx = assoc_sym_idx_disk.0;
            let Ok(assoc_body) = self.get_symbol_body(assoc_sym_idx as usize) else {
                continue;
            };
            let SymbolBody::Fn(fn_data) = assoc_body else {
                continue;
            };

            let val_def_id =
                ValDefId::new(DefId::new(pkg_id, PackageLocalDefId::new(assoc_sym_idx)));

            let impl_genarg_count = struct_genarg_count.min(fn_data.genargs.0.len());
            let impl_genarg_pattern: Vec<biwac_hir::Ty> = (0..impl_genarg_count)
                .map(|i| {
                    let lgid = LocalGenDefId::new(DefId::new(
                        pkg_id,
                        PackageLocalDefId::new(ext_loc_gen_id(assoc_sym_idx, i as u32)),
                    ));
                    biwac_hir::Ty::new(biwac_hir::TyKind::LocGen(lgid), Span::dummy())
                })
                .collect();

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

        Some(DefinedTyImpl {
            ty_content: TyDefKind::Struct(Box::new(struct_def)),
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
            // TODO: メソッドの self_ty は disk format から復元していない。
            //       現状の型推論では self_ty は参照されないため問題ないが、
            //       将来 emit 等で必要になった場合は disk format への追加が必要。
            self_ty: None,
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

/// 外部パッケージの fn ローカルジェネリクス引数 ID エンコード。
fn ext_loc_gen_id(fn_sym_idx: u32, ordinal: u32) -> u32 {
    (fn_sym_idx << 10) | ordinal
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
            let sym_id = ty_to_sym
                .get(&dt.def_id)
                .copied()
                .unwrap_or(DiskSymbolIndex(0));
            let genargs = dt
                .genargs
                .iter()
                .map(|t| impl_encode_ty(t, ty_to_sym, gen_ord, loc_gen_ord, mod_to_file_idx))
                .collect();
            DiskTy {
                hdr: DiskTyHeader {
                    kind: DiskTyKind::Defined as u32,
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
                .map(|a| impl_encode_ty(a, ty_to_sym, gen_ord, loc_gen_ord, mod_to_file_idx))
                .collect();
            genargs.push(impl_encode_ty(
                &ft.rty,
                ty_to_sym,
                gen_ord,
                loc_gen_ord,
                mod_to_file_idx,
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

#[allow(clippy::too_many_arguments)]
fn impl_encode_fn_data(
    name: &biwac_hir::Ident,
    signature: &biwac_hir::FnSignature,
    impl_genargs: &[(biwac_hir::Ident, biwac_span::LocalGenDefId)],
    ty_to_sym: &HashMap<biwac_span::TyDefId, DiskSymbolIndex>,
    mod_to_file_idx: &HashMap<biwac_base::ModId, DiskFileIndex>,
    source_holder: &biwac_base::SourceHolder,
    strings: &mut StringTable,
    interner: &biwac_base::IdentInterner,
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
    );

    format::DiskFnData {
        name: disk_name,
        name_span,
        def_raw_code,
        def_span,
        genargs: disk_genargs,
        args: disk_args,
        rty,
    }
}
