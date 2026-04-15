use std::path::Path;

use crate::ResolveCtx;

#[test]
fn test1() {
    // assets/tests/test1
    // 以下にbiwaのパッケージのディレクトリがあることを前提とする

    let mut srcs = biwac_base::SourceHolder::default();
    let mut metadata = biwac_base::MetadataHolder::default();
    let pkg_root_path = Path::new("../../assets/tests/test1");

    biwac_metadata_loader::try_load_package_metadata(&mut metadata, pkg_root_path.to_path_buf())
        .unwrap();

    let build_dir_path = pkg_root_path.join(Path::new(biwac_base::BIWA_BUILD_DIRECTORY_NAME));

    let pkg =
        biwac_package_loader::Pkg::try_load(&metadata, &mut srcs, pkg_root_path.to_path_buf())
            .unwrap();

    let deps =
        biwac_dependency_loader::try_load_dependencies(build_dir_path.to_path_buf()).unwrap();

    let ctx = ResolveCtx::new(&metadata, &deps).unwrap();
    let _hir = ctx.try_resolve(pkg).unwrap();
}
