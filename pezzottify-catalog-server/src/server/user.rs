use std::time::Instant;

pub struct User {
    pub name: String,
    pub id: String,
    pub created: Instant,
}