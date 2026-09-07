use std::collections::HashMap;

/// InternedIdent is interned identifier (<identifier>)
/// to compare identifiers faster than when use just [`String`].
/// Actual implementation is a wrapper of 32bit unsigned integer
/// stored in [`IdentInterner`] which ensure uniqueness of InternedIdent.
/// InternedIdent is unique in global scope (inter-package) and inter-session,
/// so used in incremental compilation cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InternedIdent(u32);

/// IdentInterner is interning pool of identifier (<identifier>).
#[derive(Debug, Clone, Default)]
pub struct IdentInterner {
    next_ident_id: u32,
    idents: HashMap<String, InternedIdent>,
}

impl InternedIdent {
    /// Sentinel for the `self` receiver parameter.
    /// The actual string "self" cannot be retrieved without an IdentInterner,
    /// so this reserved value is used wherever the name is only needed as an identity.
    pub const SELF: InternedIdent = InternedIdent(u32::MAX);
}

impl IdentInterner {
    pub fn new() -> Self {
        Self {
            next_ident_id: 0,
            idents: HashMap::new(),
        }
    }

    pub fn get_or_insert(&mut self, ident: &str) -> InternedIdent {
        match self.idents.get(ident) {
            Some(interned) => *interned,
            None => {
                let interned = InternedIdent(self.next_ident_id);
                self.idents.insert(ident.to_string(), interned);
                self.next_ident_id += 1;
                interned
            }
        }
    }

    /// 既に intern されている識別子だけを引く。
    ///
    /// 新しく登録しないので `&self` で呼べる。
    /// 見つからないということは、そのソースがその名前を一度も書いていない
    /// ということなので、名前の照合には「一致しない」と同じ意味になる。
    #[inline]
    pub fn get(&self, ident: &str) -> Option<InternedIdent> {
        self.idents.get(ident).copied()
    }

    #[inline]
    pub fn get_str(&self, interned: &InternedIdent) -> Option<&str> {
        self.idents
            .iter()
            .find(|(_, i)| &interned == i)
            .map(|(ident, _)| ident.as_str())
    }
}
