mod factory;
mod fts5_levenshtein_search;
mod fts5_search;
mod levenshtein;
mod pezzott_hash;
mod search_vault;

pub use factory::create_search_vault;
pub use fts5_levenshtein_search::Fts5LevenshteinSearchVault;
pub use fts5_search::Fts5SearchVault;
pub use search_vault::*;
