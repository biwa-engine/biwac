mod dep_graph;

use colored::Colorize;
use std::{
    collections::HashMap,
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
};

use biwac_base::{IdentInterner, PackageName, SourceHolder};
use biwac_dependency_metadata::{DepMetadata, ExternalPackage};

use dep_graph::DepGraph;

pub fn compile(pkg_root_path: PathBuf) -> Result<(), ()> {
    println!("{}", "Compiling...".green().bold(),);

    let mut interner = IdentInterner::new();

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

    // build directory preparation
    let build_dir_path = pkg_root_path.join(Path::new(biwac_base::BIWA_BUILD_DIRECTORY_NAME));
    if !build_dir_path.exists() {
        std::fs::DirBuilder::new()
            .recursive(true)
            .create(build_dir_path.clone())
            .unwrap();
    } else if !build_dir_path.is_dir() {
        panic!(
            "Destination directory broken, conflicted file found: `{}`",
            build_dir_path
                .as_os_str()
                .to_str()
                .expect("broken build directory path")
        );
    }

    // Dependency building via DepGraph (BFS discovery + Kahn's topological batching)
    //
    // packages_dir: sibling directory of pkg_root_path (workspace root).
    // Each package lives at packages_dir/<name>/.
    let packages_dir = pkg_root_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));

    let root_dep_names: Vec<String> = metadata
        .metadata
        .dependencies
        .iter()
        .map(|d| d.name.value().to_string())
        .collect();

    let external_packages = if root_dep_names.is_empty() {
        Vec::new()
    } else {
        let root_dep_refs: Vec<&str> = root_dep_names.iter().map(|s| s.as_str()).collect();

        // Discover full transitive dependency graph.
        let dep_graph = DepGraph::discover(&root_dep_refs, &packages_dir).map_err(|_| {
            biwac_base::print_error_finish_message(1);
        })?;

        // Build in topological order (leaves = no deps first).
        // Items within the same batch are independent and can be parallelized (future tokio).
        let batches = dep_graph.topo_batches().map_err(|_| {
            biwac_base::print_error_finish_message(1);
        })?;
        if !batches.is_empty() {
            println!("{}", "Compiling dependencies...".green().bold(),);
        }
        let mut built_deps: Vec<String> = Vec::new();
        for batch in &batches {
            // TODO: parallelize within batch using tokio

            for dep_name in batch {
                let dep_root = packages_dir.join(dep_name);
                build_single_dep(dep_root, dep_name)?;
                built_deps.push(dep_name.clone());
            }
        }
        if !batches.is_empty() {
            println!("{}", "Compiling dependencies finished!".green().bold(),);
        }

        // 生成物は import 文が `./<package>.ts` を指すため、
        // 自パッケージと推移的依存の .ts が同じディレクトリに並んでいる必要がある。
        collect_dep_bins(&built_deps, &packages_dir, &build_dir_path)?;

        load_external_packages(&dep_graph, &root_dep_names, &packages_dir, &mut interner)?
    };

    load_analyze_and_codegen_single_package(
        external_packages,
        &mut interner,
        &metadata,
        pkg_root_path,
        build_dir_path,
    )?;

    println!("{}", "Finished!".green().bold(),);

    Ok(())
}

/// Builds a single dependency if its .biwameta is not already up to date.
fn build_single_dep(dep_root: PathBuf, dep_name: &str) -> Result<(), ()> {
    let meta_path = dep_root
        .join(biwac_base::BIWA_BUILD_DIRECTORY_NAME)
        .join(format!("{}.biwameta", dep_name));
    if meta_path.exists() {
        return Ok(()); // cached
    }

    // compiling single dependency

    let mut interner = IdentInterner::new();

    let metadata =
        biwac_metadata_loader::try_load_package_metadata(dep_root.clone()).map_err(|e| {
            e.print_error_message();
            biwac_base::print_error_finish_message(1);
        })?;

    println!(
        "  {} {} v{}.{}.{}",
        "Compiling...".green().bold(),
        metadata.metadata.name.value(),
        metadata.metadata.version.major(),
        metadata.metadata.version.minor(),
        metadata.metadata.version.patch()
    );

    // build directory preparation
    let build_dir_path = dep_root.join(Path::new(biwac_base::BIWA_BUILD_DIRECTORY_NAME));
    std::fs::create_dir_all(&build_dir_path).map_err(|e| {
        eprintln!("Error: failed to create {:?}: {}", build_dir_path, e);
        biwac_base::print_error_finish_message(1);
    })?;

    // Dependency building via DepGraph (BFS discovery + Kahn's topological batching)
    //
    // packages_dir: sibling directory of pkg_root_path (workspace root).
    // Each package lives at packages_dir/<name>/.
    let packages_dir = dep_root
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));

    let root_dep_names: Vec<String> = metadata
        .metadata
        .dependencies
        .iter()
        .map(|d| d.name.value().to_string())
        .collect();

    let external_packages = if root_dep_names.is_empty() {
        Vec::new()
    } else {
        // このパッケージの依存はトポロジカル順で既にビルド済みなので、ロードするだけでよい。
        // ただしグラフは自分で引き直す:
        // 直接依存の .biwameta が、さらにその依存の型を参照している可能性があり、
        // それを解決するには推移閉包すべての ID とメタデータが要る。
        let root_dep_refs: Vec<&str> = root_dep_names.iter().map(|s| s.as_str()).collect();
        let dep_graph = DepGraph::discover(&root_dep_refs, &packages_dir).map_err(|_| {
            biwac_base::print_error_finish_message(1);
        })?;

        load_external_packages(&dep_graph, &root_dep_names, &packages_dir, &mut interner)?
    };

    load_analyze_and_codegen_single_package(
        external_packages,
        &mut interner,
        &metadata,
        dep_root,
        build_dir_path,
    )?;

    println!("    -> {}", "Finished!".green().bold(),);

    Ok(())
}

/// 依存グラフの推移閉包すべてに PackageId を振り、`.biwameta` をロードする。
///
/// 直接依存だけでは足りない。依存の `.biwameta` に載っているシグニチャが
/// さらにその依存の型を参照していることがあり
/// (`greeter::theme() -> color::Rgb`)、
/// その参照を DefId に復元するには相手の ID とメタデータが要るからである。
/// 名前で引ける (= import できる) のは直接依存だけなので、
/// [`ExternalPackage::direct`] で区別する。
///
/// PackageId はここが単一の割り当て元である。
/// 名前順に振るので、同じグラフからは常に同じ採番になる。
fn load_external_packages(
    dep_graph: &DepGraph,
    root_dep_names: &[String],
    packages_dir: &Path,
    interner: &mut IdentInterner,
) -> Result<Vec<ExternalPackage>, ()> {
    let all_names = dep_graph.all_packages();

    let pkg_ids: HashMap<String, biwac_base::PackageId> = all_names
        .iter()
        .enumerate()
        .map(|(i, name)| {
            (
                name.clone(),
                biwac_base::PackageId::new(
                    i as u32 + biwac_base::PackageId::UNRESERVED_PACKAGE_MIN,
                ),
            )
        })
        .collect();

    let mut packages = Vec::with_capacity(all_names.len());
    for name in &all_names {
        let dep_root = packages_dir.join(name);
        let meta = load_dep_metadata(&dep_root, name, &pkg_ids)?;
        packages.push(ExternalPackage {
            ident: interner.get_or_insert(name),
            pkg_id: pkg_ids[name],
            meta: Arc::new(meta),
            direct: root_dep_names.iter().any(|d| d == name),
        });
    }

    Ok(packages)
}

/// Loads a .biwameta file from a built dependency's build directory.
///
/// `pkg_ids` はファイル内の依存パッケージ表を今回の採番へ束縛するために使う。
fn load_dep_metadata(
    dep_root: &Path,
    dep_name: &str,
    pkg_ids: &HashMap<String, biwac_base::PackageId>,
) -> Result<DepMetadata, ()> {
    let meta_path = dep_root
        .join(biwac_base::BIWA_BUILD_DIRECTORY_NAME)
        .join(format!("{}.biwameta", dep_name));
    let data = std::fs::read(&meta_path).map_err(|e| {
        eprintln!("Error: failed to read {:?}: {}", meta_path, e);
    })?;
    DepMetadata::decode_file(&data, pkg_ids).map_err(|e| {
        eprintln!("Error: failed to decode {:?}: {}", meta_path, e);
    })
}

// Persist self package's symbol metadata to disk for dependents.
fn persist_dep_metadata(
    hir: &biwac_hir::Hir,
    lang_items: &biwac_lang_item::LangItemTable,
    srcs: &biwac_base::SourceHolder,
    interner: &biwac_base::IdentInterner,
    build_dir_path: PathBuf,
    metadata: &biwac_base::MetadataHolder,
) -> Result<(), ()> {
    let dep_meta = DepMetadata::new(hir, srcs, interner, lang_items);
    let meta_bytes = dep_meta.encode_file();
    let meta_path = build_dir_path.join(format!("{}.biwameta", metadata.metadata.name.value()));
    std::fs::write(&meta_path, meta_bytes).map_err(|e| {
        eprintln!("Error: failed to write {:?}: {}", meta_path, e);
    })
}

fn load_analyze_and_codegen_single_package(
    external_packages: Vec<ExternalPackage>,
    interner: &mut biwac_base::IdentInterner,
    metadata: &biwac_base::MetadataHolder,
    pkg_root_path: PathBuf,
    build_dir_path: PathBuf,
) -> Result<(), ()> {
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
    persist_dep_metadata(
        &hir,
        &lang_items,
        &srcs,
        interner,
        build_dir_path.clone(),
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

    Ok(())
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
fn collect_dep_bins(
    dep_names: &[String],
    packages_dir: &Path,
    build_dir_path: &Path,
) -> Result<(), ()> {
    if !cfg!(feature = "typescript") {
        return Ok(());
    }

    let dst_dir = build_dir_path.join("typescript");
    if dep_names.is_empty() {
        return Ok(());
    }
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
