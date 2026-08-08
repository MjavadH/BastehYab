use serde_json::Value;
use thiserror::Error;

use crate::{
    collectors::rightel::{rightel_page_url, RawRightelPackage},
    domain::{
        allowance::{DataAllowance, DataAllowanceKind, SmsAllowance, VoiceAllowance},
        operator::Operator,
        package::{
            Availability, InternetPackage, PackageKind, PackageMetadata, PurchaseInfo, SimType,
            Validity,
        },
    },
    normalizers::{
        canonical_package_id, clean_text, decimal_data_bytes, money_from_irr, normalize_digits,
        validate_package, DataUnit, NormalizationError,
    },
};

#[derive(Debug, Clone, Copy)]
pub struct RightelNormalizer;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RightelNormalizationError {
    #[error("missing or invalid Rightel package id")]
    MissingId,
    #[error("empty Rightel package name")]
    EmptyName,
    #[error("invalid Rightel package price")]
    InvalidPrice,
    #[error("invalid Rightel data volume")]
    InvalidTraffic,
    #[error("canonical package validation failed: {0}")]
    Validation(#[from] NormalizationError),
}

impl RightelNormalizer {
    pub fn normalize(
        raw: &RawRightelPackage,
        fetched_at: i64,
    ) -> Result<InternetPackage, RightelNormalizationError> {
        let external_id = clean_text(
            raw.id
                .as_deref()
                .ok_or(RightelNormalizationError::MissingId)?,
        )
        .ok_or(RightelNormalizationError::MissingId)?;
        let name = clean_text(
            raw.name
                .as_deref()
                .ok_or(RightelNormalizationError::EmptyName)?,
        )
        .ok_or(RightelNormalizationError::EmptyName)?;
        let price = raw.price.as_ref().map(parse_price).transpose()?;
        let mut data_allowances = Vec::new();
        if let Some(traffic) = &raw.traffic {
            data_allowances.push(parse_allowance_value(traffic, DataAllowanceKind::General)?);
        }
        let mut voice = None;
        let mut sms = None;
        let mut has_unknown_benefit = false;
        for benefit in &raw.combined_benefits {
            if let Some(text) = value_to_text(benefit) {
                if mentions_data(&text) {
                    data_allowances.push(parse_allowance_text(&text, classify_allowance(&text))?);
                } else if mentions_sms(&text) {
                    sms = Some(parse_sms(&text));
                } else if mentions_voice(&text) {
                    voice = Some(parse_voice(&text));
                } else {
                    has_unknown_benefit = true;
                }
            } else {
                has_unknown_benefit = true;
            }
        }
        if data_allowances.is_empty() {
            data_allowances.push(DataAllowance::unknown(DataAllowanceKind::Other));
        }
        let package_kind = if voice.is_some() || sms.is_some() || has_unknown_benefit {
            PackageKind::Combined
        } else {
            PackageKind::InternetOnly
        };
        let package = InternetPackage {
            id: canonical_package_id(Operator::Rightel, &external_id), operator: Operator::Rightel, external_id, name,
            price, validity: raw.validity.as_ref().map(parse_validity).unwrap_or(Validity::Unknown), data_allowances, voice, sms,
            sim_types: parse_sim_types(raw), package_kind, availability: Availability::Unknown,
            purchase: PurchaseInfo { official_url: Some(rightel_page_url().into()), ussd_code: None },
            metadata: PackageMetadata { fetched_at_unix_seconds: Some(fetched_at), source_url: Some("https://portal-api.rightel.ir/extra-package/api/v1/extra-package-direct/web-site/purchasable-package".into()), regulatory_code: None, offer_code: None, original_description: raw.name.clone() },
        };
        validate_package(&package)?;
        Ok(package)
    }
}

fn parse_price(value: &Value) -> Result<crate::domain::money::Money, RightelNormalizationError> {
    let raw = value_to_text(value).ok_or(RightelNormalizationError::InvalidPrice)?;
    let normalized = normalize_digits(&raw).replace([',', '٬'], "");
    let digits: String = normalized.chars().filter(|c| c.is_ascii_digit()).collect();
    let amount = digits
        .parse::<u64>()
        .map_err(|_| RightelNormalizationError::InvalidPrice)?;
    Ok(money_from_irr(amount))
}
fn parse_allowance_value(
    value: &Value,
    fallback: DataAllowanceKind,
) -> Result<DataAllowance, RightelNormalizationError> {
    let text = value_to_text(value).ok_or(RightelNormalizationError::InvalidTraffic)?;
    parse_allowance_text(&text, fallback)
}
fn parse_allowance_text(
    text: &str,
    fallback: DataAllowanceKind,
) -> Result<DataAllowance, RightelNormalizationError> {
    let t = normalize_digits(text).to_lowercase();
    let kind = if fallback == DataAllowanceKind::Other {
        DataAllowanceKind::Other
    } else {
        let c = classify_allowance(&t);
        if c == DataAllowanceKind::Other {
            fallback
        } else {
            c
        }
    };
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
        let mut a = DataAllowance::unknown(kind);
        a.description = clean_text(text);
        return Ok(a);
    };
    let number = first_number(&t).ok_or(RightelNormalizationError::InvalidTraffic)?;
    let mut a = DataAllowance::finite(
        kind,
        decimal_data_bytes(&number, unit).map_err(|_| RightelNormalizationError::InvalidTraffic)?,
    );
    a.description = clean_text(text);
    Ok(a)
}
fn parse_validity(value: &Value) -> Validity {
    let Some(text) = value_to_text(value) else {
        return Validity::Unknown;
    };
    let t = normalize_digits(&text).to_lowercase();
    let Some(n) = first_number(&t).and_then(|n| n.parse::<u32>().ok()) else {
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
fn parse_sms(text: &str) -> SmsAllowance {
    SmsAllowance {
        count: first_number(&normalize_digits(text)).and_then(|n| n.parse().ok()),
        unlimited: text.contains("نامحدود") || text.to_lowercase().contains("unlimited"),
    }
}
fn parse_voice(text: &str) -> VoiceAllowance {
    VoiceAllowance {
        minutes: first_number(&normalize_digits(text)).and_then(|n| n.parse().ok()),
        unlimited: text.contains("نامحدود") || text.to_lowercase().contains("unlimited"),
    }
}
fn parse_sim_types(raw: &RawRightelPackage) -> Vec<SimType> {
    let text = raw
        .metadata
        .values()
        .filter_map(value_to_text)
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    let mut sims = Vec::new();
    if text.contains("pre") || text.contains("اعتباری") {
        sims.push(SimType::Prepaid);
    }
    if text.contains("post") || text.contains("دائمی") {
        sims.push(SimType::Postpaid);
    }
    if sims.is_empty() {
        sims.push(SimType::Other);
    }
    sims
}
fn classify_allowance(text: &str) -> DataAllowanceKind {
    let t = text.to_lowercase();
    if t.contains("شب") || t.contains("night") {
        DataAllowanceKind::Night
    } else if t.contains("داخلی") || t.contains("domestic") {
        DataAllowanceKind::Domestic
    } else if t.contains("بین الملل") || t.contains("international") {
        DataAllowanceKind::International
    } else if t.contains("هدیه") || t.contains("gift") {
        DataAllowanceKind::Gift
    } else if t.contains("اجتماعی") || t.contains("social") {
        DataAllowanceKind::Social
    } else {
        DataAllowanceKind::Other
    }
}
fn mentions_data(text: &str) -> bool {
    let t = text.to_lowercase();
    t.contains("gb")
        || t.contains("mb")
        || t.contains("گیگ")
        || t.contains("مگ")
        || t.contains("نامحدود")
}
fn mentions_sms(text: &str) -> bool {
    let t = text.to_lowercase();
    t.contains("sms") || t.contains("پیامک")
}
fn mentions_voice(text: &str) -> bool {
    let t = text.to_lowercase();
    t.contains("voice") || t.contains("مکالمه") || t.contains("دقیقه")
}
fn value_to_text(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => clean_text(s),
        Value::Number(n) => Some(n.to_string()),
        Value::Object(map) => map
            .get("value")
            .or_else(|| map.get("text"))
            .or_else(|| map.get("title"))
            .or_else(|| map.get("name"))
            .and_then(value_to_text),
        _ => None,
    }
}
fn first_number(text: &str) -> Option<String> {
    text.split(|c: char| !(c.is_ascii_digit() || c == '.' || c == ','))
        .find(|s| s.chars().any(|c| c.is_ascii_digit()))
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Map};
    fn raw() -> RawRightelPackage {
        RawRightelPackage {
            id: Some("r1".into()),
            name: Some("بسته رایتل".into()),
            price: Some(json!(100000)),
            traffic: Some(json!("2 گیگابایت")),
            validity: Some(json!("7 روز")),
            combined_benefits: vec![],
            restrictions: vec![],
            metadata: Map::new(),
        }
    }
    #[test]
    fn normalizes_rightel_package() {
        let p = RightelNormalizer::normalize(&raw(), 1).unwrap();
        assert_eq!(p.operator, Operator::Rightel);
        assert_eq!(p.id.0, "rightel:r1");
        assert_eq!(p.price.unwrap().amount, 100000);
        assert_eq!(p.validity, Validity::Days(7));
        assert_eq!(p.data_allowances[0].kind, DataAllowanceKind::General);
    }
    #[test]
    fn combined_packages_preserve_voice_sms_and_restricted_data() {
        let mut r = raw();
        r.combined_benefits = vec![
            json!("50 پیامک"),
            json!("20 دقیقه مکالمه"),
            json!("1 گیگابایت شبانه"),
        ];
        let p = RightelNormalizer::normalize(&r, 1).unwrap();
        assert_eq!(p.package_kind, PackageKind::Combined);
        assert_eq!(p.sms.unwrap().count, Some(50));
        assert_eq!(p.voice.unwrap().minutes, Some(20));
        assert!(p
            .data_allowances
            .iter()
            .any(|a| a.kind == DataAllowanceKind::Night));
    }
    #[test]
    fn missing_identity_is_error_and_missing_validity_is_unknown() {
        let mut r = raw();
        r.id = None;
        assert!(matches!(
            RightelNormalizer::normalize(&r, 1),
            Err(RightelNormalizationError::MissingId)
        ));
        let mut r = raw();
        r.validity = None;
        assert_eq!(
            RightelNormalizer::normalize(&r, 1).unwrap().validity,
            Validity::Unknown
        );
    }
}
