use std::{process::Command, time::Duration};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use thiserror::Error;

use crate::{
    cache::now_unix_seconds,
    domain::operator::Operator,
    normalizers::irancell::IrancellNormalizer,
    refresh::orchestrator::{CollectedPackages, Collector, CollectorError},
};

const IRANCELL_PRODUCTS_URL: &str = "https://irancell.ir/e/products/5e16bf95d11fd7209ba56b20";
const IRANCELL_PAGE_URL: &str = "https://irancell.ir/o/1001/mobile-internet-packages";
const MAX_RESPONSE_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct IrancellCollector {
    source_url: String,
    timeout: Duration,
}

impl Default for IrancellCollector {
    fn default() -> Self {
        Self {
            source_url: IRANCELL_PRODUCTS_URL.into(),
            timeout: Duration::from_secs(20),
        }
    }
}

impl IrancellCollector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn collect_raw(&self) -> Result<IrancellCatalog, IrancellCollectorError> {
        let timeout = self.timeout.as_secs().max(1).to_string();
        let output = Command::new("curl")
            .args([
                "--fail",
                "--silent",
                "--show-error",
                "--location",
                "--max-time",
                &timeout,
                "--header",
                "Accept: application/json",
                "--user-agent",
                "BastehYab/0.1 (+https://github.com/MjavadH/BastehYab)",
                &self.source_url,
            ])
            .output()
            .map_err(|e| IrancellCollectorError::Request(e.to_string()))?;
        if !output.status.success() {
            return Err(IrancellCollectorError::Status(
                output.status.code().unwrap_or_default() as u16,
            ));
        }
        if output.stdout.len() as u64 > MAX_RESPONSE_BYTES {
            return Err(IrancellCollectorError::TooLarge(output.stdout.len() as u64));
        }
        let catalog = parse_catalog(&output.stdout, &self.source_url)?;
        if catalog.pagination.has_more {
            return Err(IrancellCollectorError::UnsupportedPagination);
        }
        Ok(catalog)
    }
}

impl Collector for IrancellCollector {
    fn collect(&self, operator: Operator) -> Result<CollectedPackages, CollectorError> {
        if operator != Operator::Irancell {
            return Err(CollectorError::Failed(format!(
                "IrancellCollector cannot collect {:?}",
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
            match IrancellNormalizer::normalize(&raw, fetched_at) {
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IrancellCatalog {
    pub source_url: String,
    pub packages: Vec<RawIrancellPackage>,
    pub pagination: IrancellPagination,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IrancellPagination {
    pub observed_total: Option<usize>,
    pub page: Option<u32>,
    pub page_size: Option<u32>,
    pub has_more: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawIrancellPackage {
    pub id: Option<String>,
    pub title: Option<String>,
    pub price: Option<Value>,
    pub volume: Option<Value>,
    pub validity: Option<Value>,
    pub category: Option<String>,
    pub extra_benefits: Vec<Value>,
    pub restrictions: Vec<Value>,
    pub availability: Option<Value>,
    pub purchase_url: Option<String>,
    pub unknown_fields: Map<String, Value>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum IrancellCollectorError {
    #[error("Irancell request failed: {0}")]
    Request(String),
    #[error("Irancell source returned HTTP {0}")]
    Status(u16),
    #[error("Irancell response is too large: {0} bytes")]
    TooLarge(u64),
    #[error("Irancell response is malformed JSON: {0}")]
    Json(String),
    #[error("Irancell response contained no package-like records")]
    NoPackages,
    #[error("Irancell response indicates pagination, which is not expected for the known products endpoint")]
    UnsupportedPagination,
}

pub fn parse_catalog(
    bytes: &[u8],
    source_url: &str,
) -> Result<IrancellCatalog, IrancellCollectorError> {
    if bytes.len() as u64 > MAX_RESPONSE_BYTES {
        return Err(IrancellCollectorError::TooLarge(bytes.len() as u64));
    }
    let root: Value =
        serde_json::from_slice(bytes).map_err(|e| IrancellCollectorError::Json(e.to_string()))?;
    let pagination = extract_pagination(&root);
    let mut packages = Vec::new();
    collect_package_objects(&root, &mut packages);
    if packages.is_empty() {
        return Err(IrancellCollectorError::NoPackages);
    }
    Ok(IrancellCatalog {
        source_url: source_url.into(),
        packages,
        pagination,
    })
}

fn collect_package_objects(value: &Value, out: &mut Vec<RawIrancellPackage>) {
    match value {
        Value::Array(items) => items.iter().for_each(|v| collect_package_objects(v, out)),
        Value::Object(map) => {
            if is_package_like(map) {
                out.push(raw_from_map(map));
            } else {
                map.values().for_each(|v| collect_package_objects(v, out));
            }
        }
        _ => {}
    }
}

fn is_package_like(map: &Map<String, Value>) -> bool {
    let has_identity = any_key(map, &["id", "_id", "productId", "code", "offerCode"]).is_some();
    let has_name = any_key(map, &["title", "name", "productName", "packageName"]).is_some();
    let has_commercial = any_key(map, &["price", "amount", "fee", "cost"]).is_some()
        || any_key(map, &["volume", "traffic", "data", "internet"]).is_some();
    has_identity && has_name && has_commercial
}

fn raw_from_map(map: &Map<String, Value>) -> RawIrancellPackage {
    let mut unknown = map.clone();
    let id = take_string(
        &mut unknown,
        &["id", "_id", "productId", "code", "offerCode"],
    );
    let title = take_string(
        &mut unknown,
        &["title", "name", "productName", "packageName"],
    );
    let price = take_value(&mut unknown, &["price", "amount", "fee", "cost"]);
    let volume = take_value(&mut unknown, &["volume", "traffic", "data", "internet"]);
    let validity = take_value(&mut unknown, &["validity", "duration", "period"]);
    let category = take_string(&mut unknown, &["category", "type", "packageType"]);
    let availability = take_value(
        &mut unknown,
        &["availability", "available", "status", "isActive"],
    );
    let purchase_url = take_string(&mut unknown, &["purchaseUrl", "buyUrl", "url"]);
    let extra_benefits = take_array(
        &mut unknown,
        &["extraBenefits", "benefits", "gifts", "addons"],
    );
    let restrictions = take_array(&mut unknown, &["restrictions", "limitations", "terms"]);
    RawIrancellPackage {
        id,
        title,
        price,
        volume,
        validity,
        category,
        extra_benefits,
        restrictions,
        availability,
        purchase_url,
        unknown_fields: unknown,
    }
}

fn any_key<'a>(map: &'a Map<String, Value>, keys: &[&'a str]) -> Option<&'a str> {
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

fn extract_pagination(root: &Value) -> IrancellPagination {
    let mut p = IrancellPagination::default();
    if let Value::Object(map) = root {
        p.observed_total = map
            .get("total")
            .and_then(Value::as_u64)
            .map(|v| v as usize)
            .or_else(|| map.get("count").and_then(Value::as_u64).map(|v| v as usize));
        p.page = map.get("page").and_then(Value::as_u64).map(|v| v as u32);
        p.page_size = map
            .get("pageSize")
            .and_then(Value::as_u64)
            .map(|v| v as u32);
        p.has_more = map.get("hasMore").and_then(Value::as_bool).unwrap_or(false);
    }
    p
}

pub fn irancell_page_url() -> &'static str {
    IRANCELL_PAGE_URL
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_successful_response_and_preserves_unknown_fields() {
        let c = parse_catalog(
            include_bytes!("../../tests/fixtures/irancell/success.json"),
            "fixture",
        )
        .unwrap();
        assert_eq!(c.packages.len(), 1);
        assert_eq!(c.packages[0].id.as_deref(), Some("pkg-1"));
        assert!(c.packages[0].unknown_fields.contains_key("mystery"));
    }
    #[test]
    fn malformed_response_is_rejected() {
        assert!(matches!(
            parse_catalog(b"{", "fixture"),
            Err(IrancellCollectorError::Json(_))
        ));
    }
    #[test]
    fn missing_package_like_fields_are_not_accepted_as_packages() {
        assert!(matches!(
            parse_catalog(
                include_bytes!("../../tests/fixtures/irancell/missing_fields.json"),
                "fixture"
            ),
            Err(IrancellCollectorError::NoPackages)
        ));
    }

    #[test]
    fn parsed_and_normalized_packages_are_cache_candidates() {
        let json = r#"{"products":[{"id":"cache-1","title":"Cache Package","price":1000,"volume":"1 GB","validity":"7 day","available":true}]}"#;
        let catalog = parse_catalog(json.as_bytes(), "fixture").unwrap();
        let package = IrancellNormalizer::normalize(&catalog.packages[0], 10).unwrap();
        assert_eq!(package.operator, Operator::Irancell);
        assert_eq!(package.id.0, "irancell:cache-1");
        assert_eq!(catalog.packages.len(), 1);
    }
}
