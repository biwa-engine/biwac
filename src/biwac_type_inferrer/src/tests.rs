#[test]
fn test1() {
    // assets/tests/test1
    // 以下にbiwaのパッケージのディレクトリがあることを前提とする

    let pkg = biwac_package_loader::Pkg::try_load("../../assets/tests/test1").unwrap();

    let pkg = biwac_name_resolver::PkgSymMap::try_resolve_symbol(pkg).unwrap();

    let pkg = crate::infer(pkg).unwrap();
}
