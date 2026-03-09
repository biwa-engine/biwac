use crate::ResolveCtx;

#[test]
fn test1() {
    // assets/tests/test1
    // 以下にbiwaのパッケージのディレクトリがあることを前提とする

    let pkg = biwac_package_loader::Pkg::try_load("../../assets/tests/test1").unwrap();

    let ctx = ResolveCtx::new();
    let _hir = ctx.try_resolve(pkg).unwrap();
}
