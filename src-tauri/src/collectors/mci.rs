use crate::{
    cache::now_unix_seconds,
    domain::operator::Operator,
    normalizers::mci::MCINormalizer,
    refresh::orchestrator::{CollectedPackages, Collector, CollectorError},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeSet, process::Command, time::Duration};
use thiserror::Error;

pub const MCI_PAGE_URL: &str = "https://mci.ir/internet-plans";
const MCI_PRODUCTS_URL: &str =
    "https://shop.mci.ir/api/search/v1/products?category=19&page=0&size=100";
const MAX_RESPONSE_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct MCICollector {
    source_url: String,
    timeout: Duration,
}

impl Default for MCICollector {
    fn default() -> Self {
        Self {
            source_url: MCI_PRODUCTS_URL.into(),
            timeout: Duration::from_secs(20),
        }
    }
}

impl MCICollector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn collect_raw(&self) -> Result<MCICatalog, MCICollectorError> {
        parse_catalog(&self.fetch_products()?, &self.source_url)
    }

    fn fetch_products(&self) -> Result<Vec<u8>, MCICollectorError> {
        let timeout = self.timeout.as_secs().max(1).to_string();
        let o = Command::new("curl")
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
            .map_err(|e| MCICollectorError::Request(e.to_string()))?;
        if !o.status.success() {
            return Err(MCICollectorError::Status(
                o.status.code().unwrap_or_default() as u16,
            ));
        }
        if o.stdout.len() as u64 > MAX_RESPONSE_BYTES {
            return Err(MCICollectorError::TooLarge(o.stdout.len() as u64));
        }
        Ok(o.stdout)
    }
}

impl Collector for MCICollector {
    fn collect(&self, operator: Operator) -> Result<CollectedPackages, CollectorError> {
        if operator != Operator::Mci {
            return Err(CollectorError::Failed(format!(
                "MCICollector cannot collect {:?}",
                operator
            )));
        }
        let fetched_at = now_unix_seconds();
        let catalog = self
            .collect_raw()
            .map_err(|e| CollectorError::Failed(e.to_string()))?;
        let raw_record_count = catalog.packages.len();
        println!("MCI products collected: {raw_record_count}");
        let mut packages = Vec::new();
        let mut normalization_failures = 0;
        for raw in catalog.packages {
            match MCINormalizer::normalize(&raw, fetched_at) {
                Ok(p) => packages.push(p),
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
pub struct MCICatalog {
    pub source_url: String,
    pub packages: Vec<RawMCIPackage>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawMCIPackage {
    pub id: String,
    pub title: Option<String>,
    pub price: Option<Value>,
    pub traffic_mb: Option<Value>,
    pub validity: Option<Value>,
    pub package_type: Option<String>,
    pub unit_kind: Option<String>,
    pub attributes: Vec<MCIAttribute>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MCIAttribute {
    pub title: Option<String>,
    #[serde(default)]
    pub attribute_value_vms: Vec<MCIAttributeValue>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MCIAttributeValue {
    pub value: Option<Value>,
    pub display_text: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MCIProductsResponse {
    products: Vec<MCIProduct>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MCIProduct {
    id: Value,
    price: Option<Value>,
    #[serde(default)]
    attributes: Vec<MCIAttribute>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MCICollectorError {
    #[error("MCI request failed: {0}")]
    Request(String),
    #[error("MCI source returned HTTP {0}")]
    Status(u16),
    #[error("MCI response is too large: {0} bytes")]
    TooLarge(u64),
    #[error("MCI response is malformed JSON: {0}")]
    Json(String),
    #[error("MCI response contained no products")]
    NoPackages,
}

pub fn parse_catalog(bytes: &[u8], source_url: &str) -> Result<MCICatalog, MCICollectorError> {
    if bytes.len() as u64 > MAX_RESPONSE_BYTES {
        return Err(MCICollectorError::TooLarge(bytes.len() as u64));
    }
    let response: MCIProductsResponse =
        serde_json::from_slice(bytes).map_err(|e| MCICollectorError::Json(e.to_string()))?;
    let mut packages = Vec::new();
    let mut ids = BTreeSet::new();
    for product in response.products {
        let raw = raw_from_product(product);
        if ids.insert(raw.id.clone()) {
            packages.push(raw);
        }
    }
    if packages.is_empty() {
        return Err(MCICollectorError::NoPackages);
    }
    Ok(MCICatalog {
        source_url: source_url.into(),
        packages,
    })
}

fn raw_from_product(product: MCIProduct) -> RawMCIPackage {
    let id = value_to_string(&product.id).unwrap_or_else(|| product.id.to_string());
    let title = attribute_text(&product.attributes, "title");
    let traffic_mb = attribute_value(&product.attributes, "حجم");
    let validity = attribute_value(&product.attributes, "بازه زمانی بسته");
    let package_type = attribute_text(&product.attributes, "نوع بسته");
    let unit_kind = attribute_text(&product.attributes, "unitKind");
    RawMCIPackage {
        id,
        title,
        price: product.price,
        traffic_mb,
        validity,
        package_type,
        unit_kind,
        attributes: product.attributes,
    }
}

fn attribute_text(attributes: &[MCIAttribute], title: &str) -> Option<String> {
    attribute_value(attributes, title).and_then(|v| value_to_string(&v))
}

fn attribute_value(attributes: &[MCIAttribute], title: &str) -> Option<Value> {
    attributes
        .iter()
        .find(|a| a.title.as_deref() == Some(title))
        .and_then(|a| a.attribute_value_vms.first())
        .and_then(|v| {
            v.value
                .clone()
                .or_else(|| v.display_text.clone().map(Value::String))
        })
}

fn value_to_string(v: &Value) -> Option<String> {
    match v {
        Value::String(s) if !s.trim().is_empty() => Some(s.trim().to_owned()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

pub fn mci_page_url() -> &'static str {
    MCI_PAGE_URL
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_products_from_shop_api_response() {
        let c = parse_catalog(
            include_bytes!("../../tests/fixtures/mci/products.json"),
            "fixture",
        )
        .unwrap();
        assert_eq!(c.packages.len(), 2);
        assert_eq!(c.packages[0].id, "825596");
        assert_eq!(c.packages[0].title.as_deref(), Some("100 گیگابایت اینترنت"));
        assert_eq!(
            c.packages[0].traffic_mb,
            Some(Value::String("102400".into()))
        );
        assert_eq!(
            c.packages[0].validity,
            Some(Value::String("120 روزه".into()))
        );
    }

    #[test]
    fn prevents_duplicate_product_ids() {
        let c = parse_catalog(
            include_bytes!("../../tests/fixtures/mci/products-duplicate.json"),
            "fixture",
        )
        .unwrap();
        assert_eq!(c.packages.len(), 1);
    }

    #[test]
    fn empty_products_are_rejected() {
        assert!(matches!(
            parse_catalog(br#"{"products":[]}"#, "fixture"),
            Err(MCICollectorError::NoPackages)
        ));
    }
}
