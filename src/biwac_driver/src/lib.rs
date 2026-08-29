mod dep_graph;

use colored::Colorize;
use std::{
    collections::HashSet,
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
};

use biwac_base::{IdentInterner, PackageId, PackageName, SourceHolder};
use biwac_dependency_metadata::{DepMetadata, ExternalPackage, SymbolIndexMap};
use biwac_fingerprint::{Fingerprint, Freshness, PackageHashes, SourceEntry, StaleReason};
use biwac_hash::Hash64;

use dep_graph::DepGraph;

/// ビルドの振る舞いの指定。
#[derive(Debug, Clone, Copy, Default)]
pub struct BuildOptions {
    /// 鮮度判定を飛ばして全パッケージを建て直す。
    pub force_rebuild: bool,

    /// MIR までで止める。codegen は走らせない。
    ///
    /// `.biwamir` 自体は毎ビルド書かれるので、これは
    /// 「既定の出力を作らずに MIR だけ確かめたい」ときの指定である。
    pub emit_mir: bool,
}

/// このターゲットが、依存パッケージの **関数の本体** を
/// 自分の出力に取り込むかどうか。
///
/// 単相化するターゲット (WASM 等) では依存の本体が自分の出力に混ざるので、
/// 依存の `.biwamir` が変われば建て直さなければならない。
/// TypeScript は `greeter.ts` に std の本体を入れないので、
/// 依存の関数の中身が変わっても建て直す必要がない。
///
/// `--target` を入れるときに、ここがターゲットごとの分岐になる。
const TARGET_CONSUMES_DEP_MIR: bool = false;

/// このコンパイラの同一性。
///
/// これが前回と違えばキャッシュはすべて無効になる。
/// `.biwameta` の形式版数を混ぜてあるので、形式を変えたときは
/// 「古い成果物を掴んでエラー」ではなく「フィンガープリント不一致で建て直し」になる。
fn compiler_identity() -> Hash64 {
    biwac_fingerprint::compiler_hash(&[
        biwac_dependency_metadata::BIWAC_DEPENDENCY_METADATA_FORMAT_VERSION,
        biwac_mir::BIWAC_MIR_FORMAT_VERSION,
    ])
}

/// 依存グラフの各パッケージの、今回のビルドで確定したハッシュ。
///
/// トポロジカル順 (葉から) に埋まっていくので、
/// あるパッケージを判定する時点で、その依存のハッシュは必ず揃っている。
type HashMapOfPackages = std::collections::HashMap<PackageId, PackageHashes>;

pub fn compile(pkg_root_path: PathBuf, options: BuildOptions) -> Result<(), ()> {
    println!("{}", "Compiling...".green().bold(),);

    // MIR は成果物ではなくキャッシュもされないので、
    // 鮮度判定で飛ばされると何も出力されずに終わってしまう。
    // 求められたら建て直す。
    let options = BuildOptions {
        force_rebuild: options.force_rebuild || options.emit_mir,
        ..options
    };

    let metadata = biwac_metadata_loader::try_load_package_metadata(pkg_root_path.clone())
        .map_err(|e| {
            e.print_error_message();
            biwac_base::print_error_finish_message(1);
        })?;

    println!(
        "Package: {} v{}.{}.{}",
        metadata.metadata.name.value(),
        metadata.metadata.version.major(),
        metadata.metadata.version.minor(),
        metadata.metadata.version.patch()
    );

    // 依存パッケージは <root>/.biwa_build/deps/<name>/ に取得済みである前提。
    //
    // 推移的依存も含めてここに平らに並ぶので、ビルド全体で参照する依存ディレクトリは
    // このひとつだけになる。依存パッケージ自身の deps/ は見ない。
    // (deps/greeter を建てるときも、その依存 std / color はここから引く)
    let packages_dir = biwac_base::dependencies_dir(&pkg_root_path);

    let root_dep_names: Vec<String> = metadata
        .metadata
        .dependencies
        .iter()
        .map(|d| d.name.value().to_string())
        .collect();

    // 取得は未実装なので、無ければその旨を伝えて止まる。
    if !root_dep_names.is_empty() && !packages_dir.is_dir() {
        eprintln!(
            "Error: dependencies are not fetched: `{}` does not exist",
            packages_dir.display()
        );
        biwac_base::print_error_finish_message(1);
        return Err(());
    }

    // Discover full transitive dependency graph.
    // 依存が無くても空グラフとして扱い、以降の分岐を減らす。
    let root_dep_refs: Vec<&str> = root_dep_names.iter().map(|s| s.as_str()).collect();
    let dep_graph = DepGraph::discover(&root_dep_refs, &packages_dir).map_err(|_| {
        biwac_base::print_error_finish_message(1);
    })?;

    // Build in topological order (leaves = no deps first).
    // Items within the same batch are independent and can be parallelized (future tokio).
    let batches = dep_graph.topo_batches().map_err(|_| {
        biwac_base::print_error_finish_message(1);
    })?;

    let mut svhs = HashMapOfPackages::new();
    for batch in &batches {
        // TODO: parallelize within batch using tokio

        for dep_name in batch {
            let dep_root = packages_dir.join(dep_name);
            let svh = build_or_reuse_package(
                dep_root,
                &packages_dir,
                &dep_graph,
                &svhs,
                options,
                DisplayDepth::Dependency,
            )?;
            let pkg_id = dep_graph
                .pkg_id(dep_name)
                .expect("package must be in graph");
            svhs.insert(pkg_id, svh);
        }
    }

    // 生成物は import 文が `./<package>.ts` を指すため、
    // 自パッケージと推移的依存の .ts が同じディレクトリに並んでいる必要がある。
    //
    // 再ビルドしたものだけでなく **グラフの全パッケージ** を対象にする。
    // キャッシュが効いた依存の .ts も要るし、
    // 依存から外れたパッケージの .ts は消さなければならない。
    let build_dir_path = prepare_build_dir(&pkg_root_path)?;
    if !options.emit_mir {
        collect_dep_bins(
            &dep_graph.all_packages(),
            metadata.metadata.name.value(),
            &packages_dir,
            &build_dir_path,
        )?;
    }

    build_or_reuse_package(
        pkg_root_path,
        &packages_dir,
        &dep_graph,
        &svhs,
        options,
        DisplayDepth::Root,
    )?;

    println!("{}", "Finished!".green().bold(),);

    Ok(())
}

/// 進捗表示のインデント。ルートパッケージと依存で見た目を変えるだけ。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DisplayDepth {
    Root,
    Dependency,
}

impl DisplayDepth {
    fn indent(&self) -> &'static str {
        match self {
            Self::Root => "",
            Self::Dependency => "  ",
        }
    }
}

/// パッケージ 1 つを、必要なら再ビルドする。
///
/// 戻り値はそのパッケージの SVH (インタフェースのハッシュ)。
/// キャッシュを使った場合は前回の値をそのまま返す。
/// 呼び出し側はこれを下流のパッケージの鮮度判定に渡す。
fn build_or_reuse_package(
    pkg_root: PathBuf,
    packages_dir: &Path,
    dep_graph: &DepGraph,
    svhs: &HashMapOfPackages,
    options: BuildOptions,
    depth: DisplayDepth,
) -> Result<PackageHashes, ()> {
    let metadata =
        biwac_metadata_loader::try_load_package_metadata(pkg_root.clone()).map_err(|e| {
            e.print_error_message();
            biwac_base::print_error_finish_message(1);
        })?;
    let pkg_name = metadata.metadata.name.value().to_string();

    let build_dir_path = prepare_build_dir(&pkg_root)?;

    // このパッケージのビルドが読むことになる依存の集合 = 推移閉包。
    //
    // ルートパッケージはグラフに含まれないので、
    // その推移閉包はグラフ全体そのものになる。
    let transitive_deps = match depth {
        DisplayDepth::Root => dep_graph.all_packages(),
        DisplayDepth::Dependency => dep_graph.transitive_deps(&pkg_name),
    };
    let dep_hashes: Vec<(PackageId, PackageHashes)> = transitive_deps
        .iter()
        .filter_map(|name| {
            let pkg_id = dep_graph.pkg_id(name)?;
            Some((pkg_id, *svhs.get(&pkg_id)?))
        })
        .collect();

    let sources = biwac_fingerprint::collect_sources(&pkg_root).map_err(|e| {
        eprintln!("Error: failed to read sources of `{pkg_name}`: {e}");
        biwac_base::print_error_finish_message(1);
    })?;

    let freshness = check_freshness(
        &build_dir_path,
        &pkg_name,
        &metadata.metadata,
        &dep_hashes,
        &sources,
        options,
    );

    if let Freshness::Fresh(hashes) = freshness {
        println!(
            "{}{} {} v{}.{}.{}",
            depth.indent(),
            "Fresh".cyan().bold(),
            pkg_name,
            metadata.metadata.version.major(),
            metadata.metadata.version.minor(),
            metadata.metadata.version.patch(),
        );
        return Ok(hashes);
    }
    let Freshness::Stale(reason) = freshness else {
        unreachable!()
    };

    println!(
        "{}{} {} v{}.{}.{} ({})",
        depth.indent(),
        "Compiling".green().bold(),
        pkg_name,
        metadata.metadata.version.major(),
        metadata.metadata.version.minor(),
        metadata.metadata.version.patch(),
        reason.describe(),
    );

    let mut interner = IdentInterner::new();
    let direct_dep_names: Vec<String> = metadata
        .metadata
        .dependencies
        .iter()
        .map(|d| d.name.value().to_string())
        .collect();

    let external_packages = load_external_packages(
        &transitive_deps,
        &direct_dep_names,
        dep_graph,
        packages_dir,
        &mut interner,
    )?;

    // 依存の `.biwamir` を読む。
    //
    // 単相化の入力であり、同時に「書き出した MIR が読み戻せるか」の検査でもある。
    // 既定のビルド (TypeScript) は単相化を通らないので読まない。
    let dep_mirs = if options.emit_mir {
        let deps: Vec<(PackageId, String, Hash64)> = external_packages
            .iter()
            .filter_map(|p| {
                Some((
                    p.pkg_id,
                    interner.get_str(&p.ident)?.to_string(),
                    p.meta.svh,
                ))
            })
            .collect();
        load_dep_mirs(&deps, packages_dir, &mut interner, depth)?
    } else {
        Vec::new()
    };

    let hashes = load_analyze_and_codegen_single_package(
        external_packages,
        &mut interner,
        &metadata,
        &dep_hashes,
        dep_mirs,
        pkg_root,
        build_dir_path.clone(),
        options,
    )?;

    // `--emit` を付けたビルドは既定の生成物を作っていないので、
    // 鮮度を記録してはいけない。記録すると次回「Fresh」と言い張って
    // 生成物が無いまま成功してしまう。
    if options.emit_mir {
        return Ok(hashes);
    }

    // 次回の鮮度判定のために、今回のビルドの状態を記録する。
    let fingerprint = Fingerprint::of_build(
        compiler_identity(),
        &metadata.metadata,
        hashes,
        &dep_hashes,
        sources,
    );
    let fp_path = biwac_fingerprint::fingerprint_path(&build_dir_path, &pkg_name);
    std::fs::write(&fp_path, fingerprint.encode_file()).map_err(|e| {
        eprintln!("Error: failed to write {:?}: {}", fp_path, e);
        biwac_base::print_error_finish_message(1);
    })?;

    Ok(hashes)
}

/// 前回のビルドから状況が変わっていないかを判定する。
///
/// 判定材料は [`biwac_fingerprint`] に閉じている。
/// ここは「前回の記録を読み出す」ところだけを持つ。
fn check_freshness(
    build_dir_path: &Path,
    pkg_name: &str,
    metadata: &biwac_base::PackageMetadata,
    dep_hashes: &[(PackageId, PackageHashes)],
    sources: &[SourceEntry],
    options: BuildOptions,
) -> Freshness {
    if options.force_rebuild {
        return Freshness::Stale(StaleReason::Forced);
    }

    // シグニチャと本体のキャッシュが無ければ、記録があっても意味がない。
    if !metadata_path(build_dir_path, pkg_name).exists()
        || !mir_path(build_dir_path, pkg_name).exists()
    {
        return Freshness::Stale(StaleReason::NoPreviousBuild);
    }

    // 生成物が消えていれば、記録がどうであれ建て直す。
    // 出力ディレクトリだけ消したときに「Fresh」と言い張って、
    // 生成物が無いまま成功してしまうのを防ぐ。
    if !bin_path(build_dir_path, pkg_name).exists() {
        return Freshness::Stale(StaleReason::MissingOutput);
    }

    let fp_path = biwac_fingerprint::fingerprint_path(build_dir_path, pkg_name);
    let Ok(data) = std::fs::read(&fp_path) else {
        return Freshness::Stale(StaleReason::NoPreviousBuild);
    };
    // 形式が変わった / 壊れている場合は、単に「前回の情報は使えない」と扱って建て直す。
    let Ok(previous) = Fingerprint::decode_file(&data) else {
        return Freshness::Stale(StaleReason::UnreadableFingerprint);
    };

    previous.freshness(
        compiler_identity(),
        metadata,
        dep_hashes,
        sources,
        TARGET_CONSUMES_DEP_MIR,
    )
}

fn metadata_path(build_dir_path: &Path, pkg_name: &str) -> PathBuf {
    build_dir_path.join(format!("{pkg_name}.biwameta"))
}

/// MIR のキャッシュ。`.biwameta` と対で置かれる。
fn mir_path(build_dir_path: &Path, pkg_name: &str) -> PathBuf {
    build_dir_path.join(format!("{pkg_name}.{}", biwac_mir::MIR_FILE_EXTENSION))
}

/// codegen の出力先。
fn bin_path(build_dir_path: &Path, pkg_name: &str) -> PathBuf {
    if cfg!(feature = "typescript") {
        build_dir_path
            .join("typescript")
            .join(format!("{pkg_name}.ts"))
    } else {
        todo!()
    }
}

fn prepare_build_dir(pkg_root: &Path) -> Result<PathBuf, ()> {
    let build_dir_path = pkg_root.join(Path::new(biwac_base::BIWA_BUILD_DIRECTORY_NAME));
    if build_dir_path.exists() && !build_dir_path.is_dir() {
        panic!(
            "Destination directory broken, conflicted file found: `{}`",
            build_dir_path
                .as_os_str()
                .to_str()
                .expect("broken build directory path")
        );
    }
    std::fs::create_dir_all(&build_dir_path).map_err(|e| {
        eprintln!("Error: failed to create {:?}: {}", build_dir_path, e);
        biwac_base::print_error_finish_message(1);
    })?;
    Ok(build_dir_path)
}

/// 依存グラフの推移閉包すべての `.biwameta` をロードする。
///
/// 直接依存だけでは足りない。依存の `.biwameta` に載っているシグニチャが
/// さらにその依存の型を参照していることがあり
/// (`greeter::theme() -> color::Rgb`)、
/// その参照を DefId に復元するには相手のメタデータが要るからである。
/// 名前で引ける (= import できる) のは直接依存だけなので、
/// [`ExternalPackage::direct`] で区別する。
fn load_external_packages(
    transitive_deps: &[String],
    direct_dep_names: &[String],
    dep_graph: &DepGraph,
    packages_dir: &Path,
    interner: &mut IdentInterner,
) -> Result<Vec<ExternalPackage>, ()> {
    let mut packages = Vec::with_capacity(transitive_deps.len());
    for name in transitive_deps {
        let Some(pkg_id) = dep_graph.pkg_id(name) else {
            continue;
        };
        let dep_root = packages_dir.join(name);
        let meta = load_dep_metadata(&dep_root, name)?;
        packages.push(ExternalPackage {
            ident: interner.get_or_insert(name),
            pkg_id,
            meta: Arc::new(meta),
            direct: direct_dep_names.iter().any(|d| d == name),
        });
    }

    Ok(packages)
}

/// Loads a .biwameta file from a built dependency's build directory.
fn load_dep_metadata(dep_root: &Path, dep_name: &str) -> Result<DepMetadata, ()> {
    let meta_path = dep_root
        .join(biwac_base::BIWA_BUILD_DIRECTORY_NAME)
        .join(format!("{}.biwameta", dep_name));
    let data = std::fs::read(&meta_path).map_err(|e| {
        eprintln!("Error: failed to read {:?}: {}", meta_path, e);
    })?;
    DepMetadata::decode_file(&data).map_err(|e| {
        eprintln!("Error: failed to decode {:?}: {}", meta_path, e);
    })
}

/// Persist self package's symbol metadata to disk for dependents.
///
/// 生成した `.biwameta` の SVH と、そこで決まったシンボルの採番を返す。
/// 採番は `.biwamir` を書くときにそのまま使う
/// (下流から見たこのパッケージの DefId はこの採番で決まる)。
fn persist_dep_metadata(
    hir: &biwac_hir::Hir,
    lang_items: &biwac_lang_item::LangItemTable,
    srcs: &biwac_base::SourceHolder,
    interner: &biwac_base::IdentInterner,
    dep_hashes: &[(PackageId, PackageHashes)],
    build_dir_path: &Path,
    metadata: &biwac_base::MetadataHolder,
) -> Result<(Hash64, SymbolIndexMap), ()> {
    // `.biwameta` が記録するのはインタフェースの伝播に使う SVH だけである。
    let dep_svhs: Vec<(PackageId, Hash64)> =
        dep_hashes.iter().map(|(id, h)| (*id, h.svh)).collect();
    let (dep_meta, symbol_index) = DepMetadata::new(hir, srcs, interner, lang_items, &dep_svhs);
    let svh = dep_meta.svh;
    let meta_bytes = dep_meta.encode_file();
    let meta_path = metadata_path(build_dir_path, metadata.metadata.name.value());
    std::fs::write(&meta_path, meta_bytes)
        .map_err(|e| {
            eprintln!("Error: failed to write {:?}: {}", meta_path, e);
        })
        .map(|_| (svh, symbol_index))
}

/// パイプライン本体。生成した `.biwameta` と `.biwamir` のハッシュを返す。
fn load_analyze_and_codegen_single_package(
    external_packages: Vec<ExternalPackage>,
    interner: &mut biwac_base::IdentInterner,
    metadata: &biwac_base::MetadataHolder,
    dep_hashes: &[(PackageId, PackageHashes)],
    dep_mirs: Vec<(PackageId, biwac_mir::Mir)>,
    pkg_root_path: PathBuf,
    build_dir_path: PathBuf,
    options: BuildOptions,
) -> Result<PackageHashes, ()> {
    // 型推論と codegen は「名前で引けるか」を問わないので、
    // direct かどうかを落として推移閉包すべてを渡す。
    let ext_pkgs_for_ty: Vec<(biwac_base::PackageId, Arc<DepMetadata>)> = external_packages
        .iter()
        .map(|p| (p.pkg_id, Arc::clone(&p.meta)))
        .collect();

    let mut srcs = SourceHolder::default();
    let package_name_interned = interner.get_or_insert(metadata.metadata.name.value());

    let pkg = biwac_package_loader::Pkg::try_load(metadata, interner, &mut srcs, pkg_root_path)
        .map_err(|e| e.print_error_messages())?;

    // Attribute check: AST から HIR への lowering の前に、
    // 既知の属性か / キー・値型 / 付与対象を検証する。
    // 後段の lang item 回収はこれを通過していることを前提にできる。
    check_attributes(&pkg, interner, &srcs, metadata)?;

    let pkg_kind = pkg.pkg_kind;
    let root_mod_id = pkg.root_module.mod_id;

    let biwac_name_resolver::ResolveOutput { hir, lang_items } =
        biwac_name_resolver::NameResolver::new(
            metadata,
            external_packages,
            package_name_interned,
            pkg,
        )
        .unwrap()
        .try_resolve(interner)
        .map_err(|errs| print_errors(&errs, interner, &srcs, metadata))?;

    // Scene contract check: scene のシグネチャと、
    // playable package のエントリポイント (scene main) の存在を検証する。
    // シグネチャは名前解決の時点で確定しているので型推論より前に走らせる。
    let well_known_scenes = biwac_scene::check(&hir, &lang_items, pkg_kind, root_mod_id, interner)
        .map_err(|errs| print_errors(&errs, interner, &srcs, metadata))?;

    // Persist self package's symbol metadata to disk for dependents.
    // lang item テーブルも書き出すので、依存側はこれを読んで復元する。
    let (svh, symbol_index) = persist_dep_metadata(
        &hir,
        &lang_items,
        &srcs,
        interner,
        dep_hashes,
        &build_dir_path,
        metadata,
    )?;

    let hir =
        biwac_type_inferrer::TyCtx::new(hir, lang_items.clone(), ext_pkgs_for_ty.clone(), interner)
            .infer()
            .unwrap();

    // MIR は `.biwameta` と対で毎ビルド書き出す。
    // 単相化するターゲットは依存パッケージの本体を必要とするので、
    // 「そのターゲットのときだけ書く」形にはできない
    // (ある日 wasm を建てようとしたら依存の MIR が無い、ということになる)。
    let (mir_hash, mir) = persist_mir(
        &hir,
        &lang_items,
        interner,
        &symbol_index,
        svh,
        &build_dir_path,
        metadata,
    )?;
    let hashes = PackageHashes { svh, mir: mir_hash };

    // `--emit` は既定の出力を置き換える (rustc と同じ流儀)。
    // 中間表現だけを見たいときに codegen まで走らせる理由が無いのと、
    // 中間表現の検証をターゲットの実装状況に縛られずに行えるようにするため。
    if options.emit_mir {
        // 単相化できるのは根を持つパッケージ、つまり playable なものだけである。
        // ライブラリはどの型で実体化されるかを知らないので、
        // ジェネリックなままの `.biwamir` を出して終わる。
        if pkg_kind.is_playable() {
            let mono = monomorphize_program(
                &hir,
                &mir,
                &ext_pkgs_for_ty,
                &dep_mirs,
                &well_known_scenes,
                interner,
            )?;
            println!(
                "{} {} instance(s), {} type(s)",
                "Monomorphized".cyan().bold(),
                mono.instances.len(),
                mono.types.len(),
            );
            last_monomorphized(mono);
        }
        return Ok(hashes);
    }

    // codegen も lang item を使う。
    // novel statement を std の関数呼び出しに展開するため。
    // 外部パッケージのメタデータはシンボル名のマングリングに使う
    // (外部シンボルは HIR に無く span もダミーのため)。
    let bin = biwac_generator::arch::typescript::generate(
        &hir,
        interner,
        &srcs,
        &ext_pkgs_for_ty,
        &lang_items,
        &well_known_scenes,
    );

    write_bin(build_dir_path.to_path_buf(), &metadata.metadata.name, &bin).unwrap();

    Ok(hashes)
}

/// MIR を構築して `.biwamir` に書き出し、そのハッシュを返す。
///
/// 不変条件の検査もここで走らせる。
/// 検査に落ちるのはコンパイラのバグなので、黙って出力せずエラーにする。
///
/// シンボルは `symbol_index` で `.biwameta` の索引に読み替えて書く。
/// 下流から見たこのパッケージのシンボルの DefId はその索引で決まるので、
/// `.biwamir` も同じ空間で書かなければ噛み合わない。
fn persist_mir(
    hir: &biwac_hir::Hir,
    lang_items: &biwac_lang_item::LangItemTable,
    interner: &biwac_base::IdentInterner,
    symbol_index: &SymbolIndexMap,
    meta_svh: Hash64,
    build_dir_path: &Path,
    metadata: &biwac_base::MetadataHolder,
) -> Result<(Hash64, biwac_mir::Mir), ()> {
    let pkg_id = self_package_id(&metadata.metadata);
    let mut mir = biwac_mir_build::build(hir, lang_items, pkg_id);

    let errors = biwac_mir::validate(&mir);
    if !errors.is_empty() {
        eprintln!("Error: built MIR is broken (this is a compiler bug):");
        for e in &errors {
            eprintln!("  {e}");
        }
        biwac_base::print_error_finish_message(errors.len());
        return Err(());
    }

    // 書き出す前に畳む。`.biwamir` に載る形も、下流が単相化に使う形も
    // これを通したあとのものになる。
    biwac_mir_transform::run_passes(&mut mir, biwac_mir_transform::default_passes()).map_err(
        |e| {
            eprintln!("Error: {e}");
            biwac_base::print_error_finish_message(1);
        },
    )?;

    let text = biwac_mir::encode(
        &mir,
        &biwac_mir::EncodeCtx {
            pkg_id,
            meta_svh,
            symbols: Some(symbol_index),
            interner,
        },
    );

    let path = mir_path(build_dir_path, metadata.metadata.name.value());
    std::fs::write(&path, &text).map_err(|e| {
        eprintln!("Error: failed to write {:?}: {}", path, e);
        biwac_base::print_error_finish_message(1);
    })?;

    Ok((mir_hash(&text), mir))
}

/// 直近の単相化の結果を、テストから覗けるようにしておく。
///
/// 結果はファイルにしないので、テストは in-process でこれを見る。
/// 本番の経路では書くだけで、誰も読まない。
#[cfg(test)]
fn last_monomorphized(mono: biwac_mir::MonoMir) {
    tests::LAST_MONO.with(|slot| *slot.borrow_mut() = Some(mono));
}

#[cfg(not(test))]
fn last_monomorphized(_mono: biwac_mir::MonoMir) {}

/// プログラム全体を単相化する。
///
/// 単相化の結果はファイルにしない。消費者は次に入るバックエンドで、
/// それはメモリ上で受け取れば足りるためである。
/// ここでは「通ること」と規模だけを確かめる。
fn monomorphize_program(
    hir: &biwac_hir::Hir,
    own: &biwac_mir::Mir,
    ext_pkgs: &[(PackageId, Arc<DepMetadata>)],
    dep_mirs: &[(PackageId, biwac_mir::Mir)],
    well_known_scenes: &biwac_scene::WellKnownScenes,
    interner: &mut IdentInterner,
) -> Result<biwac_mir::MonoMir, ()> {
    // 根はランタイムが名前で呼ぶ scene だけである。
    // そこから辿れない関数は成果物に入らない (到達性による除去がここで効く)。
    let roots: Vec<biwac_span::ValDefId> = biwac_scene::WellKnownScene::ALL
        .iter()
        .filter_map(|s| well_known_scenes.get(*s))
        .collect();

    let deps: Vec<(PackageId, &DepMetadata, &biwac_mir::Mir)> = dep_mirs
        .iter()
        .filter_map(|(pkg_id, mir)| {
            let meta = ext_pkgs.iter().find(|(id, _)| id == pkg_id)?;
            Some((*pkg_id, meta.1.as_ref(), mir))
        })
        .collect();

    biwac_mir_transform::monomorphize(biwac_mir_transform::MonoInput {
        hir,
        own,
        deps: &deps,
        roots: &roots,
        interner,
    })
    .map_err(|errors| {
        eprintln!("Error: monomorphization failed:");
        for e in &errors {
            eprintln!("  {e}");
        }
        biwac_base::print_error_finish_message(errors.len());
    })
}

/// `.biwamir` の内容のハッシュ。
///
/// 依存の本体が変わったかどうかの判定に使う。
/// エンコードは決定論的なので、同じ MIR なら同じ値になる。
fn mir_hash(text: &str) -> Hash64 {
    use biwac_hash::StableHasher64;
    let mut h = StableHasher64::new();
    h.write_str(text);
    h.finish()
}

/// このパッケージの [`PackageId`]。
///
/// メモリ上では自パッケージのシンボルは `SELF` を持つが、
/// ディスクに書くときは他のパッケージと同じ土俵に載せる必要がある。
/// 依存側が振るのと同じ規則 (`(name, version)` のハッシュ) で導出する。
fn self_package_id(metadata: &biwac_base::PackageMetadata) -> PackageId {
    biwac_span::PackageHashId::new(&metadata.name, &metadata.version).as_package_id()
}

/// 依存の `.biwamir` をすべて読む。
fn load_dep_mirs(
    deps: &[(PackageId, String, Hash64)],
    packages_dir: &Path,
    interner: &mut IdentInterner,
    depth: DisplayDepth,
) -> Result<Vec<(PackageId, biwac_mir::Mir)>, ()> {
    if deps.is_empty() {
        return Ok(Vec::new());
    }
    let mut out = Vec::with_capacity(deps.len());
    let mut items = 0;
    for (pkg_id, name, svh) in deps {
        let root = packages_dir.join(name);
        let mir = load_dep_mir(&root, name, *svh, interner).map_err(|_| {
            biwac_base::print_error_finish_message(1);
        })?;
        items += mir.items.len();
        out.push((*pkg_id, mir));
    }
    println!(
        "{}{} {} dependency MIR file(s), {} item(s)",
        depth.indent(),
        "Loaded".cyan().bold(),
        deps.len(),
        items,
    );
    Ok(out)
}

/// 依存パッケージの `.biwamir` を読む。
///
/// `.biwameta` と対で書かれているので、対応が崩れていないかをここで確かめる。
/// `.biwamir` はシンボルを `.biwameta` の索引で参照しており、
/// 索引がずれれば SVH も変わるので、SVH の照合で誤読を止められる
/// (rustc が `CrateDep { name, hash: Svh }` でやっているのと同じ)。
fn load_dep_mir(
    dep_root: &Path,
    dep_name: &str,
    expected_meta_svh: Hash64,
    interner: &mut IdentInterner,
) -> Result<biwac_mir::Mir, ()> {
    let path = dep_root
        .join(biwac_base::BIWA_BUILD_DIRECTORY_NAME)
        .join(format!("{dep_name}.{}", biwac_mir::MIR_FILE_EXTENSION));

    let text = std::fs::read_to_string(&path).map_err(|e| {
        eprintln!("Error: failed to read {:?}: {}", path, e);
    })?;

    let decoded = biwac_mir::decode(&text, interner).map_err(|e| {
        eprintln!("Error: failed to decode {:?}: {}", path, e);
    })?;

    if decoded.meta_svh != expected_meta_svh {
        eprintln!(
            "Error: {:?} was built against a different `{dep_name}.biwameta` \
             (recorded {}, found {})",
            path, decoded.meta_svh, expected_meta_svh
        );
        return Err(());
    }

    Ok(decoded.mir)
}

/// パッケージ内の全モジュールに属性検証パスを走らせる。
///
/// モジュール木の走査はここが持つ。
/// biwac_attribute は biwac_ast までしか知らない
/// (biwac_package_loader は biwac_parser に依存しており、
///  そこに依存させると循環する)。
fn check_attributes(
    pkg: &biwac_package_loader::Pkg,
    interner: &biwac_base::IdentInterner,
    srcs: &biwac_base::SourceHolder,
    metadata: &biwac_base::MetadataHolder,
) -> Result<(), ()> {
    let mut errors = Vec::new();
    pkg.walk_modules(|module| {
        biwac_attribute::check_mod_ast(&module.ast, interner, &mut errors);
    });

    if errors.is_empty() {
        return Ok(());
    }

    print_errors(&errors, interner, srcs, metadata);

    Err(())
}

/// 収集済みのエラーをまとめて表示する。
fn print_errors<E: biwac_base::BiwacError>(
    errors: &[E],
    interner: &biwac_base::IdentInterner,
    srcs: &biwac_base::SourceHolder,
    metadata: &biwac_base::MetadataHolder,
) {
    let ctx = biwac_base::ErrorContext {
        metadata,
        srcs,
        interner,
    };
    for e in errors {
        e.print_error_message(&ctx);
    }
    biwac_base::print_error_finish_message(errors.len());
}

/// 依存パッケージの生成物を自パッケージの出力ディレクトリに集める。
///
/// 各パッケージは自分の .biwa_build/typescript/<name>.ts に出力するが、
/// codegen が生成する import は `./<package>.ts` という相対パスなので、
/// ルートパッケージの出力ディレクトリに推移的依存も含めて並べる必要がある。
///
/// 再ビルドしたものだけでなく **依存グラフの全パッケージ** が対象である。
/// キャッシュが効いた依存の .ts も並んでいなければならないし、
/// 依存から外れたパッケージの .ts は取り除かなければならない。
fn collect_dep_bins(
    dep_names: &[String],
    self_pkg_name: &str,
    packages_dir: &Path,
    build_dir_path: &Path,
) -> Result<(), ()> {
    if !cfg!(feature = "typescript") {
        return Ok(());
    }

    let dst_dir = build_dir_path.join("typescript");
    std::fs::create_dir_all(&dst_dir).map_err(|e| {
        eprintln!("Error: failed to create {:?}: {}", dst_dir, e);
    })?;

    for dep_name in dep_names {
        let file_name = format!("{}.ts", dep_name);
        let src = packages_dir
            .join(dep_name)
            .join(biwac_base::BIWA_BUILD_DIRECTORY_NAME)
            .join("typescript")
            .join(&file_name);
        let dst = dst_dir.join(&file_name);

        std::fs::copy(&src, &dst).map_err(|e| {
            eprintln!("Error: failed to copy {:?} to {:?}: {}", src, dst, e);
        })?;
    }

    // グラフから消えたパッケージの生成物を掃除する。
    // 残したままだと、依存を外したのに古いコードが出力に紛れ続ける。
    let mut keep: HashSet<String> = dep_names.iter().map(|n| format!("{n}.ts")).collect();
    keep.insert(format!("{self_pkg_name}.ts"));

    let Ok(entries) = std::fs::read_dir(&dst_dir) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("ts") {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !keep.contains(name) {
            let _ = std::fs::remove_file(&path);
        }
    }

    Ok(())
}

fn write_bin(
    build_dir_path: PathBuf,
    pkg_name: &PackageName,
    bin: &str,
) -> Result<(), std::io::Error> {
    if cfg!(feature = "typescript") {
        let dstpath = build_dir_path.join(Path::new("typescript"));
        if !dstpath.exists() {
            std::fs::DirBuilder::new()
                .recursive(true)
                .create(dstpath.clone())
                .unwrap();
        } else if !dstpath.is_dir() {
            panic!(
                "Destination directory broken, conflicted file found: `{}`",
                dstpath
                    .as_os_str()
                    .to_str()
                    .expect("broken build directory path")
            );
        }

        let binpath = dstpath.join(Path::new(&format!("{}.ts", pkg_name.value())));

        let mut f = std::fs::File::create(binpath).unwrap();

        f.write_all(bin.as_bytes())
    } else {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::HashSet;
    use std::path::{Path, PathBuf};
    use std::sync::{Mutex, OnceLock};

    use biwac_base::IdentInterner;
    use biwac_hir::TyKind;
    use biwac_mir::{MirItem, MonoMir, MonoTyDefKind};

    use crate::{BuildOptions, compile};

    thread_local! {
        /// 直近の単相化の結果。
        ///
        /// 単相化の結果はファイルにしないので、テストはここから受け取る。
        pub(super) static LAST_MONO: RefCell<Option<MonoMir>> = const { RefCell::new(None) };
    }

    /// `--emit mir` でビルドし、単相化の結果を返す。
    ///
    /// 単相化は playable パッケージでしか走らないので、
    /// ライブラリに対して呼ぶと `None` になる。
    fn monomorphize(pkg: &str) -> Option<MonoMir> {
        with_build_lock(|built| {
            // 結果はスレッドローカルに置かれるので、同じスレッドで建てて取り出す。
            LAST_MONO.with(|slot| *slot.borrow_mut() = None);
            emit_mir_build(pkg);
            built.insert(pkg.to_string());
            LAST_MONO.with(|slot| slot.borrow_mut().take())
        })
    }

    /// ビルドは常にこの中で行う。
    ///
    /// 同じパッケージを 2 つのテストが同時に建てると
    /// 出力ファイルの書き込みがぶつかるので、直列化する。
    /// `built` には一度建てたパッケージが入り、無駄な建て直しを省く。
    fn with_build_lock<T>(f: impl FnOnce(&mut HashSet<String>) -> T) -> T {
        static BUILT: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
        let built = BUILT.get_or_init(|| Mutex::new(HashSet::new()));
        let mut built = built.lock().unwrap_or_else(|e| e.into_inner());
        f(&mut built)
    }

    fn emit_mir_build(pkg: &str) {
        compile(
            Path::new("../../assets/tests").join(pkg),
            BuildOptions {
                force_rebuild: true,
                emit_mir: true,
            },
        )
        .expect("MIR emission failed");
    }

    /// まだ建てていなければ建てる。
    fn build_once(pkg: &str) {
        with_build_lock(|built| {
            if built.insert(pkg.to_string()) {
                emit_mir_build(pkg);
            }
        });
    }

    fn build_dir(pkg: &str) -> PathBuf {
        Path::new("../../assets/tests")
            .join(pkg)
            .join(biwac_base::BIWA_BUILD_DIRECTORY_NAME)
    }

    /// `--emit mir` でビルドし、書かれた `.biwamir` を返す。
    fn emit_mir_of(pkg: &str) -> String {
        build_once(pkg);
        read_mir(pkg)
    }

    fn read_mir(pkg: &str) -> String {
        std::fs::read_to_string(build_dir(pkg).join(format!("{pkg}.biwamir")))
            .expect("MIR was not written")
    }

    // MIR でしか扱えない構文を含むフィクスチャ。
    //
    // TypeScript の codegen は while 文とブロック文が todo!() のままなので、
    // このパッケージは `--emit mir` でしかビルドできない。
    #[test]
    fn mir_fixture() {
        let mir = emit_mir_of("mir_fixture");

        // while: ループ頭へ戻る後方辺ができる。
        // validator が簡約可能性まで見ているので、
        // ここを通っている時点で後方辺の行き先はループ頭である。
        assert!(mir.contains("switch "), "{mir}");
        assert!(mir.contains("goto "), "{mir}");

        // simplify_cfg が掛かっていること。
        // 文の無い `goto` だけのブロックは畳まれて残らない。
        let empty_goto_blocks = mir
            .lines()
            .zip(mir.lines().skip(1))
            .filter(|(a, b)| {
                a.trim_start().starts_with("bb ") && b.trim_start().starts_with("goto ")
            })
            .count();
        assert_eq!(
            empty_goto_blocks, 0,
            "simplify_cfg should leave no empty goto block:\n{mir}"
        );

        // メンバへの代入は Place の射影になる。
        assert!(mir.contains("_1.count@"), "{mir}");
        // struct literal は集約になる。
        assert!(mir.contains("= agg "), "{mir}");
        // 二項演算は「代入先 = 演算子 被演算子 被演算子」。
        assert!(mir.contains(" = add "), "{mir}");
    }

    // 依存パッケージと scene を含むパッケージ。
    #[test]
    fn test1() {
        let mir = emit_mir_of("test1");

        // scene は普通の関数として落ちる。中断は現れない。
        assert!(!mir.contains("yield"), "{mir}");
        // novel 文は lang item への通常の呼び出しになる。文字列定数を渡している。
        assert!(mir.contains("str:"), "{mir}");

        // 自パッケージのシンボルは `.biwameta` の索引に読み替えられ、
        // ディスク上に SELF (別名の無いパッケージ) は現れない。
        assert!(mir.contains("pkg 0 test1 "), "{mir}");
    }

    /// `.biwamir` を読み戻して書き直すと、元の文字列に一致すること。
    ///
    /// decode の結果は既に `(本当の PackageId, シンボル索引)` の空間にいるので、
    /// 書き直すときの読み替えは恒等になる。
    #[test]
    fn round_trip() {
        // test1 のビルドで std / color / greeter の .biwamir も書かれる。
        build_once("test1");

        for pkg in ["std", "color", "greeter", "test1"] {
            let text = read_mir(pkg);
            let mut interner = IdentInterner::new();
            let decoded = biwac_mir::decode(&text, &mut interner)
                .unwrap_or_else(|e| panic!("failed to decode {pkg}.biwamir: {e}"));

            let again = biwac_mir::encode(
                &decoded.mir,
                &biwac_mir::EncodeCtx {
                    pkg_id: decoded.mir.pkg_id,
                    meta_svh: decoded.meta_svh,
                    // 読み戻した MIR に SELF は無いので、読み替えの表は要らない。
                    symbols: None,
                    interner: &interner,
                },
            );

            assert_eq!(again, text, "{pkg}.biwamir did not round trip");
        }
    }

    /// `.biwamir` から復元した DefId が、`.biwameta` 経由で得られる DefId と一致すること。
    ///
    /// これが崩れると、MIR 上のシンボルと HIR 上のシンボルが別物になり、
    /// 単相化のときに型定義もシグニチャも引けなくなる。
    #[test]
    fn def_ids_match_metadata() {
        build_once("test1");

        let mut interner = IdentInterner::new();

        // greeter の MIR に現れる `std::types::string::String` の TyDefId を取る。
        // greeter::greet は (String) -> String なので、その引数の型がそれである。
        let greeter_text = read_mir("greeter");
        let greeter = biwac_mir::decode(&greeter_text, &mut interner)
            .expect("failed to decode greeter.biwamir")
            .mir;

        // greeter が参照している「greeter 以外のパッケージの型」を集める。
        let mut foreign_tys = std::collections::BTreeSet::new();
        for item in greeter.items.values() {
            let biwac_mir::MirItem::Body(body) = item else {
                continue;
            };
            for local in &body.locals {
                if let biwac_hir::TyKind::Defined(dt) = &local.ty.kind
                    && dt.def_id.pkg() != greeter.pkg_id
                {
                    foreign_tys.insert(dt.def_id.value());
                }
            }
        }
        assert!(
            !foreign_tys.is_empty(),
            "greeter should refer to types from std / color"
        );

        // 同じ型を、依存メタデータ側の経路 (test1 が使っているもの) からも引く。
        // MIR は新しい id を振らないので、両者は同じ値になっていなければならない。
        let meta_path = build_dir("greeter").join("greeter.biwameta");
        let data = std::fs::read(&meta_path).expect("greeter.biwameta is missing");
        let meta = biwac_dependency_metadata::DepMetadata::decode_file(&data)
            .expect("failed to decode greeter.biwameta");

        // greeter.biwamir に書かれた SVH が、いま読んだメタデータのものと一致すること。
        let decoded = biwac_mir::decode(&greeter_text, &mut interner).unwrap();
        assert_eq!(
            decoded.meta_svh, meta.svh,
            "greeter.biwamir was built against a different greeter.biwameta"
        );

        // color::Rgb が greeter の MIR に現れること。
        // test1 は color に依存していないので、
        // 推移閉包のメタデータが揃っていないとこの型は復元できない。
        let color_meta_path = build_dir("color").join("color.biwameta");
        let color_data = std::fs::read(&color_meta_path).expect("color.biwameta is missing");
        let color_meta = biwac_dependency_metadata::DepMetadata::decode_file(&color_data).unwrap();
        let color_pkg = biwac_span::PackageHashId::new(
            &biwac_metadata_loader::try_load_package_metadata(
                Path::new("../../assets/tests/color").to_path_buf(),
            )
            .unwrap()
            .metadata
            .name,
            &biwac_metadata_loader::try_load_package_metadata(
                Path::new("../../assets/tests/color").to_path_buf(),
            )
            .unwrap()
            .metadata
            .version,
        )
        .as_package_id();
        let _ = color_meta;

        assert!(
            foreign_tys
                .iter()
                .any(|v| (*v >> 32) as u32 == color_pkg.value()),
            "greeter's MIR should mention a type owned by color"
        );
    }

    /// 単相化がジェネリクスを消し、到達可能なものだけを残すこと。
    ///
    /// 結果はファイルにしないので、構造をそのまま見る。
    #[test]
    fn monomorphization() {
        let mono = monomorphize("test1").expect("test1 is playable");

        // 同じ関数が複数の実体を持つこと。
        //
        // `scene main` -> `foo()` は `Pair::new(l, z)` と `Pair::new(x, ...)` を呼び、
        // 前者は [Line, Int]、後者は [Int, Int] になる。
        // 名前を引く経路をテストに持ち込みたくないので、
        // 「2 つ以上の実体を持つ def_id」として見る。
        let mut by_def: std::collections::HashMap<biwac_span::ValDefId, Vec<&biwac_mir::GenArgs>> =
            std::collections::HashMap::new();
        for inst in &mono.instances {
            by_def
                .entry(inst.key.def_id)
                .or_default()
                .push(&inst.key.args);
        }
        let multi: Vec<_> = by_def.iter().filter(|(_, v)| v.len() > 1).collect();
        assert_eq!(
            multi.len(),
            1,
            "exactly one function (Pair::new) should have several instances, got {:?}",
            multi
                .iter()
                .map(|(k, v)| (k.value(), v.len()))
                .collect::<Vec<_>>()
        );
        let (_, args) = multi[0];
        assert_eq!(args.len(), 2, "Pair::new should have two instances");
        assert_ne!(args[0], args[1], "the two instances must differ");

        // どの実体にもジェネリック型が残っていないこと。
        for inst in &mono.instances {
            assert!(
                inst.key.args.iter().all(|(_, ty)| is_concrete(ty)),
                "instance val#{} still has a generic argument",
                inst.key.def_id.value()
            );
            let MirItem::Body(body) = &inst.item else {
                continue;
            };
            assert!(
                body.genargs.is_empty(),
                "a monomorphized body must not declare generic arguments"
            );
            for (i, local) in body.locals.iter().enumerate() {
                assert!(
                    is_concrete(&local.ty),
                    "local _{i} of val#{} is not concrete: {:?}",
                    inst.key.def_id.value(),
                    local.ty.kind
                );
            }
        }

        // 推移的依存 (color) の型が、メンバ付きで並んでいること。
        // test1 は color に依存していないので、
        // 推移閉包の `.biwamir` / `.biwameta` を辿れていないと出てこない。
        //
        // color には struct が Rgb しか無く、メンバは r / g / b の 3 つである。
        let color_pkg = package_id_of("color");
        let rgb = mono.types.iter().find(|t| t.key.def_id.pkg() == color_pkg);
        assert!(
            rgb.is_some(),
            "a type owned by color should be among the monomorphized types"
        );
        let MonoTyDefKind::Struct { members } = &rgb.unwrap().kind else {
            panic!("color::Rgb should be a struct");
        };
        assert_eq!(members.len(), 3, "color::Rgb has r / g / b");

        // 到達しない関数は入らない。
        // test1 自身の `.biwamir` に載っている item のほうが多いはずである
        // (`Line::len` や `Pair::y_int` などは誰からも呼ばれていない)。
        let own_instances = mono
            .instances
            .iter()
            .filter(|i| i.key.def_id.pkg().is_self())
            .count();
        let own_items = read_mir("test1")
            .lines()
            .filter(|l| l.starts_with("fn ") || l.starts_with("native "))
            .count();
        assert!(
            own_instances < own_items,
            "unreachable functions must be dropped ({own_instances} instances vs {own_items} items)"
        );

        // エントリポイントが自パッケージの scene であること。
        let entry = mono.entry_instance().expect("entry point");
        assert!(entry.key.def_id.pkg().is_self());
        assert!(
            entry.key.args.is_empty(),
            "a scene takes no generic argument"
        );

        // 2 回走らせて同じ並びになること。
        // 実体の索引がそのまま番号になるので、順序が揺れると成果物も揺れる。
        let again = monomorphize("test1").expect("test1 is playable");
        let keys = |m: &MonoMir| {
            format!(
                "{:?}",
                m.instances.iter().map(|i| &i.key).collect::<Vec<_>>()
            )
        };
        assert_eq!(keys(&mono), keys(&again), "instance order must be stable");
        let ty_keys =
            |m: &MonoMir| format!("{:?}", m.types.iter().map(|t| &t.key).collect::<Vec<_>>());
        assert_eq!(ty_keys(&mono), ty_keys(&again), "type order must be stable");
    }

    /// ライブラリでは単相化が走らないこと (根が無い)。
    #[test]
    fn library_is_not_monomorphized() {
        assert!(
            monomorphize("mir_fixture").is_none(),
            "a library package has no entry point, so nothing is monomorphized"
        );
        // `.biwamir` は今までどおり書かれる。
        assert!(!read_mir("mir_fixture").is_empty());
    }

    fn package_id_of(pkg: &str) -> biwac_base::PackageId {
        let metadata = biwac_metadata_loader::try_load_package_metadata(
            Path::new("../../assets/tests").join(pkg),
        )
        .unwrap();
        biwac_span::PackageHashId::new(&metadata.metadata.name, &metadata.metadata.version)
            .as_package_id()
    }

    fn is_concrete(ty: &biwac_hir::Ty) -> bool {
        match &ty.kind {
            TyKind::Int | TyKind::Float | TyKind::Bool | TyKind::Void => true,
            TyKind::Gen(_) | TyKind::LocGen(_) | TyKind::Infer(_) => false,
            TyKind::Defined(dt) => dt.genargs.iter().all(is_concrete),
            TyKind::Fn(f) => f.args.iter().all(is_concrete) && is_concrete(&f.rty),
        }
    }

    /// `.biwamir` と `.biwameta` の対応が崩れていたら読み込みで止まること。
    #[test]
    fn detects_metadata_mismatch() {
        build_once("test1");

        let text = read_mir("greeter");
        let broken = text
            .lines()
            .map(|l| {
                if l.starts_with("meta-svh") {
                    "meta-svh 0000000000000000".to_string()
                } else {
                    l.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        let mut interner = IdentInterner::new();
        let decoded = biwac_mir::decode(&broken, &mut interner).expect("still decodable");

        let data = std::fs::read(build_dir("greeter").join("greeter.biwameta")).unwrap();
        let meta = biwac_dependency_metadata::DepMetadata::decode_file(&data).unwrap();

        assert_ne!(
            decoded.meta_svh, meta.svh,
            "the mismatch must be visible to the loader"
        );
    }
}
