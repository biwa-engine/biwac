use biwac_base::Span;

#[derive(Clone, Debug)]
pub struct Token {
    pub kind: TkKind,
    pub span: Span,
    pub val: Option<TkVal>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TkVal {
    Integer(u64),
    String(String),
}

impl Token {
    pub fn unwrap_integer_value(&self) -> u64 {
        if let Some(TkVal::Integer(i)) = &self.val {
            *i
        } else {
            panic!("compiler bug: no integer value token unwrapped as integer")
        }
    }

    pub fn unwrap_string_value(&self) -> String {
        if let Some(TkVal::String(s)) = &self.val {
            s.clone()
        } else {
            panic!("compiler bug: no string value token unwrapped as string")
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TkKind {
    Ident,            // identifier
    IntegerLiteral,   // integer literal
    StringLiteral,    // string literal
    BoolLiteralTrue,  // bool literal `TRUE`
    BoolLiteralFalse, // bool literal `FALSE`
    Import,           // import
    Package,          // package
    Fn,               // fn
    Type,             // type
    Let,              // let
    If,               // if
    Else,             // else
    While,            // while
    Return,           // return
    Uint,             // Uint (reserved word of type)
    Int,              // Int (reserved word of type)
    Float,            // Float (reserved word of type)
    Bool,             // Bool (reserved word of type)
    Struct,           // struct (reserved word of type)
    Impl,             // impl (reserved word of implementation for type)
    SelfTyp,          // Self (reserved word of type)
    SelfVar,          // self (reserved word of method value)
    LPare,            // (
    RPare,            // )
    LBrace,           // {
    RBrace,           // }
    LBracket,         // [
    RBracket,         // ]
    Plus,             // +
    Minus,            // -
    Asterisk,         // *
    Slash,            // /
    Percent,          // %
    Ampersand,        // &
    Lesser,           // <
    Greater,          // >
    LesEq,            // <=
    GrtEq,            // >=
    Equal,            // ==
    NotEq,            // !=
    Assign,           // =
    Comma,            // ,
    Dot,              // .
    Arrow,            // ->
    Colon,            // :
    SemiColon,        // ;
    DoubleColon,      // ::
    DoubleLBrace,     // {{
    DoubleRBrace,     // }}
    DslLiteral,       // DSL
}

impl TkKind {
    pub fn pattern(&self) -> String {
        match self {
            Self::Ident => "".to_string(),
            Self::IntegerLiteral => "".to_string(),
            Self::StringLiteral => "".to_string(),
            Self::BoolLiteralTrue => "TRUE".to_string(),
            Self::BoolLiteralFalse => "FALSE".to_string(),
            Self::Import => "import".to_string(),
            Self::Package => "package".to_string(),
            Self::Fn => "fn".to_string(),
            Self::Type => "type".to_string(),
            Self::Let => "let".to_string(),
            Self::If => "if".to_string(),
            Self::Else => "else".to_string(),
            Self::While => "while".to_string(),
            Self::Return => "return".to_string(),
            Self::Uint => "Uint".to_string(),
            Self::Int => "Int".to_string(),
            Self::Float => "Float".to_string(),
            Self::Bool => "Bool".to_string(),
            Self::Struct => "struct".to_string(),
            Self::Impl => "impl".to_string(),
            Self::SelfTyp => "Self".to_string(),
            Self::SelfVar => "self".to_string(),
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
            Self::DoubleLBrace => "{{".to_string(),
            Self::DoubleRBrace => "}}".to_string(),
            Self::DslLiteral => "...".to_string(),
        }
    }
}
