use std::{process::Command, time::Duration};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use thiserror::Error;

use crate::{
    cache::now_unix_seconds,
    domain::operator::Operator,
    normalizers::rightel::RightelNormalizer,
    refresh::orchestrator::{CollectedPackages, Collector, CollectorError},
};
use crate::domain::package::InternetPackage;

const RIGHTEL_AUTH_URL: &str =
    "https://portal-api.rightel.ir/user-management/api/v1/auth/authenticate";
const RIGHTEL_PACKAGES_URL: &str = "https://portal-api.rightel.ir/extra-package/api/v1/extra-package-direct/web-site/purchasable-package";
const RIGHTEL_PAGE_URL: &str = "https://package.rightel.ir/packagesList";
const MAX_RESPONSE_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct RightelCollector {
    auth_url: String,
    packages_url: String,
    timeout: Duration,
}

impl Default for RightelCollector {
    fn default() -> Self {
        Self {
            auth_url: RIGHTEL_AUTH_URL.into(),
            packages_url: RIGHTEL_PACKAGES_URL.into(),
            timeout: Duration::from_secs(20),
        }
    }
}

impl RightelCollector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn collect_raw(&self) -> Result<RightelCatalog, RightelCollectorError> {
        let token = self.authenticate()?;
        let bytes = self.get_packages(&token)?;
        // token is deliberately scoped to this operation and is not stored, cached, logged, or returned.
        parse_catalog(&bytes, &self.packages_url)
    }

    fn authenticate(&self) -> Result<String, RightelCollectorError> {
        let bytes = curl(
            &[
                "--request",
                "POST",
                "--header",
                "Accept: application/json",
                "--header",
                "Content-Type: application/json",
                "--data",
                r#"{"username":"website"}"#,
                &self.auth_url,
            ],
            self.timeout,
        )?;
        parse_auth_token(&bytes)
    }

    fn get_packages(&self, token: &str) -> Result<Vec<u8>, RightelCollectorError> {
        if token.trim().is_empty() {
            return Err(RightelCollectorError::InvalidToken);
        }
        let auth = format!("Authorization: Bearer {token}");
        curl(
            &[
                "--request",
                "GET",
                "--header",
                "Accept: application/json",
                "--header",
                &auth,
                &self.packages_url,
            ],
            self.timeout,
        )
    }
}

impl Collector for RightelCollector {
    fn collect(&self, operator: Operator) -> Result<CollectedPackages, CollectorError> {
        if operator != Operator::Rightel {
            return Err(CollectorError::Failed(format!(
                "RightelCollector cannot collect {:?}",
                operator
            )));
        }
        let fetched_at = now_unix_seconds();
        let catalog = self
            .collect_raw()
            .map_err(|e| CollectorError::Failed(e.to_string()))?;
        let raw_record_count = catalog.packages.len();
        println!("Rightel products collected: {raw_record_count}");
        let mut packages: Vec<InternetPackage> = Vec::new();
        let mut normalization_failures = 0;

        for raw in catalog.packages {
            match RightelNormalizer::normalize(&raw, fetched_at) {
                Ok(package) => {
                    if let Some(existing) = packages.iter_mut().find(|p| p.name == package.name && p.price == package.price) {
                        for sim in package.sim_types {
                            if !existing.sim_types.contains(&sim) {
                                existing.sim_types.push(sim);
                            }
                        }
                    } else {
                        packages.push(package);
                    }
                }
                Err(_) => normalization_failures += 1,
            }
        }

        Ok(CollectedPackages {
            fetched_at_unix_seconds: fetched_at,
            raw_record_count,
            packages,
            normalization_failures,
        })
    }
}

fn curl(args: &[&str], timeout: Duration) -> Result<Vec<u8>, RightelCollectorError> {
    let timeout = timeout.as_secs().max(1).to_string();
    let mut full = vec![
        "--fail",
        "--silent",
        "--show-error",
        "--location",
        "--max-time",
        &timeout,
        "--user-agent",
        "BastehYab/0.1 (+https://github.com/MjavadH/BastehYab)",
    ];
    full.extend_from_slice(args);
    let output = Command::new("curl")
        .args(full)
        .output()
        .map_err(|e| RightelCollectorError::Request(e.to_string()))?;
    if !output.status.success() {
        return Err(RightelCollectorError::Status(
            output.status.code().unwrap_or_default() as u16,
        ));
    }
    if output.stdout.len() as u64 > MAX_RESPONSE_BYTES {
        return Err(RightelCollectorError::TooLarge(output.stdout.len() as u64));
    }
    Ok(output.stdout)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RightelCatalog {
    pub source_url: String,
    pub packages: Vec<RawRightelPackage>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawRightelPackage {
    pub purchasable_package_id: Option<String>,
    pub name_fa: Option<String>,
    pub name_en: Option<String>,
    pub price: Option<Value>,
    pub product_type: Option<String>,
    pub offer_code: Option<String>,
    pub filters: Vec<Value>,
    pub channel_categories: Vec<Value>,
    pub unknown_fields: Map<String, Value>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RightelCollectorError {
    #[error("Rightel request failed: {0}")]
    Request(String),
    #[error("Rightel source returned HTTP {0}")]
    Status(u16),
    #[error("Rightel response is too large: {0} bytes")]
    TooLarge(u64),
    #[error("Rightel response is malformed JSON: {0}")]
    Json(String),
    #[error("Rightel authentication failed: token missing")]
    Authentication,
    #[error("Rightel token is invalid")]
    InvalidToken,
    #[error("Rightel response contained no package-like records")]
    EmptyResponse,
}

pub fn parse_auth_token(bytes: &[u8]) -> Result<String, RightelCollectorError> {
    let root: Value =
        serde_json::from_slice(bytes).map_err(|e| RightelCollectorError::Json(e.to_string()))?;
    root.pointer("/data/token")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .ok_or(RightelCollectorError::Authentication)
}

pub fn parse_catalog(
    bytes: &[u8],
    source_url: &str,
) -> Result<RightelCatalog, RightelCollectorError> {
    if bytes.len() as u64 > MAX_RESPONSE_BYTES {
        return Err(RightelCollectorError::TooLarge(bytes.len() as u64));
    }
    let root: Value =
        serde_json::from_slice(bytes).map_err(|e| RightelCollectorError::Json(e.to_string()))?;

    let mut packages = Vec::new();

    if let Some(data_array) = root.pointer("/data").and_then(Value::as_array) {
        for item in data_array {
            if is_internet_package(item) {
                packages.push(raw_from_map(item));
            }
        }
    }

    if packages.is_empty() {
        return Err(RightelCollectorError::EmptyResponse);
    }
    Ok(RightelCatalog {
        source_url: source_url.into(),
        packages,
    })
}

fn is_internet_package(item: &Value) -> bool {
    if let Some(categories) = item.get("channelCategories").and_then(Value::as_array) {
        for cat in categories {
            if let Some(name) = cat.get("channelCategoryNameEn").and_then(Value::as_str) {
                if name.eq_ignore_ascii_case("internet") {
                    return true;
                }
            }
            if let Some(id) = cat.get("channelCategoryId").and_then(|v| v.as_u64().or_else(|| v.as_str().and_then(|s| s.parse().ok()))) {
                if id == 21 {
                    return true;
                }
            }
        }
    }
    false
}
fn raw_from_map(item: &Value) -> RawRightelPackage {
    let mut unknown_fields = Map::new();
    let mut purchasable_package_id = None;
    let mut name_fa = None;
    let mut name_en = None;
    let mut price = None;
    let mut product_type = None;
    let mut offer_code = None;

    if let Some(pkg) = item.get("purchasablePackage").and_then(Value::as_object) {
        for (k, v) in pkg {
            match k.as_str() {
                "purchasablePackageId" => purchasable_package_id = value_to_string(v.clone()),
                "purchasablePackageNameFa" => name_fa = value_to_string(v.clone()),
                "purchasablePackageNameEn" => name_en = value_to_string(v.clone()),
                "packagePrice" => price = Some(v.clone()),
                "mainProductType" => product_type = value_to_string(v.clone()),
                "pricePlanOfferCode" => offer_code = value_to_string(v.clone()),
                _ => {
                    unknown_fields.insert(k.clone(), v.clone());
                }
            }
        }
    }

    let filters = item.get("filters").and_then(Value::as_array).cloned().unwrap_or_default();
    let channel_categories = item.get("channelCategories").and_then(Value::as_array).cloned().unwrap_or_default();

    if let Some(obj) = item.as_object() {
        for (k, v) in obj {
            if k != "purchasablePackage" && k != "filters" && k != "channelCategories" {
                unknown_fields.insert(k.clone(), v.clone());
            }
        }
    }

    RawRightelPackage {
        purchasable_package_id,
        name_fa,
        name_en,
        price,
        product_type,
        offer_code,
        filters,
        channel_categories,
        unknown_fields,
    }
}
fn value_to_string(v: Value) -> Option<String> {
    match v {
        Value::String(s) if !s.trim().is_empty() => Some(s),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}
pub fn rightel_page_url() -> &'static str {
    RIGHTEL_PAGE_URL
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::{validate_snapshot, OperatorSnapshot};
    use crate::normalizers::rightel::RightelNormalizer;

    #[test]
        fn authentication_failure_rejects_missing_token() {
            assert!(matches!(
                parse_auth_token(r#"{"data":{}}"#.as_bytes()),
                Err(RightelCollectorError::Authentication)
            ));
        }

        #[test]
        fn package_retrieval_parser_extracts_internet_only() {
            let json_resp = r#"{
                "data": [
                    {
                        "purchasablePackage": {
                            "purchasablePackageId": 123,
                            "purchasablePackageNameFa": "30 روزه 10 گیگابایت",
                            "packagePrice": 500000,
                            "mainProductType": "PREPAID",
                            "pricePlanOfferCode": "OFF1"
                        },
                        "channelCategories": [{"channelCategoryId": 21, "channelCategoryNameEn": "internet"}]
                    },
                    {
                        "purchasablePackage": {
                            "purchasablePackageId": 124,
                            "purchasablePackageNameFa": "بسته مکالمه",
                            "packagePrice": 10000
                        },
                        "channelCategories": [{"channelCategoryNameEn": "voice"}]
                    }
                ]
            }"#;

            let c = parse_catalog(json_resp.as_bytes(), "fixture").unwrap();
            assert_eq!(c.packages.len(), 1);
            assert_eq!(c.packages[0].purchasable_package_id.as_deref(), Some("123"));
            assert_eq!(c.packages[0].offer_code.as_deref(), Some("OFF1"));
        }

        #[test]
        fn cache_candidate_generation_for_rightel() {
            let json_resp = r#"{
                "data": [
                    {
                        "purchasablePackage": {
                            "purchasablePackageId": "cache-r",
                            "purchasablePackageNameFa": "7 روزه 1 گیگابایت",
                            "packagePrice": 10000,
                            "mainProductType": "PREPAID"
                        },
                        "channelCategories": [{"channelCategoryId": 21, "channelCategoryNameEn": "internet"}]
                    }
                ]
            }"#;
            let c = parse_catalog(json_resp.as_bytes(), "fixture").unwrap();
            let p = RightelNormalizer::normalize(&c.packages[0], 10).unwrap();
            let s = OperatorSnapshot {
                operator: Operator::Rightel,
                fetched_at_unix_seconds: 10,
                stored_at_unix_seconds: 10,
                packages: vec![p],
            };
            validate_snapshot(&s).unwrap();
        }
    }
