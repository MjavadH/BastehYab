use crate::{
    cache::now_unix_seconds,
    domain::operator::Operator,
    normalizers::mci::MCINormalizer,
    refresh::orchestrator::{CollectedPackages, Collector, CollectorError},
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::{collections::BTreeSet, process::Command, time::Duration};
use thiserror::Error;
pub const MCI_PAGE_URL: &str = "https://mci.ir/internet-plans";
const MCI_PRODUCTS_URL_TEMPLATE: &str = "https://mci.ir/api/products?page={page}&size=10";
const MAX_RESPONSE_BYTES: u64 = 2 * 1024 * 1024;
const MAX_PAGES: u32 = 50;
#[derive(Debug, Clone)]
pub struct MCICollector {
    url_template: String,
    timeout: Duration,
}
impl Default for MCICollector {
    fn default() -> Self {
        Self {
            url_template: MCI_PRODUCTS_URL_TEMPLATE.into(),
            timeout: Duration::from_secs(20),
        }
    }
}
impl MCICollector {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn collect_raw(&self) -> Result<MCICatalog, MCICollectorError> {
        collect_pages(|p| self.fetch_page(p), &self.url_template)
    }
    fn fetch_page(&self, page: u32) -> Result<Vec<u8>, MCICollectorError> {
        let url = self.url_template.replace("{page}", &page.to_string());
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
                "BastehYab/0.1 (+https://github.com/BastehYab/BastehYab)",
                &url,
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
    pub pagination: MCIPagination,
}
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MCIPagination {
    pub observed_total: Option<usize>,
    pub pages_fetched: u32,
    pub page_size: Option<u32>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawMCIPackage {
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
    pub ussd_code: Option<String>,
    pub unknown_fields: Map<String, Value>,
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
    #[error("MCI response contained no package-like records")]
    NoPackages,
    #[error("MCI pagination stopped on an empty page before advertised total was reached")]
    PartialPagination,
    #[error("MCI pagination exceeded the configured page limit")]
    PageLimit,
}
pub fn parse_page(bytes: &[u8], source_url: &str) -> Result<MCICatalog, MCICollectorError> {
    if bytes.len() as u64 > MAX_RESPONSE_BYTES {
        return Err(MCICollectorError::TooLarge(bytes.len() as u64));
    }
    let root: Value =
        serde_json::from_slice(bytes).map_err(|e| MCICollectorError::Json(e.to_string()))?;
    let pagination = extract_pagination(&root);
    let mut packages = Vec::new();
    collect_package_objects(&root, &mut packages);
    Ok(MCICatalog {
        source_url: source_url.into(),
        packages,
        pagination,
    })
}
pub fn collect_pages<F>(mut fetch: F, source_url: &str) -> Result<MCICatalog, MCICollectorError>
where
    F: FnMut(u32) -> Result<Vec<u8>, MCICollectorError>,
{
    let mut all = Vec::new();
    let mut ids = BTreeSet::new();
    let mut observed_total = None;
    let mut page_size = None;
    for page in 0..MAX_PAGES {
        let parsed = parse_page(&fetch(page)?, source_url)?;
        observed_total = parsed.pagination.observed_total.or(observed_total);
        page_size = parsed
            .pagination
            .page_size
            .or(page_size)
            .or(Some(parsed.packages.len() as u32));
        let parsed_len = parsed.packages.len();
        if parsed_len == 0 {
            return if all.is_empty() {
                Err(MCICollectorError::NoPackages)
            } else if observed_total.is_some_and(|t| all.len() < t) {
                Err(MCICollectorError::PartialPagination)
            } else {
                Ok(MCICatalog {
                    source_url: source_url.into(),
                    packages: all,
                    pagination: MCIPagination {
                        observed_total,
                        pages_fetched: page,
                        page_size,
                    },
                })
            };
        }
        for pkg in parsed.packages {
            if let Some(id) = package_identity(&pkg) {
                if ids.insert(id) {
                    all.push(pkg)
                }
            } else {
                all.push(pkg)
            }
        }
        if observed_total.is_some_and(|t| all.len() >= t)
            || parsed
                .pagination
                .page_size
                .is_some_and(|s| parsed_len < (s as usize))
        {
            return Ok(MCICatalog {
                source_url: source_url.into(),
                packages: all,
                pagination: MCIPagination {
                    observed_total,
                    pages_fetched: page + 1,
                    page_size,
                },
            });
        }
    }
    Err(MCICollectorError::PageLimit)
}
fn collect_package_objects(value: &Value, out: &mut Vec<RawMCIPackage>) {
    match value {
        Value::Array(a) => a.iter().for_each(|v| collect_package_objects(v, out)),
        Value::Object(m) => {
            if is_package_like(m) {
                out.push(raw_from_map(m))
            } else {
                m.values().for_each(|v| collect_package_objects(v, out))
            }
        }
        _ => {}
    }
}
fn is_package_like(m: &Map<String, Value>) -> bool {
    any_key(
        m,
        &["id", "_id", "productId", "code", "offerCode", "packageId"],
    )
    .is_some()
        && any_key(m, &["title", "name", "productName", "packageName"]).is_some()
}
fn raw_from_map(m: &Map<String, Value>) -> RawMCIPackage {
    let mut u = m.clone();
    RawMCIPackage {
        id: take_string(
            &mut u,
            &["id", "_id", "productId", "code", "offerCode", "packageId"],
        ),
        title: take_string(&mut u, &["title", "name", "productName", "packageName"]),
        price: take_value(&mut u, &["price", "amount", "fee", "cost", "finalPrice"]),
        volume: take_value(&mut u, &["volume", "traffic", "data", "internet"]),
        validity: take_value(&mut u, &["validity", "duration", "period"]),
        category: take_string(&mut u, &["category", "type", "packageType", "simType"]),
        extra_benefits: take_array(
            &mut u,
            &["extraBenefits", "benefits", "gifts", "addons", "attributes"],
        ),
        restrictions: take_array(&mut u, &["restrictions", "limitations", "terms"]),
        availability: take_value(&mut u, &["availability", "available", "status", "isActive"]),
        purchase_url: take_string(&mut u, &["purchaseUrl", "buyUrl", "url"]),
        ussd_code: take_string(&mut u, &["ussd", "ussdCode", "activationCode"]),
        unknown_fields: u,
    }
}
fn package_identity(p: &RawMCIPackage) -> Option<String> {
    p.id.clone()
        .or_else(|| p.ussd_code.clone())
        .or_else(|| p.title.clone())
}
fn any_key<'a>(m: &'a Map<String, Value>, keys: &[&str]) -> Option<&'a str> {
    keys.iter().copied().find(|k| m.contains_key(*k))
}
fn take_value(m: &mut Map<String, Value>, keys: &[&str]) -> Option<Value> {
    let k = any_key(m, keys)?.to_string();
    m.remove(&k)
}
fn take_string(m: &mut Map<String, Value>, keys: &[&str]) -> Option<String> {
    take_value(m, keys).and_then(value_to_string)
}
fn take_array(m: &mut Map<String, Value>, keys: &[&str]) -> Vec<Value> {
    match take_value(m, keys) {
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
fn extract_pagination(root: &Value) -> MCIPagination {
    let mut p = MCIPagination::default();
    if let Value::Object(m) = root {
        p.observed_total =
            first_u64(m, &["total", "totalCount", "recordsTotal"]).map(|v| v as usize);
        p.page_size = first_u64(m, &["pageSize", "size", "perPage", "limit"]).map(|v| v as u32)
    }
    p
}
fn first_u64(m: &Map<String, Value>, keys: &[&str]) -> Option<u64> {
    keys.iter().find_map(|k| m.get(*k).and_then(Value::as_u64))
}
pub fn mci_page_url() -> &'static str {
    MCI_PAGE_URL
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_first_page() {
        let c = parse_page(
            include_bytes!("../../tests/fixtures/mci/page1.json"),
            "fixture",
        )
        .unwrap();
        assert_eq!(c.packages.len(), 2);
        assert_eq!(c.pagination.observed_total, Some(4));
        assert_eq!(c.packages[0].ussd_code.as_deref(), Some("*100*211#"));
    }
    #[test]
    fn collects_multiple_pages_and_detects_final_page() {
        let pages = [
            include_bytes!("../../tests/fixtures/mci/page1.json").to_vec(),
            include_bytes!("../../tests/fixtures/mci/page2.json").to_vec(),
        ];
        let c = collect_pages(|p| Ok(pages[p as usize].clone()), "fixture").unwrap();
        assert_eq!(c.packages.len(), 4);
        assert_eq!(c.pagination.pages_fetched, 2);
    }
    #[test]
    fn prevents_duplicates() {
        let pages = [
            include_bytes!("../../tests/fixtures/mci/page1.json").to_vec(),
            include_bytes!("../../tests/fixtures/mci/page2-duplicate.json").to_vec(),
        ];
        let c = collect_pages(|p| Ok(pages[p as usize].clone()), "fixture").unwrap();
        assert_eq!(c.packages.len(), 3);
    }
    #[test]
    fn partial_pagination_is_rejected() {
        let pages = [
            include_bytes!("../../tests/fixtures/mci/page1.json").to_vec(),
            br#"{"total":4,"pageSize":2,"products":[]}"#.to_vec(),
        ];
        assert!(matches!(
            collect_pages(|p| Ok(pages[p as usize].clone()), "fixture"),
            Err(MCICollectorError::PartialPagination)
        ));
    }
}
