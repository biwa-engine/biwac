use biwac_base::{ModId, Span};

use crate::{TkKind, TkVal};

#[test]
fn test1() {
    let modu = ModId::new(0);

    // NOTE: Rustの生文字列の扱いでは以下の場合
    // 空文字列の0行目が含まれ、fnは1行目となるため注意
    let src = r#"
fn foo() {
    let x = 0;
    // comment "
    let str = "string";
}
"#;

    let tokens = crate::lex(modu, src).unwrap();

    assert_eq!(TkKind::KwFn, tokens[0].kind);

    assert_eq!(TkKind::Ident("foo"), tokens[1].kind);
    assert_eq!(Some(TkVal::String("foo".to_string())), tokens[1].val);
    assert_eq!(Span::new(modu, 4, 7,), tokens[1].span);

    assert_eq!(TkKind::MarkLPare, tokens[2].kind);

    assert_eq!(TkKind::MarkRPare, tokens[3].kind);

    assert_eq!(TkKind::MarkLBrace, tokens[4].kind);

    assert_eq!(TkKind::KwLet, tokens[5].kind);

    assert_eq!(TkKind::Ident("x"), tokens[6].kind);
    assert_eq!(Some(TkVal::String("x".to_string())), tokens[6].val);
    assert_eq!(Span::new(modu, 20, 21,), tokens[6].span);

    assert_eq!(TkKind::MarkAssign, tokens[7].kind);

    assert_eq!(TkKind::LiteralInteger(0), tokens[8].kind);
    assert_eq!(Some(TkVal::Integer(0)), tokens[8].val);
    assert_eq!(Span::new(modu, 24, 25,), tokens[8].span);

    assert_eq!(TkKind::MarkSemiColon, tokens[9].kind);

    assert_eq!(TkKind::KwLet, tokens[10].kind);

    assert_eq!(TkKind::Ident("str"), tokens[11].kind);
    assert_eq!(Some(TkVal::String("str".to_string())), tokens[11].val);
    assert_eq!(Span::new(modu, 52, 55), tokens[11].span);

    assert_eq!(TkKind::MarkAssign, tokens[12].kind);

    assert_eq!(TkKind::LiteralString("string"), tokens[13].kind);
    assert_eq!(Some(TkVal::String("string".to_string())), tokens[13].val);
    assert_eq!(Span::new(modu, 58, 66), tokens[13].span);

    assert_eq!(TkKind::MarkSemiColon, tokens[14].kind);

    assert_eq!(TkKind::MarkRBrace, tokens[15].kind);
}
