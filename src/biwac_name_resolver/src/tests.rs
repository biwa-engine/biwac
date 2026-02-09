use crate::PkgSymMap;

#[test]
fn test1() {
    // assets/tests/test1
    // 以下にbiwaのパッケージのディレクトリがあることを前提とする

    let pkg = biwac_package_loader::Pkg::try_load("../../assets/tests/test1").unwrap();

    let _pkg = PkgSymMap::try_resolve_symbol(pkg).unwrap();
}
