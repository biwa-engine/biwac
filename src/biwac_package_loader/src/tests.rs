use biwac_base::ModPath;

use crate::Pkg;

#[test]
fn test1() {
    // assets/tests/test1
    // 以下にbiwaのパッケージのディレクトリがあることを前提とする

    let modu = Pkg::try_load("../../assets/tests/test1").unwrap();

    println!("modu.modules.keys: {:#?}", modu.modules.keys());

    assert!(modu.modules.contains_key(&ModPath::Main));
    assert!(
        modu.modules
            .contains_key(&ModPath::Mod(vec!["math".to_string()]))
    );
    assert!(
        modu.modules
            .contains_key(&ModPath::Mod(vec!["math".to_string(), "pos".to_string()]))
    );
    assert!(
        modu.modules
            .contains_key(&ModPath::Mod(vec!["math".to_string(), "line".to_string()]))
    );
}
