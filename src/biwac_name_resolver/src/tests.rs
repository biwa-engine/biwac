use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use biwac_base::{BiwacError, ErrorContext};

use crate::NameResolver;

/// Loads a .biwameta file from a built dependency's build directory.
fn load_dep_metadata(dep_root: &Path, dep_name: &str) -> biwac_dependency_metadata::DepMetadata {
    let meta_path = dep_root
        .join(biwac_base::BIWA_BUILD_DIRECTORY_NAME)
        .join(format!("{}.biwameta", dep_name));
    let data = std::fs::read(&meta_path).unwrap();
    biwac_dependency_metadata::DepMetadata::decode_file(&data).unwrap()
}

#[test]
fn test1() {
    // assets/tests/test1
    // 以下にbiwaのパッケージのディレクトリがあることを前提とする

    let mut srcs = biwac_base::SourceHolder::default();
    let mut interner = biwac_base::IdentInterner::default();
    let pkg_root_path = Path::new("../../assets/tests/std");
    let pkg_name = interner.get_or_insert("std");

    let metadata =
        biwac_metadata_loader::try_load_package_metadata(pkg_root_path.to_path_buf()).unwrap();

    let build_dir_path = pkg_root_path.join(Path::new(biwac_base::BIWA_BUILD_DIRECTORY_NAME));

    // Dependency building via DepGraph (BFS discovery + Kahn's topological batching)
    //
    // packages_dir: sibling directory of pkg_root_path (workspace root).
    // Each package lives at packages_dir/<name>/.
    let packages_dir = build_dir_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));

    let root_dep_names: Vec<String> = metadata
        .metadata
        .dependencies
        .iter()
        .map(|d| d.name.value().to_string())
        .collect();

    let external_packages: Vec<(
        biwac_base::InternedIdent,
        Arc<biwac_dependency_metadata::DepMetadata>,
    )> = if !root_dep_names.is_empty() {
        // Load .biwameta for direct (root-level) dependencies only.
        let mut ext_pkgs = Vec::new();
        for dep_name in &root_dep_names {
            let dep_root = packages_dir.join(dep_name);
            let dep_meta = load_dep_metadata(&dep_root, dep_name);
            let dep_ident = interner.get_or_insert(dep_name);
            ext_pkgs.push((dep_ident, Arc::new(dep_meta)));
        }
        ext_pkgs
    } else {
        Vec::new()
    };

    // PackageId を driver が単一の割り当て元として決定する。
    let external_packages_with_ids: Vec<(
        biwac_base::InternedIdent,
        biwac_base::PackageId,
        Arc<biwac_dependency_metadata::DepMetadata>,
    )> = external_packages
        .into_iter()
        .enumerate()
        .map(|(i, (ident, dep))| {
            (
                ident,
                biwac_base::PackageId::new(
                    i as u32 + biwac_base::PackageId::UNRESERVED_PACKAGE_MIN,
                ),
                dep,
            )
        })
        .collect();

    let pkg = biwac_package_loader::Pkg::try_load(
        &metadata,
        &mut interner,
        &mut srcs,
        pkg_root_path.to_path_buf(),
    )
    .unwrap();

    let _hir = NameResolver::new(&metadata, external_packages_with_ids, pkg_name, pkg)
        .unwrap()
        .try_resolve(&interner)
        .map_err(|errors| {
            for e in errors {
                e.print_error_message(&ErrorContext {
                    metadata: &metadata,
                    srcs: &srcs,
                    interner: &interner,
                });
            }
        })
        .unwrap();
}
