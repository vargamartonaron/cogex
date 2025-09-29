use lazy_static::lazy_static;
use std::sync::RwLock;
pub use string_cache::DefaultAtom as Atom;

lazy_static! {
    static ref TEXT_INTERNER: RwLock<Vec<Atom>> = RwLock::new(Vec::new());
}

/// Intern a string and return its ID
pub fn intern_text(s: &str) -> usize {
    let atom = Atom::from(s);
    let mut v = TEXT_INTERNER.write().unwrap();
    match v.iter().position(|a| *a == atom) {
        Some(idx) => idx,
        None => {
            v.push(atom);
            v.len() - 1
        }
    }
}

/// Current count of unique texts
pub fn text_count() -> usize {
    TEXT_INTERNER.read().unwrap().len()
}

pub fn get_text(id: usize) -> String {
    TEXT_INTERNER.read().unwrap()[id].to_string()
}
