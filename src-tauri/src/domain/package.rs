use serde::{Deserialize, Serialize};

use super::{allowance::DataAllowance, money::Money, operator::Operator};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InternetPackage {
    pub id: PackageId,
    pub operator: Operator,
    pub external_id: String,
    pub name: String,
    pub price: Money,
    pub data_allowances: Vec<DataAllowance>,
}
