use biwac_base::{ModPath, Pos, Span};

use crate::{TkKind, TkVal};

#[test]
fn test1() {
    let modu = ModPath::Main;

    // NOTE: Rustの生文字列の扱いでは以下の場合
    // から文字列の0行目が含まれ、fnは1行目となるため注意
    let src = r#"
fn foo() {
    let x = 0;
    // comment "
    let str = "string";
}
"#;

    let tokens = crate::lex(modu.clone(), src).unwrap();

    assert_eq!(TkKind::Fn, tokens[0].kind);

    assert_eq!(TkKind::Ident, tokens[1].kind);
    assert_eq!(Some(TkVal::String("foo".to_string())), tokens[1].val);
    assert_eq!(
        Span::new(modu, Pos::new(1, 3), Pos::new(1, 6),),
        tokens[1].span
    );

    assert_eq!(TkKind::LPare, tokens[2].kind);

    assert_eq!(TkKind::RPare, tokens[3].kind);
}
