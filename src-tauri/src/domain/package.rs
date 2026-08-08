use serde::{Deserialize, Serialize};

use super::{
    allowance::{DataAllowance, SmsAllowance, VoiceAllowance},
    money::Money,
    operator::Operator,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PackageId(pub String);

impl PackageId {
    pub fn canonical(operator: Operator, external_id: &str) -> Self {
        Self(format!("{}:{}", operator.as_str(), external_id))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InternetPackage {
    pub id: PackageId,
    pub operator: Operator,
    pub external_id: String,
    pub name: String,
    pub price: Option<Money>,
    pub validity: Validity,
    pub data_allowances: Vec<DataAllowance>,
    pub voice: Option<VoiceAllowance>,
    pub sms: Option<SmsAllowance>,
    pub sim_types: Vec<SimType>,
    pub package_kind: PackageKind,
    pub availability: Availability,
    pub purchase: PurchaseInfo,
    pub metadata: PackageMetadata,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Validity {
    Hours(u32),
    Days(u32),
    Other,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SimType {
    Prepaid,
    Postpaid,
    Tdlte,
    DataSim,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackageKind {
    InternetOnly,
    Combined,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Availability {
    Available,
    Unavailable,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PurchaseInfo {
    pub official_url: Option<String>,
    pub ussd_code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageMetadata {
    pub fetched_at_unix_seconds: Option<i64>,
    pub source_url: Option<String>,
    pub regulatory_code: Option<String>,
    pub offer_code: Option<String>,
    pub original_description: Option<String>,
}
