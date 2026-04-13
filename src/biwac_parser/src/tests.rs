use biwac_base::{FileId, Span};

use biwac_ast::{
    Exprs, Globals, Ident, IntegerLiteral, Literal, Primary, Stmt, StringLiteral, TypDecl, VarDecl,
};

#[test]
fn test1() {
    let file_id = FileId::new(0);

    // NOTE: Rustの生文字列の扱いでは以下の場合
    // 空文字列の0行目が含まれ、fnは1行目となるため注意
    let src = r#"
fn foo() {
    let x = 0;
    // comment "
    let str = "string";
}
"#;

    let tokens = biwac_lexer::lex(file_id, src).unwrap();

    let module = crate::Parser::new(tokens).try_parse().unwrap();

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
                span: Span::new(file_id, 20, 21)
            },
            init: Exprs::Primary(Primary::Literal(Literal::Integer(IntegerLiteral {
                val: 0,
                span: Span::new(file_id, 25, 26)
            }))),
            span: Span::new(file_id, 16, 28)
        }),
        fn_foo.stmts.first().unwrap()
    );
    assert_eq!(
        &Stmt::VarDecl(VarDecl {
            typ: TypDecl::Any,
            id: Ident {
                id: "str".to_string(),
                span: Span::new(file_id, 54, 57)
            },
            init: Exprs::Primary(Primary::Literal(Literal::String(StringLiteral {
                val: "string".to_string(),
                span: Span::new(file_id, 60, 68)
            }))),
            span: Span::new(file_id, 50, 69)
        }),
        fn_foo.stmts.get(1).unwrap()
    );
    assert_eq!(None, fn_foo.expr);
}
