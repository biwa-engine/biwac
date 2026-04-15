use std::path::Path;

use biwac_base::{MetadataHolder, ModPath, SourceHolder};

use crate::Pkg;

#[test]
fn test1() {
    // assets/tests/test1
    // 以下にbiwaのパッケージのディレクトリがあることを前提とする

    let mut srcs = SourceHolder::default();
    let mut metadata = MetadataHolder::default();
    let pkg_root_path = Path::new("../../assets/tests/test1");

    biwac_metadata_loader::try_load_package_metadata(&mut metadata, pkg_root_path.to_path_buf())
        .unwrap();

    let pkg = Pkg::try_load(&metadata, &mut srcs, pkg_root_path.to_path_buf()).unwrap();

    assert!(pkg.modules.iter().any(|(_, m)| m.modpath == ModPath::Main));
    assert!(
        pkg.modules
            .iter()
            .any(|(_, m)| m.modpath == ModPath::Mod(vec!["math".to_string()]))
    );
    assert!(
        pkg.modules
            .iter()
            .any(|(_, m)| m.modpath == ModPath::Mod(vec!["math".to_string(), "pos".to_string()]))
    );
    assert!(
        pkg.modules
            .iter()
            .any(|(_, m)| m.modpath == ModPath::Mod(vec!["math".to_string(), "line".to_string()]))
    );
}
