use biwac_name_resolver::AbsId;

use crate::{Sym, Ty};

#[test]
fn test1() {
    // assets/tests/test1
    // 以下にbiwaのパッケージのディレクトリがあることを前提とする

    let pkg = biwac_package_loader::Pkg::try_load("../../assets/tests/test1").unwrap();

    let pkg = biwac_name_resolver::PkgSymMap::try_resolve_symbol(pkg).unwrap();

    let pkg = crate::infer(pkg).unwrap();

    let fn_main = if let Sym::FnDef(f) = pkg
        .syms
        .get(&AbsId::new(vec![], "main".to_string()))
        .unwrap()
    {
        f
    } else {
        panic!("not a function");
    };

    let fn_add = if let Sym::FnDef(f) = pkg
        .syms
        .get(&AbsId::new(vec![], "add".to_string()))
        .unwrap()
    {
        f
    } else {
        panic!("not a function");
    };

    let fn_math_fact = if let Sym::FnDef(f) = pkg
        .syms
        .get(&AbsId::new(vec!["math".to_string()], "fact".to_string()))
        .unwrap()
    {
        f
    } else {
        panic!("not a function");
    };

    let fn_math_pos_pos_new = if let Sym::FnDef(f) = pkg
        .syms
        .get(&AbsId::new(
            vec!["math".to_string(), "pos".to_string()],
            "pos_new".to_string(),
        ))
        .unwrap()
    {
        f
    } else {
        panic!("not a function");
    };

    let mut fn_main_exprs = fn_main.ty_info.exprs.iter().collect::<Vec<_>>();
    fn_main_exprs.sort_by(|(id1, _), (id2, _)| id1.cmp(id2));

    assert_eq!(&Ty::Int, fn_main_exprs[0].1); // 3
    assert_eq!(&Ty::Int, fn_main_exprs[1].1); // 2
    assert_eq!(&Ty::Int, fn_main_exprs[2].1); // add(3, 2)
    assert_eq!(&Ty::Int, fn_main_exprs[3].1); // x
    assert_eq!(&Ty::Int, fn_main_exprs[4].1); // math::fact(x)

    assert_eq!(&Ty::Int, fn_main_exprs[5].1); // 0
    assert_eq!(&Ty::Int, fn_main_exprs[6].1); // x
    assert_eq!(
        &Ty::Struct(AbsId {
            quals: vec!["math".to_string(), "pos".to_string()],
            id: "Pos".to_string()
        }),
        fn_main_exprs[7].1
    ); // pos_new(0, x)

    assert_eq!(&Ty::Int, fn_main_exprs[8].1); // 11
    assert_eq!(&Ty::Int, fn_main_exprs[9].1); // y
    assert_eq!(
        &Ty::Struct(AbsId {
            quals: vec!["math".to_string(), "pos".to_string()],
            id: "Pos".to_string()
        }),
        fn_main_exprs[10].1
    ); // pos_new(11, y)

    assert_eq!(
        &Ty::Struct(AbsId {
            quals: vec!["math".to_string(), "line".to_string()],
            id: "Line".to_string()
        }),
        fn_main_exprs[11].1
    ); // math::line::Line{ ... }

    let mut fn_add_exprs = fn_add.ty_info.exprs.iter().collect::<Vec<_>>();
    fn_add_exprs.sort_by(|(id1, _), (id2, _)| id1.cmp(id2));

    assert_eq!(&Ty::Int, fn_add_exprs[0].1); // x
    assert_eq!(&Ty::Int, fn_add_exprs[1].1); // y
    assert_eq!(&Ty::Int, fn_add_exprs[2].1); // x + y

    let mut fn_math_fact_exprs = fn_math_fact.ty_info.exprs.iter().collect::<Vec<_>>();
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

    let mut fn_math_pos_pos_new_exprs =
        fn_math_pos_pos_new.ty_info.exprs.iter().collect::<Vec<_>>();
    fn_math_pos_pos_new_exprs.sort_by(|(id1, _), (id2, _)| id1.cmp(id2));

    assert_eq!(&Ty::Int, fn_math_pos_pos_new_exprs[0].1); // x
    assert_eq!(&Ty::Int, fn_math_pos_pos_new_exprs[1].1); // y
    assert_eq!(
        &Ty::Struct(AbsId {
            quals: vec!["math".to_string(), "pos".to_string()],
            id: "Pos".to_string()
        }),
        fn_math_pos_pos_new_exprs[2].1
    ); // Pos { x = x, y = y}
}
