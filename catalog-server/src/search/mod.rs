mod factory;
mod fts5_search;
mod pezzott_hash;
mod search_vault;

pub use factory::create_search_vault;
pub use fts5_search::Fts5SearchVault;
pub use search_vault::*;
