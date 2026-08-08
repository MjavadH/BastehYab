use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Operator {
    Mci,
    Irancell,
    Rightel,
    Samantel,
}

impl Operator {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Mci => "mci",
            Self::Irancell => "irancell",
            Self::Rightel => "rightel",
            Self::Samantel => "samantel",
        }
    }
}
