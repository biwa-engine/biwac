use biwac_hir::{DefinedTy, ImplValDefContentKind, Ty, TyId, ValDefContentKind, ValId};

#[test]
fn test1() {
    // assets/tests/test1
    // 以下にbiwaのパッケージのディレクトリがあることを前提とする

    let pkg = biwac_package_loader::Pkg::try_load("../../assets/tests/test1").unwrap();

    let hir = biwac_name_resolver::ResolveCtx::new()
        .try_resolve(pkg)
        .unwrap();

    let hir = crate::TyCtx::new(hir).infer().unwrap();

    let fn_main = if let ValDefContentKind::Fn(f) = hir
        .vals
        .get(&ValId::new(vec![], "main".to_string()))
        .unwrap()
    {
        f
    } else {
        panic!("not a function");
    };

    let fn_add = if let ValDefContentKind::Fn(f) = hir
        .vals
        .get(&ValId::new(vec![], "add".to_string()))
        .unwrap()
    {
        f
    } else {
        panic!("not a function");
    };

    let fn_math_fact = if let ValDefContentKind::Fn(f) = hir
        .vals
        .get(&ValId::new(vec!["math".to_string()], "fact".to_string()))
        .unwrap()
    {
        f
    } else {
        panic!("not a function");
    };

    let struct_math_pos_pos_tid = TyId::new(
        vec!["math".to_string(), "pos".to_string()],
        "Pos".to_string(),
    );
    let struct_math_pos_pos = Ty::Defined(DefinedTy {
        tid: struct_math_pos_pos_tid.clone(),
        genargs: vec![],
    });
    let fn_math_pos_pos_new_impl_valid = hir
        .get_impl_value_id_of_type(&struct_math_pos_pos, &"new".to_string())
        .unwrap()
        .unwrap();

    let fn_math_pos_pos_new = if let ImplValDefContentKind::Fn(f) = &hir
        .tys
        .get(&struct_math_pos_pos_tid)
        .unwrap()
        .vals
        .get("new")
        .unwrap()
        .vals
        .get(&fn_math_pos_pos_new_impl_valid)
        .unwrap()
        .val_content
    {
        f
    } else {
        panic!("not a function");
    };

    let mut fn_main_exprs = fn_main.expr_tys.iter().collect::<Vec<_>>();
    fn_main_exprs.sort_by(|(id1, _), (id2, _)| id1.cmp(id2));

    assert_eq!(&Ty::Int, fn_main_exprs[0].1); // 3
    assert_eq!(&Ty::Int, fn_main_exprs[1].1); // 2
    assert_eq!(&Ty::Int, fn_main_exprs[2].1); // add(3, 2)
    assert_eq!(&Ty::Int, fn_main_exprs[3].1); // x
    assert_eq!(&Ty::Int, fn_main_exprs[4].1); // math::fact(x)

    assert_eq!(&Ty::Int, fn_main_exprs[5].1); // 0
    assert_eq!(&Ty::Int, fn_main_exprs[6].1); // x
    assert_eq!(
        &Ty::Defined(DefinedTy {
            tid: TyId::new(
                vec!["math".to_string(), "pos".to_string()],
                "Pos".to_string()
            ),
            genargs: vec![]
        }),
        fn_main_exprs[7].1
    ); // pos_new(0, x)

    assert_eq!(&Ty::Int, fn_main_exprs[8].1); // 11
    assert_eq!(&Ty::Int, fn_main_exprs[9].1); // y
    assert_eq!(
        &Ty::Defined(DefinedTy {
            tid: TyId::new(
                vec!["math".to_string(), "pos".to_string()],
                "Pos".to_string()
            ),
            genargs: vec![]
        }),
        fn_main_exprs[10].1
    ); // pos_new(11, y)

    assert_eq!(
        &Ty::Defined(DefinedTy {
            tid: TyId::new(
                vec!["math".to_string(), "line".to_string()],
                "Line".to_string()
            ),
            genargs: vec![]
        }),
        fn_main_exprs[11].1
    ); // math::line::Line{ ... }

    let mut fn_add_exprs = fn_add.expr_tys.iter().collect::<Vec<_>>();
    fn_add_exprs.sort_by(|(id1, _), (id2, _)| id1.cmp(id2));

    assert_eq!(&Ty::Int, fn_add_exprs[0].1); // x
    assert_eq!(&Ty::Int, fn_add_exprs[1].1); // y
    assert_eq!(&Ty::Int, fn_add_exprs[2].1); // x + y

    let mut fn_math_fact_exprs = fn_math_fact.expr_tys.iter().collect::<Vec<_>>();
    fn_math_fact_exprs.sort_by(|(id1, _), (id2, _)| id1.cmp(id2));

    assert_eq!(&Ty::Int, fn_math_fact_exprs[0].1); // n
    assert_eq!(&Ty::Int, fn_math_fact_exprs[1].1); // 0
    assert_eq!(&Ty::Bool, fn_math_fact_exprs[2].1); // n == 0
    assert_eq!(&Ty::Int, fn_math_fact_exprs[3].1); // 1
    assert_eq!(&Ty::Int, fn_math_fact_exprs[4].1); // n
    assert_eq!(&Ty::Int, fn_math_fact_exprs[5].1); // n
    assert_eq!(&Ty::Int, fn_math_fact_exprs[6].1); // 1
    assert_eq!(&Ty::Int, fn_math_fact_exprs[7].1); // fact(n - 1)
    assert_eq!(&Ty::Int, fn_math_fact_exprs[8].1); // n * fact(n - 1)
    assert_eq!(&Ty::Int, fn_math_fact_exprs[9].1); // if n == 0 { 1 } else { n * fact(n - 1) }

    let mut fn_math_pos_pos_new_exprs = fn_math_pos_pos_new.expr_tys.iter().collect::<Vec<_>>();
    fn_math_pos_pos_new_exprs.sort_by(|(id1, _), (id2, _)| id1.cmp(id2));

    assert_eq!(&Ty::Int, fn_math_pos_pos_new_exprs[0].1); // x
    assert_eq!(&Ty::Int, fn_math_pos_pos_new_exprs[1].1); // y
    assert_eq!(
        &Ty::Defined(DefinedTy {
            tid: TyId::new(
                vec!["math".to_string(), "pos".to_string()],
                "Pos".to_string()
            ),
            genargs: vec![]
        }),
        fn_math_pos_pos_new_exprs[2].1
    ); // Pos { x = x, y = y}
}
