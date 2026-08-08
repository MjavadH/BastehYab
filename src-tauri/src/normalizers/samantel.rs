use serde_json::Value;
use thiserror::Error;

use crate::{
    collectors::samantel::{samantel_package_url, RawSamantelPackage},
    domain::{
        allowance::{DataAllowance, DataAllowanceKind, SmsAllowance, VoiceAllowance},
        operator::Operator,
        package::{
            Availability, InternetPackage, PackageKind, PackageMetadata, PurchaseInfo, SimType,
            Validity,
        },
    },
    normalizers::{
        canonical_package_id, clean_text, decimal_data_bytes, money_from_toman, normalize_digits,
        validate_package, DataUnit, NormalizationError,
    },
};

#[derive(Debug, Clone, Copy)]
pub struct SamantelNormalizer;
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SamantelNormalizationError {
    #[error("missing or invalid Samantel package id")]
    MissingId,
    #[error("empty Samantel package name")]
    EmptyName,
    #[error("invalid Samantel package price")]
    InvalidPrice,
    #[error("invalid Samantel data volume")]
    InvalidVolume,
    #[error("canonical package validation failed: {0}")]
    Validation(#[from] NormalizationError),
}

impl SamantelNormalizer {
    pub fn normalize(
        raw: &RawSamantelPackage,
        fetched_at: i64,
    ) -> Result<InternetPackage, SamantelNormalizationError> {
        let external_id = clean_text(
            raw.id
                .as_deref()
                .ok_or(SamantelNormalizationError::MissingId)?,
        )
        .ok_or(SamantelNormalizationError::MissingId)?;
        let name = clean_text(
            raw.name
                .as_deref()
                .ok_or(SamantelNormalizationError::EmptyName)?,
        )
        .ok_or(SamantelNormalizationError::EmptyName)?;
        let price = raw.price.as_ref().map(parse_price).transpose()?;
        let mut data_allowances = Vec::new();
        if let Some(v) = &raw.volume {
            data_allowances.push(parse_allowance_value(v, DataAllowanceKind::General)?);
        }
        for b in &raw.benefits {
            if let Some(a) = parse_benefit(b) {
                data_allowances.push(a?);
            }
        }
        if data_allowances.is_empty() && mentions_data(&name) {
            data_allowances.push(parse_allowance_text(&name, DataAllowanceKind::General)?);
        }
        if data_allowances.is_empty() {
            data_allowances.push(DataAllowance::unknown(DataAllowanceKind::Other));
        }
        let validity = raw
            .validity
            .as_ref()
            .map(parse_validity)
            .unwrap_or_else(|| parse_validity_text(&name));
        let package_kind = if raw
            .benefits
            .iter()
            .any(|b| !mentions_data(&value_to_text(b).unwrap_or_default()))
        {
            PackageKind::Combined
        } else {
            PackageKind::InternetOnly
        };
        let package = InternetPackage {
            id: canonical_package_id(Operator::Samantel, &external_id),
            operator: Operator::Samantel,
            external_id,
            name: name.clone(),
            price,
            validity,
            data_allowances,
            voice: parse_voice(&raw.benefits),
            sms: parse_sms(&raw.benefits),
            sim_types: parse_sim_types(raw.category.as_deref()),
            package_kind,
            availability: Availability::Unknown,
            purchase: PurchaseInfo {
                official_url: raw
                    .purchase_url
                    .clone()
                    .or_else(|| Some(samantel_package_url().into())),
                ussd_code: None,
            },
            metadata: PackageMetadata {
                fetched_at_unix_seconds: Some(fetched_at),
                source_url: Some(samantel_package_url().into()),
                regulatory_code: None,
                offer_code: None,
                original_description: Some(name),
            },
        };
        validate_package(&package)?;
        Ok(package)
    }
}
fn parse_price(v: &Value) -> Result<crate::domain::money::Money, SamantelNormalizationError> {
    let raw = value_to_text(v).ok_or(SamantelNormalizationError::InvalidPrice)?;
    let t = normalize_digits(&raw).replace([',', '٬'], "");
    let digits: String = t.chars().filter(|c| c.is_ascii_digit()).collect();
    let amount: u64 = digits
        .parse()
        .map_err(|_| SamantelNormalizationError::InvalidPrice)?;
    money_from_toman(amount).map_err(|_| SamantelNormalizationError::InvalidPrice)
}
fn parse_allowance_value(
    v: &Value,
    kind: DataAllowanceKind,
) -> Result<DataAllowance, SamantelNormalizationError> {
    let text = value_to_text(v).ok_or(SamantelNormalizationError::InvalidVolume)?;
    parse_allowance_text(&text, kind)
}
fn parse_benefit(v: &Value) -> Option<Result<DataAllowance, SamantelNormalizationError>> {
    let text = value_to_text(v)?;
    mentions_data(&text).then(|| parse_allowance_text(&text, classify_allowance(&text)))
}
fn parse_allowance_text(
    text: &str,
    kind: DataAllowanceKind,
) -> Result<DataAllowance, SamantelNormalizationError> {
    let t = normalize_digits(text).to_lowercase();
    if t.contains("نامحدود") || t.contains("unlimited") {
        let mut a = DataAllowance::unlimited(kind);
        a.description = clean_text(text);
        return Ok(a);
    }
    let unit = if t.contains("گیگ") || t.contains("gb") {
        DataUnit::Gib
    } else if t.contains("مگ") || t.contains("mb") {
        DataUnit::Mib
    } else {
        return Ok(unknown_with_description(kind, text));
    };
    let n = first_number(&t).ok_or(SamantelNormalizationError::InvalidVolume)?;
    let mut a = DataAllowance::finite(
        kind,
        decimal_data_bytes(&n, unit).map_err(|_| SamantelNormalizationError::InvalidVolume)?,
    );
    a.description = clean_text(text);
    Ok(a)
}
fn parse_validity(v: &Value) -> Validity {
    value_to_text(v).map_or(Validity::Unknown, |s| parse_validity_text(&s))
}
fn parse_validity_text(text: &str) -> Validity {
    let t = normalize_digits(text).to_lowercase();
    let Some(n) = first_number_before_duration(&t).and_then(|n| n.parse::<u32>().ok()) else {
        return Validity::Unknown;
    };
    if t.contains("ساعت") || t.contains("hour") {
        Validity::Hours(n)
    } else if t.contains("روز") || t.contains("day") {
        Validity::Days(n)
    } else {
        Validity::Other
    }
}
fn parse_sim_types(c: Option<&str>) -> Vec<SimType> {
    let Some(c) = c else {
        return vec![SimType::Other];
    };
    let c = c.to_lowercase();
    let mut s = Vec::new();
    if c.contains("اعتباری") || c.contains("pre") {
        s.push(SimType::Prepaid)
    }
    if c.contains("دائمی") || c.contains("post") {
        s.push(SimType::Postpaid)
    }
    if s.is_empty() {
        s.push(SimType::Other)
    }
    s
}
fn parse_voice(bs: &[Value]) -> Option<VoiceAllowance> {
    bs.iter()
        .filter_map(value_to_text)
        .any(|t| t.contains("دقیقه") || t.to_lowercase().contains("voice"))
        .then_some(VoiceAllowance {
            minutes: None,
            unlimited: false,
        })
}
fn parse_sms(bs: &[Value]) -> Option<SmsAllowance> {
    bs.iter()
        .filter_map(value_to_text)
        .any(|t| t.contains("پیامک") || t.to_lowercase().contains("sms"))
        .then_some(SmsAllowance {
            count: None,
            unlimited: false,
        })
}
fn classify_allowance(text: &str) -> DataAllowanceKind {
    let t = text.to_lowercase();
    if t.contains("شب") || t.contains("night") {
        DataAllowanceKind::Night
    } else if t.contains("داخلی") || t.contains("domestic") {
        DataAllowanceKind::Domestic
    } else if t.contains("هدیه") || t.contains("gift") {
        DataAllowanceKind::Gift
    } else {
        DataAllowanceKind::Other
    }
}
fn mentions_data(text: &str) -> bool {
    let t = text.to_lowercase();
    t.contains("اینترنت")
        || t.contains("گیگ")
        || t.contains("مگ")
        || t.contains("gb")
        || t.contains("mb")
        || t.contains("نامحدود")
}
fn unknown_with_description(kind: DataAllowanceKind, text: &str) -> DataAllowance {
    let mut a = DataAllowance::unknown(kind);
    a.description = clean_text(text);
    a
}
fn value_to_text(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => clean_text(s),
        Value::Number(n) => Some(n.to_string()),
        Value::Object(m) => m
            .get("value")
            .or_else(|| m.get("text"))
            .or_else(|| m.get("title"))
            .or_else(|| m.get("name"))
            .and_then(value_to_text),
        _ => None,
    }
}
fn first_number(text: &str) -> Option<String> {
    text.split(|c: char| !(c.is_ascii_digit() || c == '.' || c == ','))
        .find(|s| s.chars().any(|c| c.is_ascii_digit()))
        .map(str::to_string)
}
fn first_number_before_duration(text: &str) -> Option<String> {
    first_number(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collectors::samantel::parse_document;
    const NORMAL: &[u8] = include_bytes!("../../tests/fixtures/samantel/normal_object_data.html");
    #[test]
    fn normalizes_samantel_fixture() {
        let c = parse_document(NORMAL, samantel_package_url()).unwrap();
        let p = SamantelNormalizer::normalize(&c.packages[0], 42).unwrap();
        assert_eq!(p.operator, Operator::Samantel);
        assert_eq!(p.price.unwrap().amount, 2_500_000);
        assert_eq!(p.validity, Validity::Days(30));
        assert_eq!(p.data_allowances[0].kind, DataAllowanceKind::General);
        assert_eq!(
            p.data_allowances[0].amount_bytes,
            Some(10 * 1024 * 1024 * 1024)
        );
    }
    #[test]
    fn combined_benefits_are_preserved() {
        let c = parse_document(NORMAL, samantel_package_url()).unwrap();
        let p = SamantelNormalizer::normalize(&c.packages[1], 42).unwrap();
        assert_eq!(p.package_kind, PackageKind::Combined);
        assert!(p.voice.is_some());
        assert!(p.sms.is_some());
        assert!(p
            .data_allowances
            .iter()
            .any(|a| a.kind == DataAllowanceKind::Night));
    }
}
