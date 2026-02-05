pub mod expressions;
pub mod globals;
pub mod statements;
pub mod types;

use std::{
    collections::{HashMap, hash_map::Entry},
    fmt::Display,
};

use crate::{
    packager::{
        loader::PackageSymbolMap,
        resolver::symbols::{
            ModuleSymbols,
            globals::{FnDefContent, StructDefContent, TypeDefContent},
        },
    },
    parser::symbols::QualifiedId,
    validator::{
        expressions::Exprs,
        globals::Function,
        types::{AbsoluteType, PrimitiveType},
    },
};

pub fn validate(
    // modpath: &[&str],
    pkg: &PackageSymbolMap,
    // signtree: &SignatureTree,
    // prog: &crate::parser::symbols::Program,
) -> Result<Package, ValidateError> {
    // let mut env = Env::new(modpath, signtree, prog);
    let mut env = Env::new(pkg);
    let mut fns = HashMap::new();
    let mut global_vars = HashMap::new();
    let mut types = HashMap::new();

    for (id, sym) in pkg.syms().iter() {
        match sym {
            ModuleSymbols::VarDec(v) => {
                global_vars.insert(id.clone(), v.init.validate(&mut env)?);
            }
            ModuleSymbols::TypeDef(t) => {
                types.insert(id.clone(), t.clone());
            }
            ModuleSymbols::FnDef(f) => {
                env.begin_local(id.clone(), &f.args, f.rtype.clone());
                fns.insert(id.clone(), f.validate(&mut env)?);

                env.end_local();
            }
        }
    }

    // for g in &prog.globals {
    //     match g {
    //         symbols::globals::Globals::LanglibfnDec(_) => {
    //             // nothing to do
    //         }
    //         symbols::globals::Globals::Import(_) => {
    //             // nothing to do
    //         }
    //         symbols::globals::Globals::FnDef(f) => {
    //             // for next codegen path, validate function body
    //             env.begin_local(
    //                 &f.args
    //                     .iter()
    //                     .map(|(typ, id)| typ.clone().into_abs(&env).map(|abs| (abs, id)))
    //                     .collect::<Result<Vec<(AbsoluteType, &String)>, ValidateError>>()?,
    //                 f.rtype.clone(),
    //             );
    //
    //             fns.insert(
    //                 AbsoluteId {
    //                     quals: env
    //                         .global
    //                         .modpath
    //                         .iter()
    //                         .map(|module| module.to_string())
    //                         .collect(),
    //                     id: f.name.clone(),
    //                 },
    //                 f.validate(&mut env)?,
    //             );
    //
    //             env.end_local();
    //         }
    //         symbols::globals::Globals::VarDec(var) => {
    //             // insert variable to env global variable list
    //             env.global
    //                 .vars
    //                 .insert(var.name.clone(), var.typ.clone().into_abs(&env)?);
    //
    //             let (init_typ, init) = var.init.validate(&mut env)?;
    //
    //             // NOTE: 変数の型がIntでかつ初期化の右辺値がUintと判定されるときを除き、
    //             // 型が一致していなければエラー
    //             if !(var.typ.clone().into_abs(&env)?.equals(&init_typ, &env)
    //                 || (matches!(init_typ, AbsoluteType::Primitive(PrimitiveType::Uint))
    //                     && matches!(var.typ, Type::Primitive(PrimitiveType::Int))))
    //             {
    //                 return Err(ValidateError::Mismatch(
    //                     Box::new(var.typ.clone().into_abs(&env)?),
    //                     Box::new(init_typ),
    //                 ));
    //             }
    //             // for next codegen path, insert var
    //             global_vars.insert(var.name.clone(), (var.typ.clone().into_abs(&env)?, init));
    //         }
    //         symbols::globals::Globals::TypeDef(_) => {
    //             // nothing to do
    //         }
    //     }
    // }

    Ok(Package {
        types,
        fns,
        global_vars,
        // string_literals: env.global.string_literals,
    })
}

#[derive(Debug)]
pub enum ValidateError {
    // something not found
    VariableNotFound(String),
    FunctionNotFound(AbsoluteId),
    TypeNotFound(AbsoluteId),
    StructMemberNotFound(AbsoluteId, String), // struct name, member string
    // something conflict
    VariableConflict(String),
    StructMemberConflict(String, String), // struct name, member string
    TypeConflict(String),
    // type mismatch
    ArgumentMismatch(Option<Box<AbsoluteType>>, Option<Box<AbsoluteType>>), // callee type, calling type
    Mismatch(Box<AbsoluteType>, Box<AbsoluteType>), // outer type, inner type
    // operation not allowed for the type
    StructNotAssignable(String),                 // struct name
    TypeAndOperatorNotSupported(String, String), // type name, op string
    // validator bug
    OutOfScopes,
}

#[derive(Debug)]
pub struct Package {
    // pub string_literals: HashMap<String, usize>,
    pub types: HashMap<AbsoluteId, TypeDefContent>,
    pub fns: HashMap<AbsoluteId, Function>,
    pub global_vars: HashMap<AbsoluteId, (AbsoluteType, Exprs)>,
}

#[derive(Debug)]
pub struct Env<'parsed> {
    pkg: &'parsed PackageSymbolMap,
    local: Option<EnvLocal>,
}

#[derive(Debug)]
struct EnvLocal {
    absid: AbsoluteId,
    rtype: Option<AbsoluteType>,
    vars: Vec<HashMap<String, Variable>>,
}

impl<'parsed> Env<'parsed> {
    fn new(pkg: &'parsed PackageSymbolMap) -> Self {
        Self { pkg, local: None }
    }

    fn begin_local(
        &mut self,
        absid: AbsoluteId,
        args: &[(AbsoluteType, String)],
        rtype: Option<AbsoluteType>,
    ) {
        self.local = Some(EnvLocal {
            absid,
            rtype: rtype.clone(),
            vars: vec![HashMap::new()],
        });

        for (atyp, aid) in args {
            if let Err(e) = self.insert_var(aid.clone(), atyp.clone()) {
                match e {
                    ValidateError::OutOfScopes => {
                        panic!("Compiler Error, Out of Scopes");
                    }
                    ValidateError::VariableConflict(var) => {
                        panic!("Function Arg Name Conflicting: {var}");
                    }
                    _ => {
                        panic!("Compiler Error, Unknown");
                    }
                }
            }
        }

        if let Some(local) = &mut self.local {
            local.rtype = rtype.clone();
        }
    }

    fn end_local(&mut self) {
        self.local = None;
    }

    pub fn begin_scope(&mut self) {
        if let Some(local) = &mut self.local {
            local.vars.push(HashMap::new());
        }
    }

    pub fn end_scope(&mut self) {
        if let Some(local) = &mut self.local {
            local.vars.pop();
        }
    }

    pub fn insert_var(&mut self, id: String, typ: AbsoluteType) -> Result<(), ValidateError> {
        if let Some(local) = &mut self.local {
            if let Some(last_scope) = local.vars.last_mut() {
                match last_scope.entry(id.clone()) {
                    Entry::Vacant(e) => {
                        e.insert(Variable {
                            typ,
                            pos: VarPos::Local(id),
                        });

                        Ok(())
                    }

                    Entry::Occupied(_) => Err(ValidateError::VariableConflict(id)),
                }
            } else {
                Err(ValidateError::OutOfScopes)
            }
        } else {
            Err(ValidateError::OutOfScopes)
        }
    }

    pub fn get_var(&self, id: &String) -> Option<Variable> {
        if let Some(local) = &self.local {
            for scope in local.vars.iter().rev() {
                if let Some(var) = scope.get(id) {
                    return Some(var.clone());
                }
            }

            let global_var_absid = AbsoluteId::new(local.absid.quals.clone(), id.clone());

            if let Some(ModuleSymbols::VarDec(g)) = self.pkg.syms().get(&global_var_absid) {
                Some(Variable {
                    typ: g.typ.clone(),
                    pos: VarPos::Global(global_var_absid),
                })
            } else {
                None
            }
        } else {
            None
        }

        // NOTE: 他のmoduleのグローバル変数を参照できない
        // 言語機能として必要かどうかという議論もあるが
    }

    pub fn get_fnsign(&self, absid: &AbsoluteId) -> Option<&FnDefContent> {
        if let Some(ModuleSymbols::FnDef(f)) = self.pkg.syms().get(absid) {
            Some(f)
        } else {
            None
        }
    }

    pub fn fn_exists(&self, absid: &AbsoluteId) -> Result<(), ValidateError> {
        if self.pkg.syms().get(absid).is_some() {
            Ok(())
        } else {
            Err(ValidateError::FunctionNotFound(absid.clone()))
        }
    }

    pub fn get_structsign(&self, absid: &AbsoluteId) -> Option<&StructDefContent> {
        if let Some(ModuleSymbols::TypeDef(TypeDefContent::Struct(ssign))) =
            self.pkg.syms().get(absid)
        {
            Some(ssign)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AbsoluteId {
    // pub package: enum Package { Internal, External(String)}
    pub quals: Vec<String>,
    pub id: String,
}

impl AbsoluteId {
    pub fn new(quals: Vec<String>, id: String) -> Self {
        Self { quals, id }
    }

    pub fn equals(&self, other: &Self) -> bool {
        self.quals.eq(&other.quals) && self.id == other.id
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VarPos {
    Local(String),
    Global(AbsoluteId),
}

#[derive(Debug, Clone)]
pub struct Variable {
    pub typ: AbsoluteType,
    pub pos: VarPos,
}

impl ValidateError {
    pub fn panic_with_error_message(&self) -> ! {
        eprint!("\x1b[1;38;2;255;20;0merror\x1b[m: ");
        eprint!("\x1b[1m");
        match self {
            Self::VariableNotFound(var) => {
                eprint!("variable `{var}` not found");
            }
            Self::FunctionNotFound(f) => {
                eprint!("function `{f}` not found");
            }
            Self::TypeNotFound(qualed_id) => {
                eprint!("type `{qualed_id}` not found");
            }
            Self::StructMemberNotFound(s, mem) => {
                eprint!("member `{mem}` not found in struct `{s}`");
            }
            Self::VariableConflict(var) => {
                eprint!("variable `{var}` conflicting");
            }
            Self::ArgumentMismatch(callee_typ, calling_typ) => {
                if let Some(callee_typ) = callee_typ {
                    if let Some(calling_typ) = calling_typ {
                        eprint!(
                            "types mismatch in function call, expected `{callee_typ}`, but found `{calling_typ}`"
                        );
                    } else {
                        eprint!(
                            "types mismatch in function call, expected `{callee_typ}`, but found nothing"
                        );
                    }
                } else if let Some(calling_typ) = calling_typ {
                    eprint!(
                        "types mismatch in function call, expected nothing, but found `{calling_typ}`"
                    );
                } else {
                    eprint!("types mismatch in function call, expected nothing, but found nothing");
                    // ISSUE: ???
                }
            }
            Self::Mismatch(outer_typ, inner_typ) => {
                eprint!("types mismatch, expected `{outer_typ}`, but found `{inner_typ}`");
            }
            Self::StructNotAssignable(s) => {
                eprint!("assignment not allowed for `struct {s}`");
            }
            Self::TypeAndOperatorNotSupported(typ, op) => {
                eprint!("operator `{op}` not allowed for `{typ}`");
            }
            Self::StructMemberConflict(s, mem) => {
                eprint!("member `{mem}` conflicting in `struct {s}`");
            }
            Self::TypeConflict(typ) => {
                eprint!("type `{typ}` conflicting");
            }
            Self::OutOfScopes => {
                eprint!("unknown compiler error occured, sorry");
            }
        }

        eprintln!("\x1b[m");
        panic!("");
    }
}

impl Display for AbsoluteType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Primitive(p) => match p {
                PrimitiveType::Int => write!(f, "Int"),
                PrimitiveType::Uint => write!(f, "Uint"),
                PrimitiveType::Bool => write!(f, "Bool"),
            },
            Self::List(typ) => write!(f, "{typ}[]"),
            Self::Defined(absid) => {
                write!(f, "{absid}")
            }
        }
    }
}

impl Display for QualifiedId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_from_root {
            write!(
                f,
                "package::{}{}",
                self.quals
                    .iter()
                    .map(|q| format!("{q}::"))
                    .collect::<String>(),
                self.id
            )
        } else {
            write!(
                f,
                "{}{}",
                self.quals
                    .iter()
                    .map(|q| format!("{q}::"))
                    .collect::<String>(),
                self.id
            )
        }
    }
}

impl Display for AbsoluteId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "package::{}{}",
            self.quals
                .iter()
                .map(|q| format!("{q}::"))
                .collect::<String>(),
            self.id
        )
    }
}
