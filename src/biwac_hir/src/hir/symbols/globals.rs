use std::collections::HashMap;

use biwac_span::{GenDefId, LocalGenDefId, Span, VarId};

use crate::{DecledVar, Expr, ExprId, Ident, Stmt, Ty};

// 値名前空間のシンボル
#[derive(Debug, Clone)]
pub enum ValDefKind {
    Fn(Box<FnDef>),
    Native(Box<NativeFnDef>),
    NovelScene(Box<NovelSceneDef>),
    ExternalFn(Box<FnSignature>),
}

// impl block 内での
// 値名前空間のシンボル
#[derive(Debug, Clone)]
pub enum AssocValDefKind {
    Fn(Box<FnDef>),
    NativeFn(Box<NativeFnDef>),
}

/// function and method
#[derive(Debug, Clone)]
pub struct FnDef {
    pub fn_name_span: Span,

    // signature
    pub signature: FnSignature,

    // body
    // needs type inferrence
    pub body: FnBody,

    // 型推論された結果の式に対する型が記録される
    pub expr_tys: HashMap<ExprId, Ty>,
    pub var_tys: HashMap<VarId, Ty>,

    pub impl_genargs: Vec<(Ident, LocalGenDefId)>,
}

// ```
//  fn foo[T](x: T, y: Int) -> Bar[T] { ... }
//        ^^^^^^^^^^^^^^^^^^^^^^^^^^^
//        signature
// ```
#[derive(Debug, Clone)]
pub struct FnSignature {
    // NOTE:
    // contains `self`
    pub args: Vec<(Ident, Ty)>,

    // if the function does not return value ( = void function),
    // Ty::Void
    pub rty: Ty,

    pub genargs: Vec<(Ident, LocalGenDefId)>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct FnBody {
    pub stmts: Vec<Stmt>,
    pub expr: Option<Expr>,
    pub arg_var_ids: Vec<VarId>,

    // 関数内で宣言された変数のマップ
    // 一意なid: VarIdを割り当てる
    pub vars: HashMap<VarId, DecledVar>,
}

#[derive(Debug, Clone)]
pub struct NativeFnDef {
    // signature
    pub signature: FnSignature,

    pub native_body: String,

    pub fn_name_span: Span,
    pub native_span: Span,
    pub span: Span,

    pub impl_genargs: Vec<(Ident, LocalGenDefId)>,
}

#[derive(Debug, Clone)]
pub struct NativeFnArgDecl {
    pub ty: Ty,
    pub id: Ident,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct DecledArg {
    pub id: VarId,
}

// 各種の型の定義
// e.g.) struct, enum
#[derive(Debug, Clone)]
pub enum TyDefKind {
    Struct(Box<StructDefContent>),
    // Enum(EnumDefContent),
    TypeAlias(Box<TypeAliasDefContent>),
    NativeTypeAlias(Box<NativeTypeAliasDefContent>),
}

#[derive(Debug, Clone)]
pub struct StructDefContent {
    pub members: HashMap<String, Ty>,
    pub genargs: Vec<GenDefId>,
    // TODO: その他各種情報
    pub struct_name_span: Span,
}

#[derive(Debug, Clone)]
pub struct TypeAliasDefContent {
    pub genargs: Vec<GenDefId>,
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
pub struct NovelSceneDef {
    pub scene_name_span: Span,

    // signature
    // ただし、
    // (std::game::Game[_]) -> std::game::Game[_]
    // である必要がある
    // これは登録時に検査される
    pub signature: FnSignature,

    // body
    // needs type inferrence
    pub body: FnBody,

    // 型推論された結果の式に対する型が記録される
    pub expr_tys: HashMap<ExprId, Ty>,
    pub var_tys: HashMap<VarId, Ty>,
}

impl FnDef {
    pub fn new(
        fn_ident: &biwac_ast::Ident,
        signature: FnSignature,
        body: FnBody,
        impl_genargs: Vec<(Ident, LocalGenDefId)>,
    ) -> Self {
        Self {
            fn_name_span: fn_ident.span.clone(),
            signature,
            body,
            expr_tys: HashMap::new(),
            var_tys: HashMap::new(),
            impl_genargs,
        }
    }
}

impl NativeFnDef {
    pub fn new(
        signature: FnSignature,
        fn_def: biwac_ast::NativeFnDef,
        impl_genargs: Vec<(Ident, LocalGenDefId)>,
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

impl From<&biwac_ast::NativeCode> for NativeCode {
    fn from(value: &biwac_ast::NativeCode) -> Self {
        Self {
            native: value.native.clone(),
            native_span: value.native_span.clone(),
        }
    }
}

impl NovelSceneDef {
    pub fn new(scene_ident: &biwac_ast::Ident, signature: FnSignature, body: FnBody) -> Self {
        Self {
            scene_name_span: scene_ident.span.clone(),
            signature,
            body,
            expr_tys: HashMap::new(),
            var_tys: HashMap::new(),
        }
    }
}
