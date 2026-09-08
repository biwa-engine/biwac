use biwac_base::{IdentInterner, ModId};
use biwac_span::Span;

use crate::TkKind;

#[test]
fn test1() {
    let modu = ModId::new_in_self(0);
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

    let tokens = crate::lex(&mut interner, modu, src).unwrap();

    assert_eq!(TkKind::KwFn, tokens[0].kind);

    assert_eq!(TkKind::Ident(interner.get_or_insert("foo")), tokens[1].kind);

    assert_eq!(Span::new(modu, 4, 7,), tokens[1].span);

    assert_eq!(TkKind::MarkLPare, tokens[2].kind);

    assert_eq!(TkKind::MarkRPare, tokens[3].kind);

    assert_eq!(TkKind::MarkLBrace, tokens[4].kind);

    assert_eq!(TkKind::KwLet, tokens[5].kind);

    assert_eq!(TkKind::Ident(interner.get_or_insert("x")), tokens[6].kind);

    assert_eq!(Span::new(modu, 20, 21,), tokens[6].span);

    assert_eq!(TkKind::MarkAssign, tokens[7].kind);

    assert_eq!(TkKind::LiteralInteger(0), tokens[8].kind);
    assert_eq!(Span::new(modu, 24, 25,), tokens[8].span);

    assert_eq!(TkKind::MarkSemiColon, tokens[9].kind);

    assert_eq!(TkKind::KwLet, tokens[10].kind);

    assert_eq!(
        TkKind::Ident(interner.get_or_insert("str")),
        tokens[11].kind
    );
    assert_eq!(Span::new(modu, 52, 55), tokens[11].span);

    assert_eq!(TkKind::MarkAssign, tokens[12].kind);

    assert_eq!(TkKind::LiteralString("string"), tokens[13].kind);
    assert_eq!(Span::new(modu, 58, 66), tokens[13].span);

    assert_eq!(TkKind::MarkSemiColon, tokens[14].kind);

    assert_eq!(TkKind::MarkRBrace, tokens[15].kind);
}

/// 数値リテラルは pre_lex が 1 トークンとして読み切る。
///
/// `1.5` の `.` を記号として切ってしまうと、後の段では復元できない。
#[test]
fn number_literals() {
    let modu = ModId::new_in_self(0);
    let mut interner = IdentInterner::new();

    let src = "let a = 1.5; let b = 12; let c = 0xFF; let d = x.foo(1.25)";

    let tokens = crate::lex(&mut interner, modu, src).unwrap();
    let kinds: Vec<&TkKind> = tokens.iter().map(|t| &t.kind).collect();

    assert_eq!(TkKind::LiteralFloat(1.5), *kinds[3]);
    // `1.5` は 3 文字ぶんの span を持つ
    assert_eq!(Span::new(modu, 8, 11), tokens[3].span);
    assert_eq!(TkKind::MarkSemiColon, *kinds[4]);

    assert_eq!(TkKind::LiteralInteger(12), *kinds[8]);
    assert_eq!(TkKind::LiteralInteger(0xFF), *kinds[13]);

    // 引数の中でも同じ
    assert_eq!(TkKind::LiteralFloat(1.25), *kinds[22]);
    assert_eq!(TkKind::MarkRPare, *kinds[23]);
}

/// 整数のあとのドットは、後ろに数字が続かなければ区切りである。
///
/// `1.foo()` を小数として読んでしまうとメソッド呼び出しが書けなくなる。
#[test]
fn dot_after_integer_is_a_mark() {
    let modu = ModId::new_in_self(0);
    let mut interner = IdentInterner::new();

    let tokens = crate::lex(&mut interner, modu, "1.foo()").unwrap();

    assert_eq!(TkKind::LiteralInteger(1), tokens[0].kind);
    assert_eq!(TkKind::MarkDot, tokens[1].kind);
    assert_eq!(TkKind::Ident(interner.get_or_insert("foo")), tokens[2].kind);
}

/// 数字の連続のあとに識別子の文字が続く並びは、どう解釈しても意味を持たない。
#[test]
fn broken_number_literals_are_errors() {
    let modu = ModId::new_in_self(0);

    for src in ["let x = 012abc;", "let x = 012.345abc;", "let x = 1e5;"] {
        let mut interner = IdentInterner::new();
        assert!(
            matches!(
                crate::lex(&mut interner, modu, src),
                Err(crate::TokenizeError::InvalidNumberLiteral { .. })
            ),
            "`{src}` must be rejected"
        );
    }
}

/// 区切りに出会わないまま領域が終わる場合。
#[test]
fn token_at_the_end_of_a_region() {
    let modu = ModId::new_in_self(0);
    let mut interner = IdentInterner::new();

    // 末尾に改行が無く、1 文字のトークンで終わる
    let tokens = crate::lex(&mut interner, modu, "let x = y").unwrap();

    assert_eq!(4, tokens.len());
    assert_eq!(TkKind::Ident(interner.get_or_insert("y")), tokens[3].kind);
}
