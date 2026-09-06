use biwac_base::InternedIdent;
use biwac_hir::Ty;

use crate::Local;

/// 値の置き場所。
///
/// 「どこにあるか」(レジスタか、線形メモリか、GC されるオブジェクトか) は言わない。
/// それはバックエンドが決めることである。
#[derive(Debug, Clone)]
pub struct Place {
    pub local: Local,

    /// local からの射影。空なら local そのもの。
    pub projection: Vec<PlaceElem>,
}

impl Place {
    /// 射影の無い、local そのものを指す場所。
    pub fn from_local(local: Local) -> Self {
        Self {
            local,
            projection: Vec::new(),
        }
    }

    /// この場所にさらにフィールド射影を重ねた場所。
    pub fn field(&self, name: InternedIdent, ty: Ty) -> Self {
        let mut projection = self.projection.clone();
        projection.push(PlaceElem::Field(name, ty));
        Self {
            local: self.local,
            projection,
        }
    }

    /// 射影が無いなら true。
    pub fn is_local(&self) -> bool {
        self.projection.is_empty()
    }
}

#[derive(Debug, Clone)]
pub enum PlaceElem {
    /// struct のメンバ。
    ///
    /// biwa の struct のメンバは名前で引く
    /// (HIR も `HashMap<InternedIdent, Ty>` で持っている) ので、
    /// ここでも名前で持つ。名前からオフセットやフィールド番号への変換は
    /// レイアウトを決めるバックエンドの仕事である。
    ///
    /// 型を添えてあるのは、射影した先の型を引くのに定義表を辿らずに済ませるため。
    Field(InternedIdent, Ty),

    /// enum の値を、特定のバリアントとして見る。
    ///
    /// 必ず [`PlaceElem::Field`] の直前に来る
    /// (`[Downcast(1), Field("_0", T)]`)。
    /// 値のタグが本当にそのバリアントであることは、
    /// `match` の lowering が保証する。
    Downcast(u32),
}
