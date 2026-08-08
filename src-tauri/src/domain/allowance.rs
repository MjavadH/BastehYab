use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataAllowance {
    pub amount_bytes: Option<u64>,
    pub unlimited: bool,
    pub kind: DataAllowanceKind,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataAllowanceKind {
    General,
    Night,
    Domestic,
    International,
    Social,
    ApplicationSpecific,
    Gift,
    Other,
}
