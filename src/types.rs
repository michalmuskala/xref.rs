use fxhash::FxHashMap;
use string_interner::{DefaultBackend, StringInterner, symbol::SymbolU32};

pub type Imports = FxHashMap<Atom, Vec<(Atom, u32)>>;
pub type Exports = Vec<(Atom, u32)>;
pub type Modules = FxHashMap<Atom, (Imports, Exports)>;
pub type Apps = FxHashMap<Atom, Vec<Atom>>;

pub type Interner = StringInterner<SymbolU32, DefaultBackend<SymbolU32>, fxhash::FxBuildHasher>;

#[derive(Debug, Eq, PartialEq, Hash, Copy, Clone)]
pub struct Atom(pub SymbolU32);

impl Atom {
    pub fn resolve<'a>(&self, interner: &'a Interner) -> Option<&'a str> {
        interner.resolve(self.0)
    }
}
