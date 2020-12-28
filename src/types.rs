use fxhash::FxHashMap;
use petgraph::graphmap::DiGraphMap;
use string_interner::{symbol::SymbolU32, DefaultBackend, StringInterner};

pub type Imports = FxHashMap<Atom, Vec<(Atom, u32)>>;
pub type Exports = Vec<(Atom, u32)>;
pub type Modules = FxHashMap<Atom, (Imports, Exports)>;
pub type AppModules = FxHashMap<Atom, Vec<Atom>>;
pub type AppDeps = DiGraphMap<Atom, ()>;

pub type Interner = StringInterner<SymbolU32, DefaultBackend<SymbolU32>, fxhash::FxBuildHasher>;

#[derive(Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Copy, Clone)]
pub struct Atom(pub SymbolU32);

impl Atom {
    pub fn intern(interner: &mut Interner, value: &str) -> Atom {
        Atom(interner.get_or_intern(value))
    }

    pub fn resolve<'a>(&self, interner: &'a Interner) -> Option<&'a str> {
        interner.resolve(self.0)
    }
}
