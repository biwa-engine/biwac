use std::collections::HashMap;

use biwac_ast::{
    FnDef, Ident, MethodDef, NativeFnDef, NovelScene, symbols::globals::NativeMethodDef,
};
use biwac_base::{ModPath, Span};

use crate::{DecledVar, Expr, ExprId, LocVarId, Progressive, Stmt, Ty};

// 型名前空間のシンボルを
// 識別するid
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TyId {
    // pub pkg: enum Package { Internal, External(String)}
    quals: Vec<String>,
    id: String,
}

// 型定義側で
// 宣言されるジェネリクス型に割り当てられるid
// GenTyIdに対するTyの割り当て(HashMap<GenTyId, Ty>)を保持することで、
// あるジェネリック型の使用箇所におけるのメンバなどへの型付けを計算できる
//  ```
//  struct Foo[T, U] {
//            ^^^^^^
//      x: T,
//      y: U,
//      z: Int,
//  }
//  ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GenTyId(usize);

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LocGenTyId(usize);

// 値名前空間のシンボル
// - 関数
// - グローバル変数(const)
// を識別するid
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ValId {
    // pub pkg: enum Package { Internal, External(String)}
    quals: Vec<String>,
    id: String,
}

// 値名前空間のシンボル
#[derive(Debug, Clone)]
pub enum ValDefContentKind {
    Fn(Box<FnDefContent>),
    Native(Box<NativeFnDefContent>),
    NovelScene(Box<NovelSceneDefContent>),
}

// impl block 内での
// 値名前空間のシンボル
#[derive(Debug, Clone)]
pub enum ImplValDefContentKind {
    Fn(Box<FnDefContent>),
    NativeFn(Box<NativeFnDefContent>),
    Method(Box<MethodDefContent>),
    NativeMethod(Box<NativeMethodDefContent>),
}

#[derive(Debug, Clone)]
pub struct FnDefContent {
    pub fn_name_span: Span,

    // signature
    pub signature: FnDefContentSignature,

    // body
    // needs type inferrence
    pub body: Progressive<FnDef, FnDefContentBody>,

    // 型推論された結果の式に対する型が記録される
    pub expr_tys: HashMap<ExprId, Ty>,
    pub var_tys: HashMap<LocVarId, Ty>,

    pub impl_genargs: Vec<(Ident, LocGenTyId)>,
}

// ```
//  fn foo[T](x: T, y: Int) -> Bar[T] { ... }
//        ^^^^^^^^^^^^^^^^^^^^^^^^^^^
//        signature
// ```
#[derive(Debug, Clone)]
pub struct FnDefContentSignature {
    pub args: Vec<(Ident, Ty)>,

    // if the function does not return value ( = void function),
    // Ty::Void
    pub rty: Ty,

    pub genargs: Vec<(Ident, LocGenTyId)>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct FnDefContentBody {
    pub stmts: Vec<Stmt>,
    pub expr: Option<Expr>,
    pub arg_var_ids: Vec<LocVarId>,

    // 関数内で宣言された変数のマップ
    // 一意なid: LocVarIdを割り当てる
    pub vars: HashMap<LocVarId, DecledVar>,
}

#[derive(Debug, Clone)]
pub struct NativeFnDefContent {
    // signature
    pub signature: FnDefContentSignature,

    pub native_body: String,

    pub fn_name_span: Span,
    pub native_span: Span,
    pub span: Span,

    pub impl_genargs: Vec<(Ident, LocGenTyId)>,
}

#[derive(Debug, Clone)]
pub struct NativeFnArgDecl {
    pub ty: Ty,
    pub id: Ident,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct DecledArg {
    pub id: LocVarId,
}

// TODO: FnDefContent と同じで済むなら同じに
#[derive(Debug, Clone)]
pub struct MethodDefContent {
    pub fn_name_span: Span,

    // NOTE: selfは0が割り当てられることを決めてしまう?
    // pub self_id: LocVarId,

    // signature
    pub signature: FnDefContentSignature,

    // body
    // needs type inferrence
    pub body: Progressive<MethodDef, FnDefContentBody>,

    // 関数内で宣言された変数のマップ
    // 一意なid: LocVarIdを割り当てる
    pub vars: HashMap<LocVarId, DecledVar>,

    // 型推論された結果の式に対する型が記録される
    pub expr_tys: HashMap<ExprId, Ty>,
    pub var_tys: HashMap<LocVarId, Ty>,

    pub impl_genargs: Vec<(Ident, LocGenTyId)>,
}

// TODO: NativeFnDefContent と同じで済むなら同じに
#[derive(Debug, Clone)]
pub struct NativeMethodDefContent {
    // signature
    pub signature: FnDefContentSignature,

    pub native_body: String,

    pub fn_name_span: Span,
    pub native_span: Span,
    pub span: Span,

    pub self_ty: Ty,
    pub impl_genargs: Vec<(Ident, LocGenTyId)>,
}

// 各種の型の定義
// e.g.) struct, enum
#[derive(Debug, Clone)]
pub enum TyDefContentKind {
    Struct(Box<StructDefContent>),
    // Enum(EnumDefContent),
    TypeAlias(Box<TypeAliasDefContent>),
    NativeTypeAlias(Box<NativeTypeAliasDefContent>),
}

#[derive(Debug, Clone)]
pub struct StructDefContent {
    pub members: HashMap<String, (Ty, Span)>,
    pub genargs: Vec<GenTyId>,
    // TODO: その他各種情報
    pub struct_name_span: Span,
}

#[derive(Debug, Clone)]
pub struct TypeAliasDefContent {
    pub genargs: Vec<GenTyId>,
    pub right: Ty,
    pub alias_name_span: Span,
}

#[derive(Debug, Clone)]
pub struct NativeTypeAliasDefContent {
    pub alias_name_span: Span,
    pub genargs: Vec<Ident>,
    pub native: String,
    pub native_span: Span,
}

#[derive(Debug, Clone)]
pub struct NativeCode {
    pub native: String,
    pub native_span: Span,
}

#[derive(Debug, Clone)]
pub struct NovelSceneDefContent {
    pub scene_name_span: Span,

    // signature
    // ただし、
    // (std::game::Game[_]) -> std::game::Game[_]
    // である必要がある
    // これは登録時に検査される
    pub signature: FnDefContentSignature,

    // body
    // needs type inferrence
    pub body: Progressive<NovelScene, FnDefContentBody>,

    // 型推論された結果の式に対する型が記録される
    pub expr_tys: HashMap<ExprId, Ty>,
    pub var_tys: HashMap<LocVarId, Ty>,
}

impl TyId {
    pub fn new(quals: Vec<String>, id: String) -> Self {
        Self { quals, id }
    }

    pub fn from_modpath(modpath: &ModPath, id: String) -> Self {
        Self {
            quals: match modpath {
                ModPath::Main => vec![],
                ModPath::Lib => vec![],
                ModPath::Mod(m) => m.clone(),
            },
            id,
        }
    }

    pub fn quals(&self) -> &[String] {
        &self.quals
    }

    pub fn id(&self) -> &str {
        &self.id
    }
}

impl ValId {
    pub fn new(quals: Vec<String>, id: String) -> Self {
        Self { quals, id }
    }

    pub fn from_modpath(modpath: &ModPath, id: String) -> Self {
        Self {
            quals: match modpath {
                ModPath::Main => vec![],
                ModPath::Lib => vec![],
                ModPath::Mod(m) => m.clone(),
            },
            id,
        }
    }

    pub fn quals(&self) -> &[String] {
        &self.quals
    }

    pub fn id(&self) -> &str {
        &self.id
    }
}

impl GenTyId {
    pub fn new(id: usize) -> Self {
        Self(id)
    }

    pub fn value(&self) -> usize {
        self.0
    }
}

impl LocGenTyId {
    pub fn new(id: usize) -> Self {
        Self(id)
    }

    pub fn value(&self) -> usize {
        self.0
    }
}

impl FnDefContent {
    pub fn new(
        signature: FnDefContentSignature,
        fn_def: FnDef,
        impl_genargs: Vec<(Ident, LocGenTyId)>,
    ) -> Self {
        Self {
            fn_name_span: fn_def.id.span.clone(),
            signature,
            body: Progressive::NotYet(fn_def),
            expr_tys: HashMap::new(),
            var_tys: HashMap::new(),
            impl_genargs,
        }
    }
}

impl MethodDefContent {
    pub fn new(
        signature: FnDefContentSignature,
        method_def: MethodDef,
        impl_genargs: Vec<(Ident, LocGenTyId)>,
    ) -> Self {
        Self {
            fn_name_span: method_def.id.span.clone(),
            signature,
            body: Progressive::NotYet(method_def),
            vars: HashMap::new(),
            expr_tys: HashMap::new(),
            var_tys: HashMap::new(),
            impl_genargs,
        }
    }
}

impl NativeFnDefContent {
    pub fn new(
        signature: FnDefContentSignature,
        fn_def: NativeFnDef,
        impl_genargs: Vec<(Ident, LocGenTyId)>,
    ) -> Self {
        Self {
            signature,
            native_body: fn_def.native,
            fn_name_span: fn_def.id.span,
            native_span: fn_def.native_span,
            span: fn_def.span,
            impl_genargs,
        }
    }
}

impl NativeMethodDefContent {
    pub fn new(
        signature: FnDefContentSignature,
        method_def: NativeMethodDef,
        self_ty: Ty,
        impl_genargs: Vec<(Ident, LocGenTyId)>,
    ) -> Self {
        Self {
            signature,
            native_body: method_def.native,
            fn_name_span: method_def.id.span,
            native_span: method_def.native_span,
            span: method_def.span,
            self_ty,
            impl_genargs,
        }
    }
}

impl From<&biwac_ast::NativeCode> for NativeCode {
    fn from(value: &biwac_ast::NativeCode) -> Self {
        Self {
            native: value.native.clone(),
            native_span: value.native_span.clone(),
        }
    }
}

impl NovelSceneDefContent {
    pub fn new(signature: FnDefContentSignature, scene_def: NovelScene) -> Self {
        Self {
            scene_name_span: scene_def.id.span.clone(),
            signature,
            body: Progressive::NotYet(scene_def),
            expr_tys: HashMap::new(),
            var_tys: HashMap::new(),
        }
    }
}
