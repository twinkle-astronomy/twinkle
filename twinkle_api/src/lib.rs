use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub mod analysis;
pub mod fits;
pub mod indi;

#[derive(Serialize, Deserialize, Default)]
pub struct Count {
    pub  count: usize,
}
#[derive(Serialize, Deserialize)]
pub struct CreateCountRequestParams {
    pub id: Uuid,
}

#[derive(Serialize, Deserialize)]
pub struct CreateCountResponse {
    pub id: Uuid,
}