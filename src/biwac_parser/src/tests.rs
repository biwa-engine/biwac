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

/// `if`/`while` の条件式に構造体リテラルを置けないことの回帰テスト。
///
/// 条件式の直後にはブロックの `{` が来るので、
/// `if flag {` の `{` を構造体リテラルの開始と読むと必ず誤る。
mod no_struct_literal_in_condition {
    use biwac_ast::{Exprs, FnDef, Globals, Literal, Primary, Stmt};
    use biwac_base::{IdentInterner, ModId, ModPath};

    fn parse_fn(src: &str) -> FnDef {
        let mod_id = ModId::new_in_self(0);
        let mut interner = IdentInterner::new();

        let tokens = biwac_lexer::lex(&mut interner, mod_id, src).unwrap();
        let module = crate::Parser::new(mod_id, ModPath::Main, tokens, &mut interner)
            .try_parse()
            .unwrap_or_else(|e| panic!("parse failed: {e:#?}"));

        match module.globals.into_iter().next().unwrap() {
            Globals::FnDef(f) => f,
            g => panic!("not a function: {g:#?}"),
        }
    }

    fn is_struct_literal(expr: &Exprs) -> bool {
        matches!(expr, Exprs::Primary(Primary::Literal(Literal::Struct(_))))
    }

    /// 裸の識別子を条件に書ける。`{` はブロックの開始として読まれる。
    #[test]
    fn bare_identifier_is_a_condition_not_a_struct_literal() {
        let f = parse_fn(
            r#"
fn foo(flag: Bool) {
    if flag {
        let x = 0;
    }
}
"#,
        );

        let Stmt::If(if_stmt) = f.stmts.first().unwrap() else {
            panic!("not an if: {:#?}", f.stmts);
        };

        assert!(
            matches!(if_stmt.cond, Exprs::Primary(Primary::Variable(_))),
            "condition should be a variable, got {:#?}",
            if_stmt.cond
        );
        assert_eq!(1, if_stmt.then.stmts.len());
    }

    /// `while` の条件式も同じ扱いになる。
    #[test]
    fn bare_identifier_is_a_while_condition() {
        let f = parse_fn(
            r#"
fn foo(flag: Bool) {
    while flag {
        let x = 0;
    }
}
"#,
        );

        let Stmt::While(while_stmt) = f.stmts.first().unwrap() else {
            panic!("not a while: {:#?}", f.stmts);
        };

        assert!(
            matches!(while_stmt.cond, Exprs::Primary(Primary::Variable(_))),
            "condition should be a variable, got {:#?}",
            while_stmt.cond
        );
        assert_eq!(1, while_stmt.stmts.stmts.len());
    }

    /// 括弧の内側では意味が閉じるので、制限は解ける。
    #[test]
    fn parentheses_lift_the_restriction() {
        let f = parse_fn(
            r#"
fn foo() {
    if (Flagged { on = TRUE }).on {
        let x = 0;
    }
}
"#,
        );

        let Stmt::If(if_stmt) = f.stmts.first().unwrap() else {
            panic!("not an if: {:#?}", f.stmts);
        };

        let Exprs::Primary(Primary::MemberAccess(access)) = &if_stmt.cond else {
            panic!("not a member access: {:#?}", if_stmt.cond);
        };
        assert!(
            is_struct_literal(&access.left),
            "the parenthesized expression should be a struct literal, got {:#?}",
            access.left
        );
    }

    /// 引数リストの内側でも制限は解ける。
    #[test]
    fn arguments_lift_the_restriction() {
        let f = parse_fn(
            r#"
fn foo() {
    if takes(Flagged { on = TRUE }) {
        let x = 0;
    }
}
"#,
        );

        let Stmt::If(if_stmt) = f.stmts.first().unwrap() else {
            panic!("not an if: {:#?}", f.stmts);
        };

        let Exprs::Primary(Primary::FnCall(call)) = &if_stmt.cond else {
            panic!("not a call: {:#?}", if_stmt.cond);
        };
        assert!(
            is_struct_literal(call.args.first().unwrap()),
            "the argument should be a struct literal, got {:#?}",
            call.args
        );
    }

    /// 条件式を抜ければ元に戻る。ブロックの中では構造体リテラルを書ける。
    #[test]
    fn restriction_ends_with_the_condition() {
        let f = parse_fn(
            r#"
fn foo(flag: Bool) {
    if flag {
        let f = Flagged { on = flag };
    }
}
"#,
        );

        let Stmt::If(if_stmt) = f.stmts.first().unwrap() else {
            panic!("not an if: {:#?}", f.stmts);
        };

        let Stmt::VarDecl(decl) = if_stmt.then.stmts.first().unwrap() else {
            panic!("not a let: {:#?}", if_stmt.then.stmts);
        };
        assert!(
            is_struct_literal(&decl.init),
            "the initializer should be a struct literal, got {:#?}",
            decl.init
        );
    }
}

/// enum 宣言と match のパース。
mod enum_and_match {
    use biwac_ast::{
        EnumDef, Globals, MatchExprArm, Pattern, PatternFields, Primary, Stmt, TypeDef,
        VariantFieldsDecl,
    };
    use biwac_base::{IdentInterner, ModId, ModPath};

    fn parse(src: &str) -> (Vec<Globals>, IdentInterner) {
        let mod_id = ModId::new_in_self(0);
        let mut interner = IdentInterner::new();

        let tokens = biwac_lexer::lex(&mut interner, mod_id, src).unwrap();
        let module = crate::Parser::new(mod_id, ModPath::Main, tokens, &mut interner)
            .try_parse()
            .unwrap_or_else(|e| panic!("parse failed: {e:#?}"));

        (module.globals, interner)
    }

    fn enum_def(globals: Vec<Globals>) -> EnumDef {
        match globals.into_iter().next().unwrap() {
            Globals::TypeDef(TypeDef::Enum(e)) => e,
            g => panic!("not an enum: {g:#?}"),
        }
    }

    #[test]
    fn three_shapes_of_variant() {
        let (globals, interner) = parse(
            r#"
enum Color {
    Red,
    Rgb(Int, Int, Int),
    Named { name: Int, alpha: Int },
}
"#,
        );

        let e = enum_def(globals);
        assert_eq!("Color", interner.get_str(&e.id.id).unwrap());
        assert_eq!(3, e.variants.len());

        assert!(matches!(e.variants[0].fields, VariantFieldsDecl::Unit));
        match &e.variants[1].fields {
            VariantFieldsDecl::Tuple(typs) => assert_eq!(3, typs.len()),
            other => panic!("not a tuple variant: {other:#?}"),
        }
        match &e.variants[2].fields {
            VariantFieldsDecl::Struct(members) => assert_eq!(2, members.len()),
            other => panic!("not a struct variant: {other:#?}"),
        }
    }

    #[test]
    fn generic_enum() {
        let (globals, _) = parse("enum Option[T] { None, Some(T), }");

        let e = enum_def(globals);
        assert_eq!(1, e.genargs.as_ref().unwrap().genargs.len());
        assert_eq!(2, e.variants.len());
    }

    /// アームが値を返すなら match は式になる。
    #[test]
    fn match_as_expression() {
        let (globals, _) = parse(
            r#"
fn f(o: Int) -> Int {
    let n = match o {
        Option::None => 0,
        Option::Some(x) => x,
    };
    n
}
"#,
        );

        let Globals::FnDef(f) = globals.into_iter().next().unwrap() else {
            panic!("not a function");
        };
        let Stmt::VarDecl(decl) = f.stmts.first().unwrap() else {
            panic!("not a let: {:#?}", f.stmts);
        };
        let biwac_ast::Exprs::Primary(Primary::Match(m)) = &decl.init else {
            panic!("not a match expression: {:#?}", decl.init);
        };

        assert_eq!(2, m.arms.len());
        assert_variant_pattern(&m.arms[0], PatternShape::Unit);
        assert_variant_pattern(&m.arms[1], PatternShape::Tuple(1));
    }

    /// アームがブロック文なら match は文になる。
    #[test]
    fn match_as_statement() {
        let (globals, _) = parse(
            r#"
fn f(c: Int) {
    match c {
        Color::Red => { let x = 0; }
        Color::Named { name = n, alpha } => { let y = 0; }
        _ => { let z = 0; }
    }
}
"#,
        );

        let Globals::FnDef(f) = globals.into_iter().next().unwrap() else {
            panic!("not a function");
        };
        let Stmt::Match(m) = f.stmts.first().unwrap() else {
            panic!("not a match statement: {:#?}", f.stmts);
        };

        assert_eq!(3, m.arms.len());

        match &m.arms[1].pattern {
            Pattern::Variant(v) => match &v.fields {
                // 省略形の `alpha` も同名への束縛として入っている。
                PatternFields::Struct(fields) => {
                    assert_eq!(2, fields.len());
                    assert!(matches!(fields[1].1, Pattern::Ident(_)));
                }
                other => panic!("not a struct pattern: {other:#?}"),
            },
            other => panic!("not a variant pattern: {other:#?}"),
        }

        assert!(matches!(m.arms[2].pattern, Pattern::Wildcard(_)));
    }

    /// 単独の識別子は束縛としてパースされる。
    /// バリアントかどうかは名前解決が決める。
    #[test]
    fn bare_identifier_is_a_binding() {
        let (globals, _) = parse("fn f(c: Int) { match c { x => { let y = 0; } } }");

        let Globals::FnDef(f) = globals.into_iter().next().unwrap() else {
            panic!("not a function");
        };
        let Stmt::Match(m) = f.stmts.first().unwrap() else {
            panic!("not a match statement");
        };
        assert!(matches!(m.arms[0].pattern, Pattern::Ident(_)));
    }

    enum PatternShape {
        Unit,
        Tuple(usize),
    }

    fn assert_variant_pattern(arm: &MatchExprArm, shape: PatternShape) {
        let Pattern::Variant(v) = &arm.pattern else {
            panic!("not a variant pattern: {:#?}", arm.pattern);
        };
        match (&v.fields, shape) {
            (PatternFields::Unit, PatternShape::Unit) => {}
            (PatternFields::Tuple(pats), PatternShape::Tuple(n)) => assert_eq!(n, pats.len()),
            (other, _) => panic!("unexpected pattern fields: {other:#?}"),
        }
    }
}
