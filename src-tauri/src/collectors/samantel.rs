use std::{process::Command, time::Duration};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use thiserror::Error;

use crate::{
    cache::now_unix_seconds,
    domain::operator::Operator,
    normalizers::samantel::SamantelNormalizer,
    refresh::orchestrator::{CollectedPackages, Collector, CollectorError},
};

const SAMANTEL_PACKAGE_URL: &str = "https://payment.samantel.ir/package";
const MAX_RESPONSE_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct SamantelCollector {
    source_url: String,
    timeout: Duration,
}
impl Default for SamantelCollector {
    fn default() -> Self {
        Self {
            source_url: SAMANTEL_PACKAGE_URL.into(),
            timeout: Duration::from_secs(20),
        }
    }
}
impl SamantelCollector {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn collect_raw(&self) -> Result<SamantelCatalog, SamantelCollectorError> {
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
                "Accept: text/html,application/xhtml+xml",
                "--user-agent",
                "BastehYab/0.1 (+https://github.com/MjavadH/BastehYab)",
                &self.source_url,
            ])
            .output()
            .map_err(|e| SamantelCollectorError::Request(e.to_string()))?;
        if !output.status.success() {
            return Err(SamantelCollectorError::Status(
                output.status.code().unwrap_or_default() as u16,
            ));
        }
        parse_document(&output.stdout, &self.source_url)
    }
}
impl Collector for SamantelCollector {
    fn collect(&self, operator: Operator) -> Result<CollectedPackages, CollectorError> {
        if operator != Operator::Samantel {
            return Err(CollectorError::Failed(format!(
                "SamantelCollector cannot collect {:?}",
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
            match SamantelNormalizer::normalize(&raw, fetched_at) {
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
pub struct SamantelCatalog {
    pub source_url: String,
    pub packages: Vec<RawSamantelPackage>,
    pub extraction: SamantelExtraction,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SamantelExtraction {
    pub strategy: String,
    pub candidate_count: usize,
    pub accepted_count: usize,
    pub rejected_count: usize,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawSamantelPackage {
    pub id: Option<String>,
    pub name: Option<String>,
    pub price: Option<Value>,
    pub volume: Option<Value>,
    pub validity: Option<Value>,
    pub category: Option<String>,
    pub benefits: Vec<Value>,
    pub purchase_url: Option<String>,
    pub unknown_fields: Map<String, Value>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SamantelCollectorError {
    #[error("Samantel request failed: {0}")]
    Request(String),
    #[error("Samantel source returned HTTP {0}")]
    Status(u16),
    #[error("Samantel response is too large: {0} bytes")]
    TooLarge(u64),
    #[error("Samantel response is not valid UTF-8")]
    Utf8,
    #[error("Samantel objectData was not found")]
    MissingObjectData,
    #[error("Samantel objectData is malformed JSON: {0}")]
    Json(String),
    #[error("Samantel response contained no package-like records")]
    NoPackages,
    #[error("Samantel parser confidence is too low: {accepted}/{candidates} candidates accepted")]
    LowConfidence { accepted: usize, candidates: usize },
}

pub fn samantel_package_url() -> &'static str {
    SAMANTEL_PACKAGE_URL
}

pub fn parse_document(
    bytes: &[u8],
    source_url: &str,
) -> Result<SamantelCatalog, SamantelCollectorError> {
    if bytes.len() as u64 > MAX_RESPONSE_BYTES {
        return Err(SamantelCollectorError::TooLarge(bytes.len() as u64));
    }
    let html = std::str::from_utf8(bytes).map_err(|_| SamantelCollectorError::Utf8)?;
    let object = extract_object_data(html).ok_or(SamantelCollectorError::MissingObjectData)?;
    let root: Value =
        serde_json::from_str(&object).map_err(|e| SamantelCollectorError::Json(e.to_string()))?;
    let mut candidates = Vec::new();
    collect_candidate_objects(&root, &mut candidates);
    let candidate_count = candidates.len();
    let packages: Vec<_> = candidates
        .into_iter()
        .filter_map(|m| raw_from_map(&m))
        .collect();
    if packages.is_empty() {
        return Err(SamantelCollectorError::NoPackages);
    }
    if packages.len() * 2 < candidate_count {
        return Err(SamantelCollectorError::LowConfidence {
            accepted: packages.len(),
            candidates: candidate_count,
        });
    }
    Ok(SamantelCatalog {
        source_url: source_url.into(),
        extraction: SamantelExtraction {
            strategy: "embedded_object_data".into(),
            candidate_count,
            accepted_count: packages.len(),
            rejected_count: candidate_count - packages.len(),
        },
        packages,
    })
}

fn extract_object_data(html: &str) -> Option<String> {
    let idx = html.find("objectData")?;
    let after = &html[idx + "objectData".len()..];
    let eq = after.find('=')?;
    let s = after[eq + 1..].trim_start();
    let open = s.find(|c| c == '[' || c == '{')?;
    let s = &s[open..];
    let mut depth = 0i32;
    let mut in_str = false;
    let mut esc = false;
    for (i, ch) in s.char_indices() {
        if in_str {
            if esc {
                esc = false;
            } else if ch == '\\' {
                esc = true;
            } else if ch == '\"' {
                in_str = false;
            }
            continue;
        }
        match ch {
            '"' => in_str = true,
            '[' | '{' => depth += 1,
            ']' | '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(s[..=i].to_string());
                }
            }
            _ => {}
        }
    }
    None
}
fn collect_candidate_objects(v: &Value, out: &mut Vec<Map<String, Value>>) {
    match v {
        Value::Array(a) => a.iter().for_each(|x| collect_candidate_objects(x, out)),
        Value::Object(m) => {
            if has_any(
                m,
                &[
                    "id",
                    "packageId",
                    "code",
                    "name",
                    "title",
                    "price",
                    "amount",
                    "volume",
                    "internet",
                    "duration",
                    "validity",
                ],
            ) {
                out.push(m.clone());
            }
            for x in m.values() {
                collect_candidate_objects(x, out);
            }
        }
        _ => {}
    }
}
fn raw_from_map(map: &Map<String, Value>) -> Option<RawSamantelPackage> {
    let mut unknown = map.clone();
    let id = take_string(&mut unknown, &["id", "packageId", "code", "offerCode"]);
    let name = take_string(&mut unknown, &["name", "title", "packageName", "caption"]);
    let price = take_value(&mut unknown, &["price", "amount", "cost", "fee"]);
    let volume = take_value(&mut unknown, &["volume", "internet", "data", "traffic"]);
    let validity = take_value(&mut unknown, &["validity", "duration", "period"]);
    if id.is_none() || name.is_none() || (price.is_none() && volume.is_none() && validity.is_none())
    {
        return None;
    }
    let category = take_string(
        &mut unknown,
        &["category", "type", "simType", "packageType"],
    );
    let purchase_url = take_string(&mut unknown, &["purchaseUrl", "buyUrl", "url"]);
    let benefits = take_array(
        &mut unknown,
        &["benefits", "gift", "gifts", "details", "items"],
    );
    Some(RawSamantelPackage {
        id,
        name,
        price,
        volume,
        validity,
        category,
        benefits,
        purchase_url,
        unknown_fields: unknown,
    })
}
fn has_any(m: &Map<String, Value>, keys: &[&str]) -> bool {
    keys.iter().any(|k| m.contains_key(*k))
}
fn take_value(m: &mut Map<String, Value>, keys: &[&str]) -> Option<Value> {
    for k in keys {
        if let Some(v) = m.remove(*k) {
            return Some(v);
        }
    }
    None
}
fn take_string(m: &mut Map<String, Value>, keys: &[&str]) -> Option<String> {
    take_value(m, keys).and_then(|v| match v {
        Value::String(s) => Some(s),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    })
}
fn take_array(m: &mut Map<String, Value>, keys: &[&str]) -> Vec<Value> {
    match take_value(m, keys) {
        Some(Value::Array(a)) => a,
        Some(v) => vec![v],
        None => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const NORMAL: &[u8] = include_bytes!("../../tests/fixtures/samantel/normal_object_data.html");
    const EMPTY: &[u8] = include_bytes!("../../tests/fixtures/samantel/empty_object_data.html");
    const CHANGED: &[u8] = include_bytes!("../../tests/fixtures/samantel/changed_markup.html");
    #[test]
    fn parses_normal_object_data() {
        let c = parse_document(NORMAL, SAMANTEL_PACKAGE_URL).unwrap();
        assert_eq!(c.packages.len(), 2);
        assert_eq!(c.extraction.strategy, "embedded_object_data");
        assert_eq!(
            c.packages[0].name.as_deref(),
            Some("بسته اینترنت 10 گیگابایت 30 روزه")
        );
    }
    #[test]
    fn missing_sections_fail_safely() {
        assert!(matches!(
            parse_document(b"<html></html>", SAMANTEL_PACKAGE_URL),
            Err(SamantelCollectorError::MissingObjectData)
        ));
    }
    #[test]
    fn changed_markup_is_rejected_by_confidence() {
        assert!(matches!(
            parse_document(CHANGED, SAMANTEL_PACKAGE_URL),
            Err(SamantelCollectorError::LowConfidence { .. })
        ));
    }
    #[test]
    fn empty_extraction_is_rejected() {
        assert!(matches!(
            parse_document(EMPTY, SAMANTEL_PACKAGE_URL),
            Err(SamantelCollectorError::NoPackages)
        ));
    }
}
