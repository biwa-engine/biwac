mod dep_graph;

use colored::Colorize;
use std::{
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
};

use biwac_base::{IdentInterner, PackageName, SourceHolder};
use biwac_dependency_metadata::DepMetadata;

use dep_graph::DepGraph;

pub fn compile(pkg_root_path: PathBuf) -> Result<(), ()> {
    println!("{}", "Compiling...".green().bold(),);

    let mut srcs = SourceHolder::default();
    let mut interner = IdentInterner::new();

    let metadata = biwac_metadata_loader::try_load_package_metadata(pkg_root_path.clone())
        .map_err(|e| {
            e.print_error_message();
            biwac_base::print_error_finish_message(1);
        })?;
    let package_name_interned = interner.get_or_insert(metadata.metadata.name.value());

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

    let external_packages: Vec<(biwac_base::InternedIdent, Arc<DepMetadata>)> =
        if !root_dep_names.is_empty() {
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
            for batch in &batches {
                // TODO: parallelize within batch using tokio
                for dep_name in batch {
                    let dep_root = packages_dir.join(dep_name);
                    build_single_dep(dep_root, dep_name)?;
                }
            }

            // Load .biwameta for direct (root-level) dependencies only.
            let mut ext_pkgs = Vec::new();
            for dep_name in &root_dep_names {
                let dep_root = packages_dir.join(dep_name);
                let dep_meta = load_dep_metadata(&dep_root, dep_name)?;
                let dep_ident = interner.get_or_insert(dep_name);
                ext_pkgs.push((dep_ident, Arc::new(dep_meta)));
            }
            ext_pkgs
        } else {
            Vec::new()
        };

    let pkg = biwac_package_loader::Pkg::try_load(
        &metadata,
        &mut interner,
        &mut srcs,
        pkg_root_path.to_path_buf(),
    )
    .map_err(|e| e.print_error_messages())?;

    println!("{interner:#?}");

    let hir = biwac_name_resolver::NameResolver::new(
        &metadata,
        external_packages,
        package_name_interned,
        pkg,
    )
    .unwrap()
    .try_resolve()
    .unwrap();

    // Persist self package's symbol metadata to disk for dependents.
    let dep_meta = DepMetadata::new(&hir, &srcs, &interner);
    let meta_bytes = dep_meta.encode_file();
    let meta_path = build_dir_path.join(format!("{}.biwameta", metadata.metadata.name.value()));
    std::fs::write(&meta_path, meta_bytes).map_err(|e| {
        eprintln!("Error: failed to write {:?}: {}", meta_path, e);
    })?;

    let hir = biwac_type_inferrer::TyCtx::new(hir).infer().unwrap();

    let bin = biwac_generator::arch::typescript::generate(&hir, &interner, &srcs);

    write_bin(build_dir_path.to_path_buf(), &metadata.metadata.name, &bin).unwrap();

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
    compile(dep_root)
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
        eprintln!("Error: failed to decode {:?}: {:?}", meta_path, e);
    })
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
