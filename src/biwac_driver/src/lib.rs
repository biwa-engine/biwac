mod dep_graph;

use colored::Colorize;
use std::{
    collections::HashSet,
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
};

use biwac_base::{IdentInterner, PackageId, PackageName, SourceHolder};
use biwac_dependency_metadata::{DepMetadata, ExternalPackage};
use biwac_fingerprint::{Fingerprint, Freshness, SourceEntry, StaleReason};
use biwac_hash::Hash64;

use dep_graph::DepGraph;

/// ビルドの振る舞いの指定。
#[derive(Debug, Clone, Copy, Default)]
pub struct BuildOptions {
    /// 鮮度判定を飛ばして全パッケージを建て直す。
    pub force_rebuild: bool,
}

/// このコンパイラの同一性。
///
/// これが前回と違えばキャッシュはすべて無効になる。
/// `.biwameta` の形式版数を混ぜてあるので、形式を変えたときは
/// 「古い成果物を掴んでエラー」ではなく「フィンガープリント不一致で建て直し」になる。
fn compiler_identity() -> Hash64 {
    biwac_fingerprint::compiler_hash(&[
        biwac_dependency_metadata::BIWAC_DEPENDENCY_METADATA_FORMAT_VERSION,
    ])
}

/// 依存グラフの各パッケージの、今回のビルドで確定した SVH。
///
/// トポロジカル順 (葉から) に埋まっていくので、
/// あるパッケージを判定する時点で、その依存の SVH は必ず揃っている。
type SvhMap = std::collections::HashMap<PackageId, Hash64>;

pub fn compile(pkg_root_path: PathBuf, options: BuildOptions) -> Result<(), ()> {
    println!("{}", "Compiling...".green().bold(),);

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

    let mut svhs = SvhMap::new();
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
    collect_dep_bins(
        &dep_graph.all_packages(),
        metadata.metadata.name.value(),
        &packages_dir,
        &build_dir_path,
    )?;

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
    svhs: &SvhMap,
    options: BuildOptions,
    depth: DisplayDepth,
) -> Result<Hash64, ()> {
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
    let dep_svhs: Vec<(PackageId, Hash64)> = transitive_deps
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
        &dep_svhs,
        &sources,
        options,
    );

    if let Freshness::Fresh(svh) = freshness {
        println!(
            "{}{} {} v{}.{}.{}",
            depth.indent(),
            "Fresh".cyan().bold(),
            pkg_name,
            metadata.metadata.version.major(),
            metadata.metadata.version.minor(),
            metadata.metadata.version.patch(),
        );
        return Ok(svh);
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

    let svh = load_analyze_and_codegen_single_package(
        external_packages,
        &mut interner,
        &metadata,
        &dep_svhs,
        pkg_root,
        build_dir_path.clone(),
    )?;

    // 次回の鮮度判定のために、今回のビルドの状態を記録する。
    let fingerprint = Fingerprint::of_build(
        compiler_identity(),
        &metadata.metadata,
        svh,
        &dep_svhs,
        sources,
    );
    let fp_path = biwac_fingerprint::fingerprint_path(&build_dir_path, &pkg_name);
    std::fs::write(&fp_path, fingerprint.encode_file()).map_err(|e| {
        eprintln!("Error: failed to write {:?}: {}", fp_path, e);
        biwac_base::print_error_finish_message(1);
    })?;

    Ok(svh)
}

/// 前回のビルドから状況が変わっていないかを判定する。
///
/// 判定材料は [`biwac_fingerprint`] に閉じている。
/// ここは「前回の記録を読み出す」ところだけを持つ。
fn check_freshness(
    build_dir_path: &Path,
    pkg_name: &str,
    metadata: &biwac_base::PackageMetadata,
    dep_svhs: &[(PackageId, Hash64)],
    sources: &[SourceEntry],
    options: BuildOptions,
) -> Freshness {
    if options.force_rebuild {
        return Freshness::Stale(StaleReason::Forced);
    }

    // シグニチャのキャッシュ本体が無ければ、記録があっても意味がない。
    if !metadata_path(build_dir_path, pkg_name).exists() {
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

    previous.freshness(compiler_identity(), metadata, dep_svhs, sources)
}

fn metadata_path(build_dir_path: &Path, pkg_name: &str) -> PathBuf {
    build_dir_path.join(format!("{pkg_name}.biwameta"))
}

/// codegen の出力先。
fn bin_path(build_dir_path: &Path, pkg_name: &str) -> PathBuf {
    if cfg!(feature = "typescript") {
        build_dir_path.join("typescript").join(format!("{pkg_name}.ts"))
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
/// 生成した `.biwameta` の SVH を返す。
fn persist_dep_metadata(
    hir: &biwac_hir::Hir,
    lang_items: &biwac_lang_item::LangItemTable,
    srcs: &biwac_base::SourceHolder,
    interner: &biwac_base::IdentInterner,
    dep_svhs: &[(PackageId, Hash64)],
    build_dir_path: &Path,
    metadata: &biwac_base::MetadataHolder,
) -> Result<Hash64, ()> {
    let dep_meta = DepMetadata::new(hir, srcs, interner, lang_items, dep_svhs);
    let svh = dep_meta.svh;
    let meta_bytes = dep_meta.encode_file();
    let meta_path = metadata_path(build_dir_path, metadata.metadata.name.value());
    std::fs::write(&meta_path, meta_bytes)
        .map_err(|e| {
            eprintln!("Error: failed to write {:?}: {}", meta_path, e);
        })
        .map(|_| svh)
}

/// パイプライン本体。生成した `.biwameta` の SVH を返す。
fn load_analyze_and_codegen_single_package(
    external_packages: Vec<ExternalPackage>,
    interner: &mut biwac_base::IdentInterner,
    metadata: &biwac_base::MetadataHolder,
    dep_svhs: &[(PackageId, Hash64)],
    pkg_root_path: PathBuf,
    build_dir_path: PathBuf,
) -> Result<Hash64, ()> {
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
    let svh = persist_dep_metadata(
        &hir,
        &lang_items,
        &srcs,
        interner,
        dep_svhs,
        &build_dir_path,
        metadata,
    )?;

    let hir =
        biwac_type_inferrer::TyCtx::new(hir, lang_items.clone(), ext_pkgs_for_ty.clone(), interner)
            .infer()
            .unwrap();

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

    Ok(svh)
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
