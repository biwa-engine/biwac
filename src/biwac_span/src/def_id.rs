use std::hash::Hash;

use biwac_base::{ModId, PackageId};

/// 64 bit definition id constructed from [`PackageId`] and [`PackageLocalDefId`].
/// This is unique in global scope (inter-package) and inter-session,
/// so used in incremental compilation cache as global unique symbol id.
/// Uniqueness must be ensured by definition collector implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct DefId {
    pkg: PackageId,
    local: PackageLocalDefId,
}

impl DefId {
    #[inline]
    pub fn new(pkg: PackageId, local: PackageLocalDefId) -> Self {
        Self { pkg, local }
    }

    #[inline]
    pub fn new_in_self_pkg(local: PackageLocalDefId) -> Self {
        Self {
            pkg: PackageId::SELF_PACKAGE,
            local,
        }
    }

    #[inline]
    pub fn pkg(&self) -> PackageId {
        self.pkg
    }

    #[inline]
    fn as_u64(&self) -> u64 {
        ((self.pkg.value() as u64) << 32) + self.local.0 as u64
    }
}

impl Hash for DefId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // `local` has higher entropy than `pkg`.
        self.local.hash(state);
        self.pkg.hash(state);
    }
}

/// 32 bit local definition id (simple increment).
/// This is unique inter session.
/// Uniqueness must be ensured by definition collector implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PackageLocalDefId(u32);

impl PackageLocalDefId {
    #[inline]
    pub fn new(id: u32) -> Self {
        Self(id)
    }
}

/// 32 bit local variable id (simple increment).
/// Unless its parent (function, associated function, or method) is not changed,
/// it is consistent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VarId(u32);

impl VarId {
    #[inline]
    pub fn new(id: u32) -> Self {
        Self(id)
    }

    pub fn value(&self) -> u32 {
        self.0
    }

    /// Variable `self` always assigned VarId(0).
    /// Normal arguments and variables must be 1 or bigger.
    pub const SELF_VARIABLE: Self = Self(0);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DefIdKind {
    Package(PackageId),
    Mod(ModId),
    Ty(TyDefId),
    Val(ValDefId),
    Gen(GenDefId),
    LocalGen(LocalGenDefId),
    Var(VarId),
}

macro_rules! impl_typed_def_id {
    ($typed_def_id:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub struct $typed_def_id(DefId);

        impl $typed_def_id {
            #[inline]
            pub fn new(def_id: DefId) -> Self {
                Self(def_id)
            }

            #[inline]
            pub fn pkg(&self) -> PackageId {
                self.0.pkg
            }

            #[inline]
            pub fn value(&self) -> u64 {
                self.0.as_u64()
            }

            #[inline]
            pub fn local_idx(&self) -> u32 {
                self.0.local.0
            }
        }
    };
}

impl_typed_def_id!(TyDefId);
impl_typed_def_id!(ValDefId);

// 型定義側で
// 宣言されるジェネリクス型に割り当てられるid
// GenDefIdに対するTyの割り当て(HashMap<GenDefId, Ty>)を保持することで、
// あるジェネリック型の使用箇所におけるのメンバなどへの型付けを計算できる
//  ```
//  struct Foo[T, U] {
//            ^^^^^^
//      x: T,
//      y: U,
//      z: Int,
//  }
//  ```
impl_typed_def_id!(GenDefId);

// impl block や fn のローカルなスコープで宣言された
// ジェネリック型に通しで振られるid
// ```
//  impl[T] Foo[T, Int] {
//      ^^^
//      fn bar[U](self) -> Baz[T, U] {
//            ^^^
//          ...
//      }
//  }
// ```
impl_typed_def_id!(LocalGenDefId);
