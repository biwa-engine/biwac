use biwac_base::Span;

#[derive(Clone, Debug)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TokenKind {
    Identifier(String),    // identifier
    IntLiteral(u32),       // int literal
    CharLiteral(u8),       // char literal
    StringLiteral(String), // string literal
    BoolLiteralTrue,       // bool literal `TRUE`
    BoolLiteralFalse,      // bool literal `FALSE`
    Langlibfn,             // langlibfn
    Import,                // import
    Package,               // package
    Fn,                    // fn
    Let,                   // let
    If,                    // if
    Else,                  // else
    While,                 // while
    Return,                // return
    SizeOf,                // sizeof
    Uint,                  // Uint (reserved word of type)
    Int,                   // Int (reserved word of type)
    Bool,                  // Bool (reserved word of type)
    Struct,                // struct (reserved word of type)
    LPare,                 // (
    RPare,                 // )
    LBrace,                // {
    RBrace,                // }
    LBracket,              // [
    RBracket,              // ]
    Plus,                  // +
    Minus,                 // -
    Asterisk,              // *
    Slash,                 // /
    Percent,               // %
    Ampersand,             // &
    Lesser,                // <
    Greater,               // >
    LesEq,                 // <=
    GrtEq,                 // >=
    Equal,                 // ==
    NotEq,                 // !=
    Assign,                // =
    Comma,                 // ,
    Dot,                   // .
    Arrow,                 // ->
    Colon,                 // :
    SemiColon,             // ;
    DoubleColon,           // ::
}

impl TokenKind {
    pub fn pattern(&self) -> String {
        match self {
            Self::Identifier(_) => "".to_string(),
            Self::IntLiteral(_) => "".to_string(),
            Self::CharLiteral(_) => "".to_string(),
            Self::StringLiteral(_) => "".to_string(),
            Self::BoolLiteralTrue => "TRUE".to_string(),
            Self::BoolLiteralFalse => "FALSE".to_string(),
            Self::Langlibfn => "langlibfn".to_string(),
            Self::Import => "import".to_string(),
            Self::Package => "package".to_string(),
            Self::Fn => "fn".to_string(),
            Self::Let => "let".to_string(),
            Self::If => "if".to_string(),
            Self::Else => "else".to_string(),
            Self::While => "while".to_string(),
            Self::Return => "return".to_string(),
            Self::SizeOf => "sizeof".to_string(),
            Self::Uint => "Uint".to_string(),
            Self::Int => "Int".to_string(),
            Self::Bool => "Bool".to_string(),
            Self::Struct => "struct".to_string(),
            Self::LPare => "(".to_string(),
            Self::RPare => ")".to_string(),
            Self::LBrace => "{".to_string(),
            Self::RBrace => "}".to_string(),
            Self::LBracket => "[".to_string(),
            Self::RBracket => "]".to_string(),
            Self::Plus => "+".to_string(),
            Self::Minus => "-".to_string(),
            Self::Asterisk => "*".to_string(),
            Self::Slash => "/".to_string(),
            Self::Percent => "%".to_string(),
            Self::Ampersand => "&".to_string(),
            Self::Lesser => "<".to_string(),
            Self::Greater => ">".to_string(),
            Self::LesEq => "<=".to_string(),
            Self::GrtEq => ">=".to_string(),
            Self::Equal => "==".to_string(),
            Self::NotEq => "!=".to_string(),
            Self::Assign => "=".to_string(),
            Self::Comma => ",".to_string(),
            Self::Dot => ".".to_string(),
            Self::Arrow => "->".to_string(),
            Self::Colon => ":".to_string(),
            Self::SemiColon => ";".to_string(),
            Self::DoubleColon => "::".to_string(),
        }
    }
}
