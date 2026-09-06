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
    const VOID_DEF_ID: Self = Self::new_reserved(PackageLocalDefId::VOID_LOCAL_DEF_ID);

    const INT_DEF_ID: Self = Self::new_reserved(PackageLocalDefId::INT_LOCAL_DEF_ID);

    const UINT_DEF_ID: Self = Self::new_reserved(PackageLocalDefId::UINT_LOCAL_DEF_ID);

    const FLOAT_DEF_ID: Self = Self::new_reserved(PackageLocalDefId::FLOAT_LOCAL_DEF_ID);

    const BOOL_DEF_ID: Self = Self::new_reserved(PackageLocalDefId::BOOL_LOCAL_DEF_ID);

    #[inline]
    pub const fn new(pkg: PackageId, local: PackageLocalDefId) -> Self {
        Self { pkg, local }
    }

    #[inline]
    pub const fn new_in_self_pkg(local: PackageLocalDefId) -> Self {
        Self {
            pkg: PackageId::SELF_PACKAGE,
            local,
        }
    }

    #[inline]
    const fn new_reserved(local: PackageLocalDefId) -> Self {
        Self {
            pkg: PackageId::BUILTIN_RESERVED_PACKAGE,
            local,
        }
    }

    #[inline]
    pub fn pkg(&self) -> PackageId {
        self.pkg
    }

    #[inline]
    pub fn local_idx(&self) -> u32 {
        self.local.0
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
    pub const VOID_LOCAL_DEF_ID: Self = Self(0);

    pub const INT_LOCAL_DEF_ID: Self = Self(1);

    pub const UINT_LOCAL_DEF_ID: Self = Self(2);

    pub const FLOAT_LOCAL_DEF_ID: Self = Self(3);

    pub const BOOL_LOCAL_DEF_ID: Self = Self(4);

    pub const UNRESERVED_LOCAL_DEF_ID_MIN: u32 = 10;

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
    /// struct / enum / type alias / native type alias。
    ///
    /// enum に専用の腕は作らない。名前解決の段で enum を他の型と
    /// 区別する必要が無く、種別が要る場面はすべて HIR が出来た後なので
    /// `TyDefId` から `TyDefKind` を引けば分かるためである。
    Ty(TyDefId),
    /// enum のバリアント。
    Variant(VariantDefId),
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

            /// 型付けを外した生の [`DefId`]。
            /// lang item テーブルのように種別を問わず DefId を扱う箇所で使う。
            #[inline]
            pub fn def_id(&self) -> DefId {
                self.0
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

        // 走査順を固定したい箇所 (MIR のシンボル表など) で
        // BTreeMap のキーにできるようにする。
        // 順序そのものに意味は無く、ビルドの決定論のためにある。
        impl Ord for $typed_def_id {
            fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                self.0.as_u64().cmp(&other.0.as_u64())
            }
        }

        impl PartialOrd for $typed_def_id {
            fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                Some(self.cmp(other))
            }
        }
    };
}

impl_typed_def_id!(TyDefId);

impl TyDefId {
    pub const VOID_TY_DEF_ID: Self = Self(DefId::VOID_DEF_ID);

    pub const INT_TY_DEF_ID: Self = Self(DefId::INT_DEF_ID);

    pub const UINT_TY_DEF_ID: Self = Self(DefId::UINT_DEF_ID);

    pub const FLOAT_TY_DEF_ID: Self = Self(DefId::FLOAT_DEF_ID);

    pub const BOOL_TY_DEF_ID: Self = Self(DefId::BOOL_DEF_ID);
}
impl_typed_def_id!(ValDefId);

// enum のバリアントに割り当てられる id。
//
// バリアントに独立した DefId を振るのは、
// `import package::color::Color::Red;` のように
// バリアント単体を import できるようにするためである。
//
// 親の enum が何番目のバリアントかは DefId からは分からない。
// `Hir::variant_owners` (外部パッケージなら `.biwameta`) から引く。
impl_typed_def_id!(VariantDefId);

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ImplId(u32);

impl ImplId {
    pub fn new(id: u32) -> Self {
        Self(id)
    }
}
