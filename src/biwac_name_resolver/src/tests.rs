use std::path::Path;

use crate::ResolveCtx;

#[test]
fn test1() {
    // assets/tests/test1
    // 以下にbiwaのパッケージのディレクトリがあることを前提とする

    let pkg =
        biwac_package_loader::Pkg::try_load(Path::new("../../assets/tests/test1").to_path_buf())
            .unwrap();

    let ctx = ResolveCtx::new();
    let _hir = ctx.try_resolve(pkg).unwrap();
}
