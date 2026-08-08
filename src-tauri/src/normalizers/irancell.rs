use serde_json::Value;
use thiserror::Error;

use crate::{
    collectors::irancell::{
        irancell_page_url, IrancellSpecificationContent, LocalizedText, RawIrancellPackage,
    },
    domain::{
        allowance::{DataAllowance, DataAllowanceKind},
        operator::Operator,
        package::{
            Availability, InternetPackage, PackageKind, PackageMetadata, PurchaseInfo, SimType,
            Validity,
        },
    },
    normalizers::{
        canonical_package_id, clean_text, data_bytes, money_from_toman, normalize_digits,
        validate_package, DataUnit, NormalizationError,
    },
};

const IRANCELL_PRODUCTS_URL: &str = "https://irancell.ir/e/products/5e16bf95d11fd7209ba56b20";

#[derive(Debug, Clone, Copy)]
pub struct IrancellNormalizer;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum IrancellNormalizationError {
    #[error("missing or invalid Irancell package id")]
    MissingId,
    #[error("empty Irancell package name")]
    EmptyName,
    #[error("invalid Irancell package price")]
    InvalidPrice,
    #[error("invalid Irancell data volume")]
    InvalidVolume,
    #[error("canonical package validation failed: {0}")]
    Validation(#[from] NormalizationError),
}

impl IrancellNormalizer {
    pub fn normalize(
        raw: &RawIrancellPackage,
        fetched_at: i64,
    ) -> Result<InternetPackage, IrancellNormalizationError> {
        let external_id = clean_text(
            raw.id
                .as_deref()
                .ok_or(IrancellNormalizationError::MissingId)?,
        )
        .ok_or(IrancellNormalizationError::MissingId)?;
        let traffic_mb = parse_traffic_mb(raw)?;
        let validity = parse_validity(raw);
        let name = generate_name(traffic_mb, validity);
        let price = raw.price.as_ref().map(parse_price).transpose()?;
        let sim_types = parse_sim_types(raw);
        let mut general = DataAllowance::finite(
            DataAllowanceKind::General,
            data_bytes(traffic_mb, DataUnit::Mib)
                .map_err(|_| IrancellNormalizationError::InvalidVolume)?,
        );
        general.description = Some(format_traffic(traffic_mb));
        let package = InternetPackage {
            id: canonical_package_id(Operator::Irancell, &external_id),
            operator: Operator::Irancell,
            external_id,
            name,
            price,
            validity,
            data_allowances: vec![general],
            voice: None,
            sms: None,
            sim_types,
            package_kind: PackageKind::InternetOnly,
            availability: Availability::Unknown,
            purchase: PurchaseInfo {
                official_url: Some(irancell_page_url().into()),
                ussd_code: specification(raw, "ussd").and_then(spec_text),
            },
            metadata: PackageMetadata {
                fetched_at_unix_seconds: Some(fetched_at),
                source_url: Some(IRANCELL_PRODUCTS_URL.into()),
                regulatory_code: None,
                offer_code: joined_offer_codes(raw),
                original_description: localized_text(raw.name.as_ref()),
            },
        };
        validate_package(&package)?;
        Ok(package)
    }
}

fn parse_price(value: &Value) -> Result<crate::domain::money::Money, IrancellNormalizationError> {
    let raw = value_to_text(value).ok_or(IrancellNormalizationError::InvalidPrice)?;
    let normalized = normalize_digits(&raw).replace([',', '٬'], "");
    let digits: String = normalized.chars().filter(|c| c.is_ascii_digit()).collect();
    let amount: u64 = digits
        .parse()
        .map_err(|_| IrancellNormalizationError::InvalidPrice)?;
    money_from_toman(amount).map_err(|_| IrancellNormalizationError::InvalidPrice)
}

fn parse_traffic_mb(raw: &RawIrancellPackage) -> Result<u64, IrancellNormalizationError> {
    let spec = specification(raw, "traffic").ok_or(IrancellNormalizationError::InvalidVolume)?;
    let text = spec_text(spec).ok_or(IrancellNormalizationError::InvalidVolume)?;
    let normalized = normalize_digits(&text)
        .replace([',', '٬'], "")
        .trim()
        .to_string();
    if normalized.is_empty() || !normalized.chars().all(|c| c.is_ascii_digit()) {
        return Err(IrancellNormalizationError::InvalidVolume);
    }
    normalized
        .parse()
        .map_err(|_| IrancellNormalizationError::InvalidVolume)
}

fn parse_validity(raw: &RawIrancellPackage) -> Validity {
    let Some(value) = specification(raw, "package_type")
        .and_then(|s| s.value.as_ref())
        .and_then(value_to_text)
    else {
        return Validity::Unknown;
    };
    match value.as_str() {
        "3days" => Validity::Days(3),
        "7days" => Validity::Days(7),
        "10days" => Validity::Days(10),
        "14days" => Validity::Days(14),
        "15days" => Validity::Days(15),
        "30days" => Validity::Days(30),
        "60days" => Validity::Days(60),
        "90days" => Validity::Days(90),
        "120days" => Validity::Days(120),
        "180days" => Validity::Days(180),
        "365days" => Validity::Days(365),
        _ => Validity::Unknown,
    }
}

fn parse_sim_types(raw: &RawIrancellPackage) -> Vec<SimType> {
    let value = specification(raw, "simcard_type")
        .and_then(|s| s.value.as_ref())
        .and_then(value_to_text);
    match value.as_deref() {
        Some("prepaid") => vec![SimType::Prepaid],
        Some("postpaid") => vec![SimType::Postpaid],
        Some("prepaid-postpaid") => vec![SimType::Prepaid, SimType::Postpaid],
        _ => vec![SimType::Other],
    }
}

fn generate_name(traffic_mb: u64, validity: Validity) -> String {
    let validity = match validity {
        Validity::Days(days) => format!("{days} روزه"),
        _ => "اعتبار نامشخص".to_string(),
    };
    format!("{} - {}", format_traffic(traffic_mb), validity)
}

fn format_traffic(mb: u64) -> String {
    if mb % 1024 == 0 {
        format!("{}GB", mb / 1024)
    } else if mb > 1024 && (mb * 10) % 1024 == 0 {
        let tenths = (mb * 10) / 1024;
        format!("{}.{}GB", tenths / 10, tenths % 10)
    } else {
        format!("{mb}MB")
    }
}

fn joined_offer_codes(raw: &RawIrancellPackage) -> Option<String> {
    let codes: Vec<_> = [
        raw.prepaid_offer_code.as_deref(),
        raw.postpaid_offer_code.as_deref(),
        raw.ldms_offer_code.as_deref(),
    ]
    .into_iter()
    .flatten()
    .filter_map(clean_text)
    .collect();
    (!codes.is_empty()).then(|| codes.join(","))
}

fn specification<'a>(
    raw: &'a RawIrancellPackage,
    key: &str,
) -> Option<&'a IrancellSpecificationContent> {
    raw.specification_contents
        .iter()
        .find(|s| s.key.as_deref() == Some(key))
}
fn spec_text(spec: &IrancellSpecificationContent) -> Option<String> {
    spec.value
        .as_ref()
        .and_then(value_to_text)
        .or_else(|| localized_text(spec.desc.as_ref()))
}
fn localized_text(text: Option<&LocalizedText>) -> Option<String> {
    text.and_then(|t| clean_text(t.en.as_deref().expect("REASON")).or_else(|| clean_text(t.fa.as_deref().expect("REASON"))))
}
fn value_to_text(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => clean_text(s),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn raw_json(traffic: &str, package_type: &str, simcard_type: &str) -> RawIrancellPackage {
        serde_json::from_value(serde_json::json!({"_id":"5e4d913aaaad730d6e6cde28","prepaid_offer_code":"PO1956VTH","postpaid_offer_code":"PO2016OEB","price":12700,"specification_contents":[{"key":"simcard_type","value":simcard_type,"desc":{"fa":"سیم کارت اعتباری و دایمی"}},{"key":"package_type","value":package_type,"desc":{"fa":"3 روزه"}},{"key":"traffic","value":"","desc":{"en":traffic,"fa":traffic}},{"key":"ussd","desc":{"en":"*555*5*12#"}}]})).unwrap()
    }
    #[test]
    fn package_without_title_normalizes_successfully_and_generates_name() {
        let p = IrancellNormalizer::normalize(&raw_json("250", "3days", "prepaid-postpaid"), 1)
            .unwrap();
        assert_eq!(p.name, "250MB - 3 روزه");
        assert_eq!(p.id.0, "irancell:5e4d913aaaad730d6e6cde28");
    }
    #[test]
    fn traffic_250_becomes_250mb() {
        let p = IrancellNormalizer::normalize(&raw_json("250", "3days", "prepaid"), 1).unwrap();
        assert_eq!(p.data_allowances[0].amount_bytes, Some(250 * 1024 * 1024));
    }
    #[test]
    fn traffic_4096_becomes_4gb() {
        let p = IrancellNormalizer::normalize(&raw_json("4096", "7days", "prepaid"), 1).unwrap();
        assert_eq!(
            p.data_allowances[0].amount_bytes,
            Some(4 * 1024 * 1024 * 1024)
        );
        assert_eq!(p.name, "4GB - 7 روزه");
    }
    #[test]
    fn package_type_3days_becomes_days_3() {
        let p = IrancellNormalizer::normalize(&raw_json("250", "3days", "prepaid"), 1).unwrap();
        assert_eq!(p.validity, Validity::Days(3));
    }
    #[test]
    fn prepaid_postpaid_becomes_both_sim_types() {
        let p = IrancellNormalizer::normalize(&raw_json("250", "3days", "prepaid-postpaid"), 1)
            .unwrap();
        assert_eq!(p.sim_types, vec![SimType::Prepaid, SimType::Postpaid]);
    }
    #[test]
    fn ussd_extraction_works() {
        let p = IrancellNormalizer::normalize(&raw_json("250", "3days", "prepaid"), 1).unwrap();
        assert_eq!(p.purchase.ussd_code.as_deref(), Some("*555*5*12#"));
    }
    #[test]
    fn real_sample_payload_normalizes_successfully_without_empty_name() {
        let raws: Vec<RawIrancellPackage> = serde_json::from_slice(include_bytes!(
            "../../tests/fixtures/irancell/real_sample.json"
        ))
        .unwrap();
        let packages: Vec<_> = raws
            .iter()
            .map(|r| IrancellNormalizer::normalize(r, 1))
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(packages[0].name, "750MB - 3 روزه");
        assert_eq!(packages[1].name, "300MB - 7 روزه");
        assert_eq!(packages[2].name, "4GB - 7 روزه");
        assert_eq!(packages[3].name, "50GB - 30 روزه");
    }
    #[test]
    fn empty_name_error_does_not_occur_for_valid_real_record() {
        let mut raw = raw_json("1536", "3days", "postpaid");
        raw.name = None;
        let p = IrancellNormalizer::normalize(&raw, 1).unwrap();
        assert_eq!(p.name, "1.5GB - 3 روزه");
    }
}
