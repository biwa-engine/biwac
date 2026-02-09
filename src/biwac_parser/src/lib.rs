mod parser;
pub mod symbols;
pub mod types;

#[cfg(test)]
mod tests;

use biwac_lexer::{TkKind, Token};

pub use symbols::{
    Ident, ModAst, QualifiedId,
    expressions::{
        BinOperator, BinaryExpr, BoolLiteral, Exprs, FnCall, IntegerLiteral, Literal, MemberAccess,
        Primary, StringLiteral, StructLiteral, UnOperator, UnaryExpr,
    },
    globals::{FnDef, Globals, ImportDecl, StructDef, TypeDef},
    statements::{
        AssignStmt, ExprStmt, ReturnStmt, Stmt, block::BlockStmt, if_stmt::IfStmt, vardec::VarDecl,
        while_stmt::WhileStmt,
    },
};
pub use types::{DefTyp, GenArg, PrimTyp, TypRepr};

#[derive(Debug, Clone)]
pub enum ParseError {
    InvalidToken(Vec<TkKind>, Token), // expected TokenKind, ... or TokenKind, but found Token in Token.range
    InvalidEOF(Vec<TkKind>),          // expected TokenKind, ... or TokenKind, but found EOF
    StructMemberConflict(String, String, Box<Token>), // struct id, member id
}

//
// impl ParseError {
//     pub fn panic_with_error_message(&self, src: &str) -> ! {
//         eprint!("\x1b[1;38;2;255;20;0merror\x1b[m: ");
//         match self {
//             Self::InvalidToken(expecteds, found) => {
//                 eprintln!(
//                     "\x1b[1mexpected {}, but found {} in the {}th character\x1b[m",
//                     string_of(expecteds),
//                     found.kind,
//                     found.range.begin
//                 );
//                 eprintln!(
//                     "... {}\x1b[4m{}\x1b[m{} ...",
//                     &src[0.max(found.range.begin - 10)..found.range.begin],
//                     &src[found.range.begin..found.range.end],
//                     &src[found.range.end..src.len().min(found.range.end + 10)],
//                 );
//             }
//             Self::InvalidEOF(expecteds) => {
//                 eprintln!(
//                     "\x1b[1mexpected {}, but found EOF\x1b[m",
//                     string_of(expecteds),
//                 );
//                 eprintln!(
//                     "... {}\x1b[4m{}\x1b[m",
//                     &src[0.max(src.len() - 20)..0.max(src.len() - 5)],
//                     &src[0.max(src.len() - 5)..],
//                 );
//             }
//             Self::StructMemberConflict(structid, memberid, range) => {
//                 eprintln!("\x1b[1mmember `{memberid}` in struct `{structid}` is conflicted\x1b[m",);
//                 eprintln!(
//                     "... {}\x1b[4m{}\x1b[m{} ...",
//                     &src[0.max(range.begin - 10)..range.begin],
//                     &src[range.begin..range.end],
//                     &src[range.end..src.len().min(range.end + 10)],
//                 );
//             }
//         }
//
//         panic!("");
//     }
// }
//
// impl Display for TokenKind {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         match self {
//             Self::Identifier(s) => {
//                 if s.is_empty() {
//                     write!(f, "identifier")
//                 } else {
//                     write!(f, "identifier `{s}`")
//                 }
//             }
//             Self::IntLiteral(i) => write!(f, "literal `{i}`"),
//             Self::CharLiteral(c) => write!(f, "literal `'{c}'`"),
//             Self::StringLiteral(s) => write!(f, "literal `\"{s}\"`"),
//             _ => write!(f, "`{}`", self.pattern()),
//         }
//     }
// }
//
// fn string_of(kinds: &[TokenKind]) -> String {
//     if kinds.is_empty() {
//         "".to_string()
//     } else if kinds.len() == 1 {
//         kinds.first().unwrap().to_string()
//     } else if kinds.len() == 2 {
//         format!("{} or {}", kinds.first().unwrap(), kinds.last().unwrap())
//     } else {
//         format!(
//             "{}{} or {}",
//             &kinds[..kinds.len() - 2]
//                 .iter()
//                 .map(|kind| format!("{kind}, "))
//                 .collect::<String>(),
//             &kinds[kinds.len() - 2..].first().unwrap(),
//             &kinds[kinds.len() - 2..].last().unwrap()
//         )
//     }
// }
