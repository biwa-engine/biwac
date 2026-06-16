use std::path::Path;

use biwac_base::{BiwacError, ErrorContext};

use crate::NameResolver;

#[test]
fn test1() {
    // assets/tests/test1
    // 以下にbiwaのパッケージのディレクトリがあることを前提とする

    let mut srcs = biwac_base::SourceHolder::default();
    let mut interner = biwac_base::IdentInterner::default();
    let pkg_root_path = Path::new("../../assets/tests/test1");
    let pkg_name = interner.get_or_insert("test1");

    let metadata =
        biwac_metadata_loader::try_load_package_metadata(pkg_root_path.to_path_buf()).unwrap();

    let build_dir_path = pkg_root_path.join(Path::new(biwac_base::BIWA_BUILD_DIRECTORY_NAME));

    let pkg = biwac_package_loader::Pkg::try_load(
        &metadata,
        &mut interner,
        &mut srcs,
        pkg_root_path.to_path_buf(),
    )
    .unwrap();

    let deps =
        biwac_dependency_loader::try_load_dependencies(build_dir_path.to_path_buf()).unwrap();

    let _hir = NameResolver::new(&metadata, &deps, pkg_name, pkg)
        .unwrap()
        .try_resolve()
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
