use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataAllowance {
    pub amount_bytes: Option<u64>,
    pub unlimited: bool,
    pub kind: DataAllowanceKind,
    pub time_window: Option<TimeWindow>,
    pub description: Option<String>,
}

impl DataAllowance {
    pub fn finite(kind: DataAllowanceKind, amount_bytes: u64) -> Self {
        Self {
            amount_bytes: Some(amount_bytes),
            unlimited: false,
            kind,
            time_window: None,
            description: None,
        }
    }

    pub fn unlimited(kind: DataAllowanceKind) -> Self {
        Self {
            amount_bytes: None,
            unlimited: true,
            kind,
            time_window: None,
            description: None,
        }
    }

    pub fn unknown(kind: DataAllowanceKind) -> Self {
        Self {
            amount_bytes: None,
            unlimited: false,
            kind,
            time_window: None,
            description: None,
        }
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeWindow {
    pub start: LocalTime,
    pub end: LocalTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalTime {
    pub hour: u8,
    pub minute: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceAllowance {
    pub minutes: Option<u32>,
    pub unlimited: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SmsAllowance {
    pub count: Option<u32>,
    pub unlimited: bool,
}
