use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub mod analysis;
pub mod fits;
pub mod indi;
pub mod flats;

#[derive(Serialize, Deserialize, Default)]
pub struct Count {
    pub  count: usize,
}
#[derive(Serialize, Deserialize)]
pub struct StreamCountRequestParams {
    pub id: Uuid,
}

#[derive(Serialize, Deserialize)]
pub struct CreateCountResponse {
    pub id: Uuid,
}

#[derive(Serialize, Deserialize)]
#[derive(Debug, Clone, Eq)]
pub struct Filter {
    pub name: String,
    pub position: usize,
}
impl PartialEq for Filter {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.position == other.position
    }
}
impl PartialOrd for Filter {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.position.partial_cmp(&other.position)
    }
}

impl Ord for Filter {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.position.cmp(&other.position)
    }
}

impl std::fmt::Display for Filter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.name.fmt(f)
    }
}

impl From<&Filter> for f64 {
    fn from(value: &Filter) -> Self {
        value.position as f64
    }
}