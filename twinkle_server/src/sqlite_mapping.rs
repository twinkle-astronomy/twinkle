// https://github.com/diesel-rs/diesel/issues/852
pub use diesel::sql_types::*;
// This will change the mapping for all Integer columns in your database to i64, not only the PRIMARY KEY AUTOINCREMENT ones.
// Depending on your use case this can be fine or not...
pub type Integer = BigInt;
