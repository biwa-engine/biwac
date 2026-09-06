use std::{path::Path, sync::Arc};

use biwac_base::{BiwacError, ErrorContext};
use biwac_dependency_metadata::ExternalPackage;

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

    // let build_dir_path = pkg_root_path.join(Path::new(biwac_base::BIWA_BUILD_DIRECTORY_NAME));

    // 依存パッケージは <root>/.biwa_build/deps/<name>/ に取得済みである前提。
    // std は依存を持たないので、実際にはここは使われない。
    let packages_dir = biwac_base::dependencies_dir(pkg_root_path);

    let root_dep_names: Vec<String> = metadata
        .metadata
        .dependencies
        .iter()
        .map(|d| d.name.value().to_string())
        .collect();

    // std は依存を持たないので、ここは常に空になる。
    // driver と違って推移閉包は辿らず、直接依存だけを見る簡易版である。
    let external_packages: Vec<ExternalPackage> = root_dep_names
        .iter()
        .map(|dep_name| {
            let dep_root = packages_dir.join(dep_name);
            let dep_metadata =
                biwac_metadata_loader::try_load_package_metadata(dep_root.clone()).unwrap();
            let dep_meta = load_dep_metadata(&dep_root, dep_name);
            ExternalPackage {
                ident: interner.get_or_insert(dep_name),
                // PackageId は (name, version) から導出される。driver と同じ規則。
                pkg_id: biwac_span::PackageHashId::new(
                    &dep_metadata.metadata.name,
                    &dep_metadata.metadata.version,
                )
                .as_package_id(),
                meta: Arc::new(dep_meta),
                direct: true,
            }
        })
        .collect();

    let mut pkg = biwac_package_loader::Pkg::try_load(
        &metadata,
        &mut interner,
        &mut srcs,
        pkg_root_path.to_path_buf(),
    )
    .unwrap();

    // driver と同じく、選択されていない arch の native を落としてから名前解決する。
    // std は同じ名前で arch 違いの native を並べているので、
    // 落とさずに渡すとシンボルが衝突する。
    pkg.walk_modules_mut(|module| {
        biwac_attribute::retain_for_target(
            &mut module.ast,
            biwac_base::Target::TypeScript,
            &interner,
        );
    });

    let _hir = NameResolver::new(&metadata, external_packages, pkg_name, pkg)
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
