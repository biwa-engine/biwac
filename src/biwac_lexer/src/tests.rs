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

    assert_eq!(TkKind::Fn, tokens[0].kind);

    assert_eq!(TkKind::Ident, tokens[1].kind);
    assert_eq!(Some(TkVal::String("foo".to_string())), tokens[1].val);
    assert_eq!(Span::new(modu, 4, 7,), tokens[1].span);

    assert_eq!(TkKind::LPare, tokens[2].kind);

    assert_eq!(TkKind::RPare, tokens[3].kind);

    assert_eq!(TkKind::LBrace, tokens[4].kind);

    assert_eq!(TkKind::Let, tokens[5].kind);

    assert_eq!(TkKind::Ident, tokens[6].kind);
    assert_eq!(Some(TkVal::String("x".to_string())), tokens[6].val);
    assert_eq!(Span::new(modu, 20, 21,), tokens[6].span);

    assert_eq!(TkKind::Assign, tokens[7].kind);

    assert_eq!(TkKind::IntegerLiteral, tokens[8].kind);
    assert_eq!(Some(TkVal::Integer(0)), tokens[8].val);
    assert_eq!(Span::new(modu, 24, 25,), tokens[8].span);

    assert_eq!(TkKind::SemiColon, tokens[9].kind);

    assert_eq!(TkKind::Let, tokens[10].kind);

    assert_eq!(TkKind::Ident, tokens[11].kind);
    assert_eq!(Some(TkVal::String("str".to_string())), tokens[11].val);
    assert_eq!(Span::new(modu, 54, 57), tokens[11].span);

    assert_eq!(TkKind::Assign, tokens[12].kind);

    assert_eq!(TkKind::StringLiteral, tokens[13].kind);
    assert_eq!(Some(TkVal::String("string".to_string())), tokens[13].val);
    assert_eq!(Span::new(modu, 60, 68), tokens[13].span);

    assert_eq!(TkKind::SemiColon, tokens[14].kind);

    assert_eq!(TkKind::RBrace, tokens[15].kind);
}
