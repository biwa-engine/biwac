use biwac_base::{ModPath, Pos, Span};

use crate::{
    Exprs, Globals, Ident, IntegerLiteral, Literal, ModAst, Primary, Stmt, StringLiteral, VarDecl,
    types::TypDecl,
};

#[test]
fn test1() {
    let modu = ModPath::Main;

    // NOTE: Rustの生文字列の扱いでは以下の場合
    // 空文字列の0行目が含まれ、fnは1行目となるため注意
    let src = r#"
fn foo() {
    let x = 0;
    // comment "
    let str = "string";
}
"#;

    let tokens = biwac_lexer::lex(modu.clone(), src).unwrap();

    let module = ModAst::try_parse(tokens).unwrap();

    let g0 = module.globals.first().unwrap();

    let fn_foo = if let Globals::FnDef(f) = g0 {
        f
    } else {
        panic!("not a function: {g0:#?}");
    };

    assert_eq!(2, fn_foo.stmts.len());
    assert_eq!(
        &Stmt::VarDecl(VarDecl {
            typ: TypDecl::Any,
            id: Ident {
                id: "x".to_string(),
                span: Span::new(modu.clone(), Pos::new(2, 8), Pos::new(2, 9))
            },
            init: Exprs::Primary(Primary::Literal(Literal::Integer(IntegerLiteral {
                val: 0,
                span: Span::new(modu.clone(), Pos::new(2, 12), Pos::new(2, 13))
            }))),
            span: Span::new(modu.clone(), Pos::new(2, 4), Pos::new(2, 14))
        }),
        fn_foo.stmts.first().unwrap()
    );
    assert_eq!(
        &Stmt::VarDecl(VarDecl {
            typ: TypDecl::Any,
            id: Ident {
                id: "str".to_string(),
                span: Span::new(modu.clone(), Pos::new(4, 8), Pos::new(4, 11))
            },
            init: Exprs::Primary(Primary::Literal(Literal::String(StringLiteral {
                val: "string".to_string(),
                span: Span::new(modu.clone(), Pos::new(4, 14), Pos::new(4, 22))
            }))),
            span: Span::new(modu.clone(), Pos::new(4, 4), Pos::new(4, 23))
        }),
        fn_foo.stmts.get(1).unwrap()
    );
    assert_eq!(None, fn_foo.expr);
}
