use std::{
    process::Command,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

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
const SAMANTEL_API_URL: &str = "https://payment.samantel.ir/api/mediator/samantel/";
const MAX_RESPONSE_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct SamantelCollector {
    api_url: String,
    timeout: Duration,
}

impl Default for SamantelCollector {
    fn default() -> Self {
        Self {
            api_url: SAMANTEL_API_URL.into(),
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
        let mobile = generated_catalog_mobile();
        let body = format!("method=getpackagelist&mobile={mobile}");
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
                "--header",
                "Content-Type: application/x-www-form-urlencoded",
                "--user-agent",
                "BastehYab/0.1 (+https://github.com/MjavadH/BastehYab)",
                "--data",
                &body,
                &self.api_url,
            ])
            .output()
            .map_err(|e| SamantelCollectorError::Request(e.to_string()))?;
        if !output.status.success() {
            return Err(SamantelCollectorError::Status(
                output.status.code().unwrap_or_default() as u16,
            ));
        }
        parse_catalog(&output.stdout, &self.api_url)
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
    pub api_url: String,
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
    #[serde(alias = "OfferID")]
    pub id: Option<String>,
    #[serde(alias = "OfferName")]
    pub offer_name: Option<String>,
    pub price: Option<Value>,
    #[serde(alias = "daydata")]
    pub day_data: Option<Value>,
    #[serde(alias = "nightdata")]
    pub night_data: Option<Value>,
    #[serde(alias = "totaldata")]
    pub total_data: Option<Value>,
    #[serde(alias = "expire")]
    pub validity: Option<Value>,
    #[serde(alias = "OnVoice")]
    pub on_voice: Option<Value>,
    #[serde(alias = "OffVoice")]
    pub off_voice: Option<Value>,
    #[serde(alias = "OnSMS")]
    pub on_sms: Option<Value>,
    #[serde(alias = "OffSMS")]
    pub off_sms: Option<Value>,
    #[serde(alias = "type")]
    pub package_type: Option<String>,
    #[serde(flatten)]
    pub metadata: Map<String, Value>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SamantelCollectorError {
    #[error("Samantel request failed: {0}")]
    Request(String),
    #[error("Samantel source returned HTTP {0}")]
    Status(u16),
    #[error("Samantel response is too large: {0} bytes")]
    TooLarge(u64),
    #[error("Samantel response is malformed JSON: {0}")]
    Json(String),
    #[error("Samantel response contained no package list")]
    MissingPackageList,
    #[error("Samantel response contained no internet package records")]
    NoPackages,
}

pub fn samantel_package_url() -> &'static str {
    SAMANTEL_PACKAGE_URL
}
pub fn samantel_api_url() -> &'static str {
    SAMANTEL_API_URL
}

pub fn parse_catalog(
    bytes: &[u8],
    api_url: &str,
) -> Result<SamantelCatalog, SamantelCollectorError> {
    if bytes.len() as u64 > MAX_RESPONSE_BYTES {
        return Err(SamantelCollectorError::TooLarge(bytes.len() as u64));
    }
    let root: Value =
        serde_json::from_slice(bytes).map_err(|e| SamantelCollectorError::Json(e.to_string()))?;
    let result = root
        .get("result")
        .and_then(Value::as_array)
        .ok_or(SamantelCollectorError::MissingPackageList)?;
    let candidate_count = result.len();
    let packages: Vec<_> = result
        .iter()
        .filter_map(|v| serde_json::from_value::<RawSamantelPackage>(v.clone()).ok())
        .filter(has_positive_internet_quota)
        .collect();
    if packages.is_empty() {
        return Err(SamantelCollectorError::NoPackages);
    }
    Ok(SamantelCatalog {
        source_url: SAMANTEL_PACKAGE_URL.into(),
        api_url: api_url.into(),
        extraction: SamantelExtraction {
            strategy: "mediator_getpackagelist".into(),
            candidate_count,
            accepted_count: packages.len(),
            rejected_count: candidate_count - packages.len(),
        },
        packages,
    })
}

fn generated_catalog_mobile() -> String {
    let suffix = 30 + random_mod_31();
    format!("099993182{suffix:02}")
}

fn random_mod_31() -> u32 {
    let mut buf = [0_u8; 8];
    if let Ok(mut file) = std::fs::File::open("/dev/urandom") {
        use std::io::Read;
        if file.read_exact(&mut buf).is_ok() {
            return (u64::from_ne_bytes(buf) % 31) as u32;
        }
    }
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos() % 31)
        .unwrap_or(0)
}

fn has_positive_internet_quota(raw: &RawSamantelPackage) -> bool {
    positive_decimal(&raw.day_data)
        || positive_decimal(&raw.night_data)
        || positive_decimal(&raw.total_data)
}

fn positive_decimal(value: &Option<Value>) -> bool {
    value
        .as_ref()
        .and_then(value_to_f64)
        .is_some_and(|n| n > 0.0)
}

fn value_to_f64(value: &Value) -> Option<f64> {
    match value {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse::<f64>().ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture() -> Vec<u8> {
        json!({"result":[
            {"OfferID":"350","OfferName":"1 روزه،1 گیگابایت","price":126500,"priceNoTax":115000,"daydata":1,"nightdata":0,"totaldata":1,"expire":"(کد 533685)1","OnVoice":0,"OffVoice":0,"OnSMS":0,"OffSMS":0,"type":"DATA","extra":"kept"},
            {"OfferID":"351","OfferName":"100 دقیقه مکالمه","price":1000,"daydata":0,"nightdata":0,"totaldata":0,"expire":"1","OnVoice":100,"OffVoice":0,"OnSMS":0,"OffSMS":0,"type":"VOICE"},
            {"OfferID":"352","OfferName":"شبانه 7 روزه،0.3 گیگابایت","price":2000,"daydata":0,"nightdata":0.3,"totaldata":0.3,"expire":"7","OnVoice":0,"OffVoice":0,"OnSMS":0,"OffSMS":0,"type":"NIGHT"}
        ]}).to_string().into_bytes()
    }

    #[test]
    fn parses_api_result_and_filters_voice_only() {
        let catalog = parse_catalog(&fixture(), SAMANTEL_API_URL).unwrap();
        assert_eq!(catalog.extraction.candidate_count, 3);
        assert_eq!(catalog.extraction.accepted_count, 2);
        assert_eq!(catalog.packages[0].id.as_deref(), Some("350"));
        assert_eq!(
            catalog.packages[0].offer_name.as_deref(),
            Some("1 روزه،1 گیگابایت")
        );
        assert_eq!(
            catalog.packages[0].metadata.get("priceNoTax"),
            Some(&json!(115000))
        );
        assert_eq!(
            catalog.packages[0].metadata.get("extra"),
            Some(&json!("kept"))
        );
    }

    #[test]
    fn rejects_missing_result() {
        assert!(matches!(
            parse_catalog(br#"{}"#, SAMANTEL_API_URL),
            Err(SamantelCollectorError::MissingPackageList)
        ));
    }

    #[test]
    fn generated_mobile_uses_public_catalog_range() {
        for _ in 0..100 {
            let mobile = generated_catalog_mobile();
            let suffix: u32 = mobile[9..].parse().unwrap();
            assert!(mobile.starts_with("099993182"));
            assert!((30..=60).contains(&suffix));
        }
    }
}
