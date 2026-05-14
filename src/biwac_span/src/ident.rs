use std::collections::HashMap;

/// InternedIdent is interned identifier (<identifier>)
/// to compare identifiers faster than when use just [`String`].
/// Actual implementation is a wrapper of 32bit unsigned integer
/// stored in [`IdentInterner`] which ensure uniqueness of InternedIdent.
/// InternedIdent is unique in global scope (inter-package) and inter-session,
/// so used in incremental compilation cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InternedIdent(u32);

/// IdentInterner is interning pool of identifier (<identifier>).
#[derive(Debug, Clone, Default)]
pub struct IdentInterner {
    next_ident_id: u32,
    idents: HashMap<String, InternedIdent>,
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
                interned
            }
        }
    }
}
