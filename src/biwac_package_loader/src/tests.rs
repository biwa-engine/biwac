use biwac_base::ModPath;

use crate::Pkg;

#[test]
fn test1() {
    // assets/tests/test1
    // 以下にbiwaのパッケージのディレクトリがあることを前提とする

    let pkg = Pkg::try_load("../../assets/tests/test1").unwrap();

    assert!(pkg.modules.contains_key(&ModPath::Main));
    assert!(
        pkg.modules
            .contains_key(&ModPath::Mod(vec!["math".to_string()]))
    );
    assert!(
        pkg.modules
            .contains_key(&ModPath::Mod(vec!["math".to_string(), "pos".to_string()]))
    );
    assert!(
        pkg.modules
            .contains_key(&ModPath::Mod(vec!["math".to_string(), "line".to_string()]))
    );
}
