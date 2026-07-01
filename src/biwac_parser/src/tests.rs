use std::cell::OnceCell;

use biwac_base::{IdentInterner, ModId, ModPath};
use biwac_span::Span;

use biwac_ast::{
    Exprs, Globals, Ident, IntegerLiteral, Literal, Primary, Stmt, StringLiteral, TypDecl, VarDecl,
};

#[test]
fn test1() {
    let modpath = ModPath::Main;
    let mod_id = ModId::new_in_self(0);
    let mut interner = IdentInterner::new();

    // NOTE: Rustの生文字列の扱いでは以下の場合
    // 空文字列の0行目が含まれ、fnは1行目となるため注意
    let src = r#"
fn foo() {
    let x = 0;
    // comment "
    let str = "string";
}
"#;

    let tokens = biwac_lexer::lex(&mut interner, mod_id, src).unwrap();

    let module = crate::Parser::new(mod_id, modpath, tokens, &mut interner)
        .try_parse()
        .unwrap();

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
                id: interner.get_or_insert("x"),
                span: Span::new(mod_id, 20, 21)
            },
            init: Exprs::Primary(Primary::Literal(Literal::Integer(IntegerLiteral {
                val: 0,
                span: Span::new(mod_id, 24, 25)
            }))),
            span: Span::new(mod_id, 16, 26),
            var_id: OnceCell::new()
        }),
        fn_foo.stmts.first().unwrap()
    );
    assert_eq!(
        &Stmt::VarDecl(VarDecl {
            typ: TypDecl::Any,
            id: Ident {
                id: interner.get_or_insert("str"),
                span: Span::new(mod_id, 52, 55)
            },
            init: Exprs::Primary(Primary::Literal(Literal::String(StringLiteral {
                val: "string".to_string(),
                span: Span::new(mod_id, 58, 66)
            }))),
            span: Span::new(mod_id, 48, 67),
            var_id: OnceCell::new()
        }),
        fn_foo.stmts.get(1).unwrap()
    );
    assert_eq!(None, fn_foo.expr);
}
