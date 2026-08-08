use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Currency {
    Irr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Money {
    pub amount: u64,
    pub currency: Currency,
}

impl Money {
    pub const fn irr(amount: u64) -> Self {
        Self {
            amount,
            currency: Currency::Irr,
        }
    }
}
