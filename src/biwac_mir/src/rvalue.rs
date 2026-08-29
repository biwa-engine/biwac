use biwac_span::{TyDefId, ValDefId};

use biwac_base::InternedIdent;

use crate::{GenArgs, Place};

/// 1 つの値を作る計算。
#[derive(Debug, Clone)]
pub enum Rvalue {
    /// そのまま使う。
    Use(Operand),

    BinaryOp(BinOp, Operand, Operand),

    UnaryOp(UnOp, Operand),

    /// struct literal。
    ///
    /// メンバは名前と値の組で、宣言順ではなくメンバ名の順に正規化して持つ。
    Aggregate(TyDefId, Vec<(InternedIdent, Operand)>),
}

/// 計算の入力。
///
/// rustc の `Copy` / `Move` の区別は持たない。
/// biwa には所有権が無く、区別が意味を持たないためである。
#[derive(Debug, Clone)]
pub enum Operand {
    Place(Place),
    Const(Const),
}

impl Operand {
    pub fn from_local(local: crate::Local) -> Self {
        Self::Place(Place::from_local(local))
    }
}

#[derive(Debug, Clone)]
pub enum Const {
    Int(i64),
    Float(f64),
    Bool(bool),

    /// 値を返さないこと。`Void` 型の場所への代入に使う。
    Void,

    /// 文字列リテラル。実体は [`StringPool`] にある。
    Str(StrId),

    /// 関数そのものを値として扱うとき。
    ///
    /// `genargs` は [`crate::Callee::Direct`] と同じ意味である。
    FnDef(ValDefId, GenArgs),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    Neg,
}

/// 文字列リテラルの番号。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StrId(u32);

impl StrId {
    #[inline]
    pub fn index(&self) -> usize {
        self.0 as usize
    }

    #[inline]
    pub fn value(&self) -> u32 {
        self.0
    }
}

/// パッケージ内の文字列リテラルの集合。
///
/// 同じ内容のリテラルは 1 つにまとめる。
#[derive(Debug, Clone, Default)]
pub struct StringPool {
    strings: Vec<String>,
}

impl StringPool {
    pub fn intern(&mut self, s: &str) -> StrId {
        if let Some(idx) = self.strings.iter().position(|existing| existing == s) {
            return StrId(idx as u32);
        }
        let idx = self.strings.len() as u32;
        self.strings.push(s.to_string());
        StrId(idx)
    }

    /// 索引から [`StrId`] を引く。ディスク形式から読み戻すのに使う。
    pub fn id(&self, idx: u32) -> Option<StrId> {
        ((idx as usize) < self.strings.len()).then_some(StrId(idx))
    }

    pub fn get(&self, id: StrId) -> Option<&str> {
        self.strings.get(id.index()).map(|s| s.as_str())
    }

    pub fn iter(&self) -> impl Iterator<Item = (StrId, &str)> + use<'_> {
        self.strings
            .iter()
            .enumerate()
            .map(|(i, s)| (StrId(i as u32), s.as_str()))
    }

    pub fn len(&self) -> usize {
        self.strings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.strings.is_empty()
    }
}
