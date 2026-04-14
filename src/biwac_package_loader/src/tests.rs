use std::path::Path;

use biwac_base::ModPath;

use crate::Pkg;

#[test]
fn test1() {
    // assets/tests/test1
    // 以下にbiwaのパッケージのディレクトリがあることを前提とする

    let pkg = Pkg::try_load(Path::new("../../assets/tests/test1").to_path_buf()).unwrap();

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
