use std::collections::HashMap;

use biwac_base::InternedIdent;
use biwac_span::{GenDefId, LocalGenDefId, Span, VarId};

use crate::{DecledVar, Expr, ExprId, Ident, Stmt, Ty};

// 値名前空間のシンボル
#[derive(Debug, Clone)]
pub enum ValDefKind {
    Fn(Box<FnDef>),
    Native(Box<NativeFnDef>),
    NovelScene(Box<NovelSceneDef>),
    // ExternalFn(Box<FnSignature>),
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
    pub name: Ident,

    // signature
    pub signature: FnSignature,

    // body
    // needs type inferrence
    pub body: FnBody,

    // 型推論された結果の式に対する型が記録される
    pub expr_tys: HashMap<ExprId, Ty>,
    pub var_tys: HashMap<VarId, Ty>,

    // 呼び出し式ごとの、呼び先のジェネリック型への割り当て。
    //
    // 単相化 (monomorphization) を行うターゲットでは、
    // 呼び出し位置でどの型が代入されたのかを知る必要がある。
    // 推論の途中でしか計算されない情報なので、ここに残しておく。
    //
    // 位置ではなく LocalGenDefId との組で持つのは、
    // 具体化が Ty::embody_by_loc_gen_ty_id で行われるためである。
    // LocalGenDefId 順に並ぶ。
    pub call_genargs: HashMap<ExprId, Vec<(LocalGenDefId, Ty)>>,

    pub impl_genargs: Vec<(Ident, LocalGenDefId)>,
}

#[derive(Debug, Clone)]
pub struct FnArgDecl {
    pub id: Ident,
    pub ty: Ty,
    pub var_id: VarId,
}

// ```
//  fn foo[T](x: T, y: Int) -> Bar[T] { ... }
//        ^^^^^^^^^^^^^^^^^^^^^^^^^^^
//        signature
// ```
#[derive(Debug, Clone)]
pub struct FnSignature {
    // explicit arguments (does NOT include `self`)
    pub args: Vec<FnArgDecl>,

    // Some if this is a method (first arg is self receiver)
    pub self_ty: Option<Ty>,

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

    // VarId for the self receiver (Some for methods, None otherwise)
    pub self_var_id: Option<VarId>,

    // 関数内で宣言された変数のマップ (includes args and self)
    pub vars: HashMap<VarId, DecledVar>,
}

#[derive(Debug, Clone)]
pub struct NativeFnDef {
    pub name: Ident,

    // signature
    pub signature: FnSignature,

    pub native_body: String,

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
    Struct(Box<StructDef>),
    // Enum(EnumDefContent),
    NativeTypeAlias(Box<NativeTypeAliasDef>),
}

#[derive(Debug, Clone)]
pub struct StructDef {
    pub name: Ident,

    pub members: HashMap<InternedIdent, Ty>,
    pub genargs: Vec<GenDefId>,
    // TODO: その他各種情報
}

#[derive(Debug, Clone)]
pub struct TypeAliasDef {
    pub name: Ident,

    pub genargs: Vec<GenDefId>,
    pub right: Ty,
}

#[derive(Debug, Clone)]
pub struct NativeTypeAliasDef {
    pub name: Ident,
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
    pub name: Ident,

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

    // FnDef と同じ。
    pub call_genargs: HashMap<ExprId, Vec<(LocalGenDefId, Ty)>>,
}

impl FnDef {
    pub fn new(
        name: Ident,
        signature: FnSignature,
        body: FnBody,
        impl_genargs: Vec<(Ident, LocalGenDefId)>,
    ) -> Self {
        Self {
            name,
            signature,
            body,
            expr_tys: HashMap::new(),
            var_tys: HashMap::new(),
            call_genargs: HashMap::new(),
            impl_genargs,
        }
    }
}

impl NativeFnDef {
    pub fn new(
        name: Ident,
        native_span: Span,
        span: Span,
        native_body: String,
        signature: FnSignature,
        impl_genargs: Vec<(Ident, LocalGenDefId)>,
    ) -> Self {
        Self {
            name,
            signature,
            native_body,
            native_span,
            span,
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
    pub fn new(name: Ident, signature: FnSignature, body: FnBody) -> Self {
        Self {
            name,
            signature,
            body,
            expr_tys: HashMap::new(),
            var_tys: HashMap::new(),
            call_genargs: HashMap::new(),
        }
    }
}
