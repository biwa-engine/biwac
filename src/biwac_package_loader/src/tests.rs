use std::path::Path;

use biwac_base::{IdentInterner, SourceHolder};

use biwac_base::PackageKind;

use crate::Pkg;

#[test]
fn test1() {
    // assets/tests/test1
    // 以下にbiwaのパッケージのディレクトリがあることを前提とする

    let mut srcs = SourceHolder::default();
    let mut interner = IdentInterner::default();
    let pkg_root_path = Path::new("../../assets/tests/test1");

    let metadata =
        biwac_metadata_loader::try_load_package_metadata(pkg_root_path.to_path_buf()).unwrap();

    let pkg = Pkg::try_load(
        &metadata,
        &mut interner,
        &mut srcs,
        pkg_root_path.to_path_buf(),
    )
    .unwrap();

    assert!(pkg.pkg_kind == PackageKind::Bin);

    // existence check of `collections` module
    assert!(
        pkg.root_module
            .children
            .contains_key(&interner.get_or_insert("collections"))
    );

    // existence check of `math` module
    let mod_math = pkg
        .root_module
        .children
        .get(&interner.get_or_insert("math"))
        .unwrap();

    // existence check of `math::pos` module
    assert!(
        mod_math
            .children
            .contains_key(&interner.get_or_insert("pos"))
    );

    // existence check of `math::line` module
    assert!(
        mod_math
            .children
            .contains_key(&interner.get_or_insert("line"))
    );
}
