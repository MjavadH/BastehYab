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
        let mut packages = Vec::new();
        let mut normalization_failures = 0;
        for raw in catalog.packages {
            match RightelNormalizer::normalize(&raw, fetched_at) {
                Ok(package) => packages.push(package),
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
        "BastehYab/0.1 (+https://github.com/BastehYab/BastehYab)",
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
    pub id: Option<String>,
    pub name: Option<String>,
    pub price: Option<Value>,
    pub traffic: Option<Value>,
    pub validity: Option<Value>,
    pub combined_benefits: Vec<Value>,
    pub restrictions: Vec<Value>,
    pub metadata: Map<String, Value>,
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
    collect_package_objects(&root, &mut packages);
    if packages.is_empty() {
        return Err(RightelCollectorError::EmptyResponse);
    }
    Ok(RightelCatalog {
        source_url: source_url.into(),
        packages,
    })
}

fn collect_package_objects(value: &Value, out: &mut Vec<RawRightelPackage>) {
    match value {
        Value::Array(items) => items.iter().for_each(|v| collect_package_objects(v, out)),
        Value::Object(map) if is_package_like(map) => out.push(raw_from_map(map)),
        Value::Object(map) => map.values().for_each(|v| collect_package_objects(v, out)),
        _ => {}
    }
}
fn is_package_like(map: &Map<String, Value>) -> bool {
    any_key(map, &["id", "packageId", "code", "offerCode"]).is_some()
        && any_key(map, &["name", "title", "packageName"]).is_some()
        && (any_key(map, &["price", "amount", "cost", "fee"]).is_some()
            || any_key(map, &["traffic", "volume", "data", "internet"]).is_some())
}
fn raw_from_map(map: &Map<String, Value>) -> RawRightelPackage {
    let mut metadata = map.clone();
    RawRightelPackage {
        id: take_string(&mut metadata, &["id", "packageId", "code", "offerCode"]),
        name: take_string(&mut metadata, &["name", "title", "packageName"]),
        price: take_value(&mut metadata, &["price", "amount", "cost", "fee"]),
        traffic: take_value(&mut metadata, &["traffic", "volume", "data", "internet"]),
        validity: take_value(&mut metadata, &["validity", "duration", "period"]),
        combined_benefits: take_array(
            &mut metadata,
            &[
                "benefits",
                "combinedBenefits",
                "gift",
                "gifts",
                "details",
                "items",
            ],
        ),
        restrictions: take_array(&mut metadata, &["restrictions", "limitations", "terms"]),
        metadata,
    }
}
fn any_key<'a>(map: &'a Map<String, Value>, keys: &[&str]) -> Option<&'a str> {
    keys.iter().copied().find(|k| map.contains_key(*k))
}
fn take_value(map: &mut Map<String, Value>, keys: &[&str]) -> Option<Value> {
    let k = any_key(map, keys)?.to_string();
    map.remove(&k)
}
fn take_string(map: &mut Map<String, Value>, keys: &[&str]) -> Option<String> {
    take_value(map, keys).and_then(value_to_string)
}
fn take_array(map: &mut Map<String, Value>, keys: &[&str]) -> Vec<Value> {
    match take_value(map, keys) {
        Some(Value::Array(v)) => v,
        Some(v) => vec![v],
        None => Vec::new(),
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
    fn authentication_success_extracts_token() {
        assert_eq!(
            parse_auth_token(r#"{"data":{"token":"abc"}}"#.as_bytes()).unwrap(),
            "abc"
        );
    }
    #[test]
    fn authentication_failure_rejects_missing_token() {
        assert!(matches!(
            parse_auth_token(r#"{"data":{}}"#.as_bytes()),
            Err(RightelCollectorError::Authentication)
        ));
    }
    #[test]
    fn package_retrieval_parser_preserves_raw_fields() {
        let c = parse_catalog(r#"{"data":[{"packageId":"r1","packageName":"Rightel 1GB","price":10000,"traffic":"1 GB","duration":"7 روز","benefits":["100 پیامک"],"limitations":["روزانه"],"meta":"x"}]}"#.as_bytes(), "fixture").unwrap();
        assert_eq!(c.packages.len(), 1);
        assert_eq!(c.packages[0].id.as_deref(), Some("r1"));
        assert!(c.packages[0].metadata.contains_key("meta"));
    }
    #[test]
    fn cache_candidate_generation_for_rightel() {
        let c = parse_catalog(r#"{"data":[{"packageId":"cache-r","packageName":"Rightel 1GB","price":10000,"traffic":"1 GB","duration":"7 روز"}]}"#.as_bytes(), "fixture").unwrap();
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
