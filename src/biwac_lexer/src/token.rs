use biwac_base::Span;

#[derive(Clone, Debug)]
pub struct Token<'src> {
    pub kind: TkKind<'src>,
    pub span: Span,
    pub val: Option<TkVal>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TkVal {
    Integer(u64),
    String(String),
}

impl Token<'_> {
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
pub enum TkKind<'src> {
    Ident(&'src str),         // identifier
    LiteralInteger(u64),      // integer literal
    LiteralString(&'src str), // string literal
    KwBoolTrue,               // bool literal `TRUE`
    KwBoolFalse,              // bool literal `FALSE`
    KwImport,                 // import
    KwPackage,                // package
    KwFn,                     // fn
    KwType,                   // type
    KwLet,                    // let
    KwIf,                     // if
    KwElse,                   // else
    KwWhile,                  // while
    KwReturn,                 // return
    KwUint,                   // Uint (reserved word of type)
    KwInt,                    // Int (reserved word of type)
    KwFloat,                  // Float (reserved word of type)
    KwBool,                   // Bool (reserved word of type)
    KwStruct,                 // struct (reserved word of type)
    KwImpl,                   // impl (reserved word of implementation for type)
    KwSelfTyp,                // Self (reserved word of type)
    KwSelfVar,                // self (reserved word of method value)
    KwScene,                  // scene (reserved word of novel scene)
    MarkLPare,                // (
    MarkRPare,                // )
    MarkLBrace,               // {
    MarkRBrace,               // }
    MarkLBracket,             // [
    MarkRBracket,             // ]
    MarkPlus,                 // +
    MarkMinus,                // -
    MarkAsterisk,             // *
    MarkSlash,                // /
    MarkPercent,              // %
    MarkAmpersand,            // &
    MarkLesser,               // <
    MarkGreater,              // >
    MarkLesEq,                // <=
    MarkGrtEq,                // >=
    MarkEqual,                // ==
    MarkNotEq,                // !=
    MarkAssign,               // =
    MarkComma,                // ,
    MarkDot,                  // .
    MarkArrow,                // ->
    MarkColon,                // :
    MarkSemiColon,            // ;
    MarkDoubleColon,          // ::
    MarkDoubleLBrace,         // {{
    MarkDoubleRBrace,         // }}
    DslLiteral,               // DSL
}

impl TkKind<'_> {
    pub fn pattern(&self) -> String {
        match self {
            Self::Ident(i) => i.to_string(),
            Self::LiteralInteger(i) => i.to_string(),
            Self::LiteralString(s) => s.to_string(),
            Self::KwBoolTrue => "TRUE".to_string(),
            Self::KwBoolFalse => "FALSE".to_string(),
            Self::KwImport => "import".to_string(),
            Self::KwPackage => "package".to_string(),
            Self::KwFn => "fn".to_string(),
            Self::KwType => "type".to_string(),
            Self::KwLet => "let".to_string(),
            Self::KwIf => "if".to_string(),
            Self::KwElse => "else".to_string(),
            Self::KwWhile => "while".to_string(),
            Self::KwReturn => "return".to_string(),
            Self::KwUint => "Uint".to_string(),
            Self::KwInt => "Int".to_string(),
            Self::KwFloat => "Float".to_string(),
            Self::KwBool => "Bool".to_string(),
            Self::KwStruct => "struct".to_string(),
            Self::KwImpl => "impl".to_string(),
            Self::KwSelfTyp => "Self".to_string(),
            Self::KwSelfVar => "self".to_string(),
            Self::KwScene => "scene".to_string(),
            Self::MarkLPare => "(".to_string(),
            Self::MarkRPare => ")".to_string(),
            Self::MarkLBrace => "{".to_string(),
            Self::MarkRBrace => "}".to_string(),
            Self::MarkLBracket => "[".to_string(),
            Self::MarkRBracket => "]".to_string(),
            Self::MarkPlus => "+".to_string(),
            Self::MarkMinus => "-".to_string(),
            Self::MarkAsterisk => "*".to_string(),
            Self::MarkSlash => "/".to_string(),
            Self::MarkPercent => "%".to_string(),
            Self::MarkAmpersand => "&".to_string(),
            Self::MarkLesser => "<".to_string(),
            Self::MarkGreater => ">".to_string(),
            Self::MarkLesEq => "<=".to_string(),
            Self::MarkGrtEq => ">=".to_string(),
            Self::MarkEqual => "==".to_string(),
            Self::MarkNotEq => "!=".to_string(),
            Self::MarkAssign => "=".to_string(),
            Self::MarkComma => ",".to_string(),
            Self::MarkDot => ".".to_string(),
            Self::MarkArrow => "->".to_string(),
            Self::MarkColon => ":".to_string(),
            Self::MarkSemiColon => ";".to_string(),
            Self::MarkDoubleColon => "::".to_string(),
            Self::MarkDoubleLBrace => "{{".to_string(),
            Self::MarkDoubleRBrace => "}}".to_string(),
            Self::DslLiteral => "...".to_string(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TkKindName {
    Ident,            // identifier
    LiteralInteger,   // integer literal
    LiteralString,    // string literal
    KwBoolTrue,       // bool literal `TRUE`
    KwBoolFalse,      // bool literal `FALSE`
    KwImport,         // import
    KwPackage,        // package
    KwFn,             // fn
    KwType,           // type
    KwLet,            // let
    KwIf,             // if
    KwElse,           // else
    KwWhile,          // while
    KwReturn,         // return
    KwUint,           // Uint (reserved word of type)
    KwInt,            // Int (reserved word of type)
    KwFloat,          // Float (reserved word of type)
    KwBool,           // Bool (reserved word of type)
    KwStruct,         // struct (reserved word of type)
    KwImpl,           // impl (reserved word of implementation for type)
    KwSelfTyp,        // Self (reserved word of type)
    KwSelfVar,        // self (reserved word of method value)
    KwScene,          // scene (reserved word of novel scene)
    MarkLPare,        // (
    MarkRPare,        // )
    MarkLBrace,       // {
    MarkRBrace,       // }
    MarkLBracket,     // [
    MarkRBracket,     // ]
    MarkPlus,         // +
    MarkMinus,        // -
    MarkAsterisk,     // *
    MarkSlash,        // /
    MarkPercent,      // %
    MarkAmpersand,    // &
    MarkLesser,       // <
    MarkGreater,      // >
    MarkLesEq,        // <=
    MarkGrtEq,        // >=
    MarkEqual,        // ==
    MarkNotEq,        // !=
    MarkAssign,       // =
    MarkComma,        // ,
    MarkDot,          // .
    MarkArrow,        // ->
    MarkColon,        // :
    MarkSemiColon,    // ;
    MarkDoubleColon,  // ::
    MarkDoubleLBrace, // {{
    MarkDoubleRBrace, // }}
    DslLiteral,       // DSL
}

impl TkKind<'_> {
    pub fn as_name(&self) -> TkKindName {
        match self {
            Self::Ident(_) => TkKindName::Ident, // identifier
            Self::LiteralInteger(_) => TkKindName::LiteralInteger, // integer literal
            Self::LiteralString(_) => TkKindName::LiteralString, // string literal
            Self::KwBoolTrue => TkKindName::KwBoolTrue, // bool literal `TRUE`
            Self::KwBoolFalse => TkKindName::KwBoolFalse, // bool literal `FALSE`
            Self::KwImport => TkKindName::KwImport, // import
            Self::KwPackage => TkKindName::KwPackage, // package
            Self::KwFn => TkKindName::KwFn,      // fn
            Self::KwType => TkKindName::KwType,  // type
            Self::KwLet => TkKindName::KwLet,    // let
            Self::KwIf => TkKindName::KwIf,      // if
            Self::KwElse => TkKindName::KwElse,  // else
            Self::KwWhile => TkKindName::KwWhile, // while
            Self::KwReturn => TkKindName::KwReturn, // return
            Self::KwUint => TkKindName::KwUint,  // Uint (reserved word of type)
            Self::KwInt => TkKindName::KwInt,    // Int (reserved word of type)
            Self::KwFloat => TkKindName::KwFloat, // Float (reserved word of type)
            Self::KwBool => TkKindName::KwBool,  // Bool (reserved word of type)
            Self::KwStruct => TkKindName::KwStruct, // struct (reserved word of type)
            Self::KwImpl => TkKindName::KwImpl,  // impl (reserved word of implementation for type)
            Self::KwSelfTyp => TkKindName::KwSelfTyp, // Self (reserved word of type)
            Self::KwSelfVar => TkKindName::KwSelfVar, // self (reserved word of method value)
            Self::KwScene => TkKindName::KwScene, // scene (reserved word of novel scene)
            Self::MarkLPare => TkKindName::MarkLPare, // (
            Self::MarkRPare => TkKindName::MarkRPare, // )
            Self::MarkLBrace => TkKindName::MarkLBrace, // {
            Self::MarkRBrace => TkKindName::MarkRBrace, // }
            Self::MarkLBracket => TkKindName::MarkLBracket, // [
            Self::MarkRBracket => TkKindName::MarkRBracket, // ]
            Self::MarkPlus => TkKindName::MarkPlus, // +
            Self::MarkMinus => TkKindName::MarkMinus, // -
            Self::MarkAsterisk => TkKindName::MarkAsterisk, // *
            Self::MarkSlash => TkKindName::MarkSlash, // /
            Self::MarkPercent => TkKindName::MarkPercent, // %
            Self::MarkAmpersand => TkKindName::MarkAmpersand, // &
            Self::MarkLesser => TkKindName::MarkLesser, // <
            Self::MarkGreater => TkKindName::MarkGreater, // >
            Self::MarkLesEq => TkKindName::MarkLesEq, // <=
            Self::MarkGrtEq => TkKindName::MarkGrtEq, // >=
            Self::MarkEqual => TkKindName::MarkEqual, // ==
            Self::MarkNotEq => TkKindName::MarkNotEq, // !=
            Self::MarkAssign => TkKindName::MarkAssign, // =
            Self::MarkComma => TkKindName::MarkComma, // ,
            Self::MarkDot => TkKindName::MarkDot, // .
            Self::MarkArrow => TkKindName::MarkArrow, // ->
            Self::MarkColon => TkKindName::MarkColon, // :
            Self::MarkSemiColon => TkKindName::MarkSemiColon, // ;
            Self::MarkDoubleColon => TkKindName::MarkDoubleColon, // ::
            Self::MarkDoubleLBrace => TkKindName::MarkDoubleLBrace, // {{
            Self::MarkDoubleRBrace => TkKindName::MarkDoubleRBrace, // }}
            Self::DslLiteral => TkKindName::DslLiteral, // DSL
        }
    }
}

impl TkKindName {
    pub fn pattern(&self) -> String {
        match self {
            Self::Ident => "<identifier>".to_string(),
            Self::LiteralInteger => "<integer-literal>".to_string(),
            Self::LiteralString => "<string-literal>".to_string(),
            Self::KwBoolTrue => "TRUE".to_string(),
            Self::KwBoolFalse => "FALSE".to_string(),
            Self::KwImport => "import".to_string(),
            Self::KwPackage => "package".to_string(),
            Self::KwFn => "fn".to_string(),
            Self::KwType => "type".to_string(),
            Self::KwLet => "let".to_string(),
            Self::KwIf => "if".to_string(),
            Self::KwElse => "else".to_string(),
            Self::KwWhile => "while".to_string(),
            Self::KwReturn => "return".to_string(),
            Self::KwUint => "Uint".to_string(),
            Self::KwInt => "Int".to_string(),
            Self::KwFloat => "Float".to_string(),
            Self::KwBool => "Bool".to_string(),
            Self::KwStruct => "struct".to_string(),
            Self::KwImpl => "impl".to_string(),
            Self::KwSelfTyp => "Self".to_string(),
            Self::KwSelfVar => "self".to_string(),
            Self::KwScene => "scene".to_string(),
            Self::MarkLPare => "(".to_string(),
            Self::MarkRPare => ")".to_string(),
            Self::MarkLBrace => "{".to_string(),
            Self::MarkRBrace => "}".to_string(),
            Self::MarkLBracket => "[".to_string(),
            Self::MarkRBracket => "]".to_string(),
            Self::MarkPlus => "+".to_string(),
            Self::MarkMinus => "-".to_string(),
            Self::MarkAsterisk => "*".to_string(),
            Self::MarkSlash => "/".to_string(),
            Self::MarkPercent => "%".to_string(),
            Self::MarkAmpersand => "&".to_string(),
            Self::MarkLesser => "<".to_string(),
            Self::MarkGreater => ">".to_string(),
            Self::MarkLesEq => "<=".to_string(),
            Self::MarkGrtEq => ">=".to_string(),
            Self::MarkEqual => "==".to_string(),
            Self::MarkNotEq => "!=".to_string(),
            Self::MarkAssign => "=".to_string(),
            Self::MarkComma => ",".to_string(),
            Self::MarkDot => ".".to_string(),
            Self::MarkArrow => "->".to_string(),
            Self::MarkColon => ":".to_string(),
            Self::MarkSemiColon => ";".to_string(),
            Self::MarkDoubleColon => "::".to_string(),
            Self::MarkDoubleLBrace => "{{".to_string(),
            Self::MarkDoubleRBrace => "}}".to_string(),
            Self::DslLiteral => "...".to_string(),
        }
    }
}
