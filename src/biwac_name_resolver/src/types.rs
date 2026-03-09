// use biwac_parser::{PrimTyp, TypRepr, TypReprVal};
//
// use crate::{ImplLevelTryResolve, ModuleLevelTryResolve, TryResolve, TypId};
//
// #[derive(Debug, Clone, PartialEq, Eq, Hash)]
// pub enum Typ {
//     Int,
//     Float,
//     Bool,
//     Fn(FnTyp),
//     Defined(DefinedTyp),
//     Gen(GenTypId),
// }
//
// #[derive(Debug, Clone, PartialEq, Eq, Hash)]
// pub struct DefinedTyp {
//     pub id: TypId,
//     pub genargs: Vec<Typ>,
// }
//
// #[derive(Debug, Clone, PartialEq, Eq, Hash)]
// pub struct FnTyp {
//     pub args: Vec<Typ>,
//     pub ret: Box<Typ>,
//     // pub genargs: Vec<String>,
// }
//
// #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
// pub struct GenTypId(usize);
//
// impl GenTypId {
//     pub(super) fn new(id: usize) -> Self {
//         Self(id)
//     }
//
//     pub(crate) fn value(&self) -> usize {
//         self.0
//     }
// }
//
// impl ModuleLevelTryResolve<&TypRepr> for Typ {
//     fn try_resolve_in_module<'pctx>(
//         value: &TypRepr,
//         mctx: &crate::context::ModLvlRslvCtx<'pctx>,
//     ) -> crate::RsvResult<Self> {
//         match &value.val {
//             TypReprVal::Primitive(p) => match p {
//                 PrimTyp::Int => Ok(Typ::Int),
//                 PrimTyp::Uint => Ok(Typ::Int), // TODO
//                 PrimTyp::Float => Ok(Typ::Float),
//                 PrimTyp::Bool => Ok(Typ::Bool),
//             },
//             TypReprVal::Defined(deftyp) => Ok(Typ::Defined(DefinedTyp {
//                 id: mctx.try_resolve_deftyp(&deftyp.qualid)?,
//                 genargs: deftyp
//                     .genargs
//                     .iter()
//                     .map(|typ| Typ::try_resolve_in_module(typ, mctx))
//                     .collect::<Result<_, _>>()?,
//             })),
//         }
//     }
// }
//
// // impl block内レベルの型の解決
// //
// // impl[T] Foo[T] { ... }
// //         ^^^^^^
// //         ここまで
// //
// // impl[T] T { ... }
// //         ^
// //         ここまで
// impl ImplLevelTryResolve<&TypRepr> for Typ {
//     fn try_resolve_in_impl<'mctx>(
//         value: &TypRepr,
//         ictx: &crate::context::ImplLvlGenTypRslvCtx<'mctx>,
//     ) -> crate::RsvResult<Self> {
//         match &value.val {
//             TypReprVal::Primitive(p) => match p {
//                 PrimTyp::Int => Ok(Typ::Int),
//                 PrimTyp::Uint => Ok(Typ::Int), // TODO
//                 PrimTyp::Float => Ok(Typ::Float),
//                 PrimTyp::Bool => Ok(Typ::Bool),
//             },
//             TypReprVal::Defined(deftyp) => {
//                 if let Some(id) = deftyp.qualid.only_id()
//                     && let Some(gid) = ictx.impl_genargs.get(id)
//                 {
//                     Ok(Typ::Gen(*gid))
//                 } else {
//                     Ok(Typ::Defined(DefinedTyp {
//                         id: ictx.mctx.try_resolve_deftyp(&deftyp.qualid)?,
//                         genargs: deftyp
//                             .genargs
//                             .iter()
//                             .map(|typ| Typ::try_resolve_in_impl(typ, ictx))
//                             .collect::<Result<_, _>>()?,
//                     }))
//                 }
//             }
//         }
//     }
// }
//
// impl TryResolve<&TypRepr> for Typ {
//     fn try_resolve<'mctx>(
//         value: &TypRepr,
//         fctx: &mut crate::context::FnLvlRslvCtx<'mctx>,
//     ) -> crate::RsvResult<Self> {
//         match &value.val {
//             TypReprVal::Primitive(p) => match p {
//                 PrimTyp::Int => Ok(Typ::Int),
//                 PrimTyp::Uint => Ok(Typ::Int), // TODO
//                 PrimTyp::Float => Ok(Typ::Float),
//                 PrimTyp::Bool => Ok(Typ::Bool),
//             },
//             TypReprVal::Defined(deftyp) => {
//                 // deftypがidのみ(ex: `T`)の場合、
//                 // 内側から名前解決する
//                 //
//                 //  (3) それ以外の外部に定義された型から解決が試みられる
//                 //
//                 //  impl[T] Foo[T] {
//                 //       ^
//                 //       | (2)次に解決が試みられる
//                 //      fn bar[T]() { ... }
//                 //             ^
//                 //             | (1)まず解決が試みられる
//                 //  }
//                 if let Some(id) = deftyp.qualid.only_id()
//                     && let Some(gid) = fctx.genargs.get(id)
//                 {
//                     Ok(Typ::Gen(*gid))
//                 } else if let Some(id) = deftyp.qualid.only_id()
//                     && let Some(ictx) = &fctx.ictx
//                     && let Some(gid) = ictx.impl_genargs.get(id)
//                 {
//                     Ok(Typ::Gen(*gid))
//                 } else {
//                     Ok(Typ::Defined(DefinedTyp {
//                         id: fctx.try_resolve_deftyp(&deftyp.qualid)?,
//                         genargs: deftyp
//                             .genargs
//                             .iter()
//                             .map(|typ| Typ::try_resolve(typ, fctx))
//                             .collect::<Result<_, _>>()?,
//                     }))
//                 }
//             }
//         }
//     }
// }
//
// impl Typ {
//     pub(crate) fn get_genargs(&self) -> Vec<Typ> {
//         match self {
//             Self::Defined(deftyp) => deftyp.genargs.clone(),
//             _ => vec![],
//         }
//     }
// }
