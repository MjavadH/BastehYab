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
    #[error("Samantel package does not contain internet quota")]
    NoInternetQuota,
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
            raw.offer_name
                .as_deref()
                .ok_or(SamantelNormalizationError::EmptyName)?,
        )
        .ok_or(SamantelNormalizationError::EmptyName)?;
        let package_type = raw.package_type.as_deref().unwrap_or_default();
        let voice = parse_voice(&raw.on_voice, &raw.off_voice);
        let sms = parse_sms(&raw.on_sms, &raw.off_sms);
        let data_allowances = parse_data_allowances(raw, package_type)?;
        let package_kind = classify_package_kind(package_type, voice.as_ref(), sms.as_ref());
        let package = InternetPackage {
            id: canonical_package_id(Operator::Samantel, &external_id),
            operator: Operator::Samantel,
            external_id,
            name: name.clone(),
            price: raw.price.as_ref().map(parse_price).transpose()?,
            validity: parse_validity(raw, &name),
            data_allowances,
            voice,
            sms,
            sim_types: vec![SimType::Other],
            package_kind,
            availability: Availability::Unknown,
            purchase: PurchaseInfo {
                official_url: Some(samantel_package_url().into()),
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
    let text = value_to_text(v).ok_or(SamantelNormalizationError::InvalidPrice)?;
    let normalized = normalize_digits(&text);
    let digits: String = normalized.chars().filter(|c| c.is_ascii_digit()).collect();
    let toman: u64 = digits
        .parse()
        .map_err(|_| SamantelNormalizationError::InvalidPrice)?;
    money_from_toman(toman).map_err(|_| SamantelNormalizationError::InvalidPrice)
}

fn parse_data_allowances(
    raw: &RawSamantelPackage,
    package_type: &str,
) -> Result<Vec<DataAllowance>, SamantelNormalizationError> {
    let day = positive_decimal_text(&raw.day_data);
    let night = positive_decimal_text(&raw.night_data);
    let total = positive_decimal_text(&raw.total_data);
    let mut allowances = Vec::new();
    if let Some(day) = day {
        allowances.push(data_allowance(
            data_kind_for_package_type(package_type, false),
            &day,
        )?);
    } else if night.is_none() {
        if let Some(total) = total {
            allowances.push(data_allowance(
                data_kind_for_package_type(package_type, false),
                &total,
            )?);
        }
    }
    if let Some(night) = night {
        allowances.push(data_allowance(DataAllowanceKind::Night, &night)?);
    }
    if allowances.is_empty() {
        return Err(SamantelNormalizationError::NoInternetQuota);
    }
    Ok(allowances)
}

fn data_allowance(
    kind: DataAllowanceKind,
    gb: &str,
) -> Result<DataAllowance, SamantelNormalizationError> {
    decimal_data_bytes(gb, DataUnit::Gib)
        .map(|bytes| DataAllowance::finite(kind, bytes))
        .map_err(|_| SamantelNormalizationError::InvalidVolume)
}

fn parse_voice(on: &Option<Value>, off: &Option<Value>) -> Option<VoiceAllowance> {
    let minutes = positive_u32(on)
        .unwrap_or(0)
        .saturating_add(positive_u32(off).unwrap_or(0));
    (minutes > 0).then_some(VoiceAllowance {
        minutes: Some(minutes),
        unlimited: false,
    })
}

fn parse_sms(on: &Option<Value>, off: &Option<Value>) -> Option<SmsAllowance> {
    let count = positive_u32(on)
        .unwrap_or(0)
        .saturating_add(positive_u32(off).unwrap_or(0));
    (count > 0).then_some(SmsAllowance {
        count: Some(count),
        unlimited: false,
    })
}

fn classify_package_kind(
    package_type: &str,
    voice: Option<&VoiceAllowance>,
    sms: Option<&SmsAllowance>,
) -> PackageKind {
    if voice.is_some() || sms.is_some() || package_type.eq_ignore_ascii_case("VOICE") {
        PackageKind::Combined
    } else {
        PackageKind::InternetOnly
    }
}

fn data_kind_for_package_type(package_type: &str, is_night_field: bool) -> DataAllowanceKind {
    if is_night_field || package_type.eq_ignore_ascii_case("NIGHT") {
        DataAllowanceKind::Night
    } else if package_type.eq_ignore_ascii_case("ROAMING") {
        DataAllowanceKind::International
    } else if package_type.eq_ignore_ascii_case("DATA")
        || package_type.eq_ignore_ascii_case("SPECIAL")
    {
        DataAllowanceKind::General
    } else {
        DataAllowanceKind::Other
    }
}

fn parse_validity(raw: &RawSamantelPackage, name: &str) -> Validity {
    raw.validity
        .as_ref()
        .and_then(value_to_text)
        .and_then(|s| validity_from_text(&s))
        .or_else(|| validity_from_text(name))
        .or_else(|| {
            raw.metadata
                .values()
                .filter_map(value_to_text)
                .find_map(|s| validity_from_text(&s))
        })
        .unwrap_or(Validity::Unknown)
}

fn validity_from_text(text: &str) -> Option<Validity> {
    let normalized = normalize_digits(text);
    let lower = normalized.to_lowercase();
    if lower.contains("روز") || lower.contains("day") {
        first_number(&lower)
            .and_then(|n| n.parse::<u32>().ok())
            .map(Validity::Days)
    } else if lower.contains("ساعت") || lower.contains("hour") {
        first_number(&lower)
            .and_then(|n| n.parse::<u32>().ok())
            .map(Validity::Hours)
    } else {
        None
    }
}

fn positive_decimal_text(v: &Option<Value>) -> Option<String> {
    let text = v.as_ref().and_then(decimal_text)?;
    (text.parse::<f64>().ok()? > 0.0).then_some(text)
}

fn positive_u32(v: &Option<Value>) -> Option<u32> {
    let text = v.as_ref().and_then(decimal_text)?;
    text.parse::<f64>()
        .ok()
        .map(|n| n as u32)
        .filter(|n| *n > 0)
}

fn decimal_text(v: &Value) -> Option<String> {
    match v {
        Value::Number(n) => Some(n.to_string()),
        Value::String(s) => Some(
            normalize_digits(s)
                .replace([',', '٬'], ".")
                .chars()
                .filter(|c| c.is_ascii_digit() || *c == '.')
                .collect(),
        ),
        _ => None,
    }
    .filter(|s| !s.is_empty())
}

fn value_to_text(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => clean_text(s),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn first_number(text: &str) -> Option<String> {
    text.split(|c: char| !(c.is_ascii_digit() || c == '.' || c == ','))
        .find(|s| s.chars().any(|c| c.is_ascii_digit()))
        .map(|s| s.replace(',', "."))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collectors::samantel::parse_catalog;
    use serde_json::{json, Map};

    fn raw(
        id: &str,
        name: &str,
        package_type: &str,
        day: Value,
        night: Value,
    ) -> RawSamantelPackage {
        RawSamantelPackage {
            id: Some(id.into()),
            offer_name: Some(name.into()),
            price: Some(json!(126500)),
            day_data: Some(day.clone()),
            night_data: Some(night.clone()),
            total_data: Some(day),
            validity: Some(json!("(کد 533685)1")),
            on_voice: Some(json!(0)),
            off_voice: Some(json!(0)),
            on_sms: Some(json!(0)),
            off_sms: Some(json!(0)),
            package_type: Some(package_type.into()),
            metadata: Map::new(),
        }
    }

    #[test]
    fn standard_internet_package() {
        let p = SamantelNormalizer::normalize(
            &raw("350", "1 روزه،1 گیگابایت", "DATA", json!(1), json!(0)),
            42,
        )
        .unwrap();
        assert_eq!(p.external_id, "350");
        assert_eq!(p.price.unwrap().amount, 1_265_000);
        assert_eq!(p.validity, Validity::Days(1));
        assert_eq!(p.data_allowances[0].kind, DataAllowanceKind::General);
        assert_eq!(p.data_allowances[0].amount_bytes, Some(1024 * 1024 * 1024));
    }

    #[test]
    fn night_package() {
        let p = SamantelNormalizer::normalize(
            &raw("351", "7 روزه شبانه", "NIGHT", json!(0), json!(1.5)),
            42,
        )
        .unwrap();
        assert_eq!(
            p.data_allowances,
            vec![DataAllowance::finite(
                DataAllowanceKind::Night,
                1_610_612_736
            )]
        );
    }

    #[test]
    fn roaming_package() {
        let p = SamantelNormalizer::normalize(
            &raw("352", "30 روزه رومینگ", "ROAMING", json!(2), json!(0)),
            42,
        )
        .unwrap();
        assert_eq!(p.data_allowances[0].kind, DataAllowanceKind::International);
    }

    #[test]
    fn decimal_volume() {
        let p = SamantelNormalizer::normalize(
            &raw("353", "1 روزه،0.3 گیگابایت", "DATA", json!(0.3), json!(0)),
            42,
        )
        .unwrap();
        assert_eq!(p.data_allowances[0].amount_bytes, Some(322_122_547));
    }

    #[test]
    fn voice_and_sms_make_combined_package() {
        let mut r = raw("354", "30 روزه ویژه", "SPECIAL", json!(1), json!(0));
        r.on_voice = Some(json!(10));
        r.off_voice = Some(json!(5));
        r.on_sms = Some(json!(2));
        let p = SamantelNormalizer::normalize(&r, 42).unwrap();
        assert_eq!(p.package_kind, PackageKind::Combined);
        assert_eq!(p.voice.unwrap().minutes, Some(15));
        assert_eq!(p.sms.unwrap().count, Some(2));
    }

    #[test]
    fn voice_only_package_filtering() {
        let body = json!({"result":[{"OfferID":"v","OfferName":"voice","price":1,"daydata":0,"nightdata":0,"totaldata":0,"expire":"1 روزه","OnVoice":1,"OffVoice":0,"OnSMS":0,"OffSMS":0,"type":"VOICE"}]}).to_string();
        assert!(parse_catalog(body.as_bytes(), "api").is_err());
    }

    #[test]
    fn validity_extraction() {
        for (text, expected) in [
            ("1 روزه", 1),
            ("7 روزه", 7),
            ("30 روزه", 30),
            ("365 روزه", 365),
        ] {
            let mut r = raw("x", text, "DATA", json!(1), json!(0));
            r.validity = None;
            assert_eq!(
                SamantelNormalizer::normalize(&r, 42).unwrap().validity,
                Validity::Days(expected)
            );
        }
    }

    #[test]
    fn snapshot_validation_compatibility() {
        let p = SamantelNormalizer::normalize(
            &raw("355", "30 روزه،1 گیگابایت", "DATA", json!(1), json!(0)),
            42,
        )
        .unwrap();
        validate_package(&p).unwrap();
        let snapshot = serde_json::to_value(&p).unwrap();
        assert_eq!(snapshot["operator"], json!("samantel"));
        assert_eq!(
            snapshot["purchase"]["officialUrl"],
            json!(samantel_package_url())
        );
    }

    #[test]
    fn metadata_preservation() {
        let body = json!({"result":[{"OfferID":"350","OfferName":"1 روزه،1 گیگابایت","price":126500,"priceNoTax":115000,"daydata":1,"nightdata":0,"totaldata":1,"expire":"1 روزه","OnVoice":0,"OffVoice":0,"OnSMS":0,"OffSMS":0,"type":"DATA","unknown":"preserved"}]}).to_string();
        let c = parse_catalog(body.as_bytes(), "api").unwrap();
        assert_eq!(
            c.packages[0].metadata.get("priceNoTax"),
            Some(&json!(115000))
        );
        assert_eq!(
            c.packages[0].metadata.get("unknown"),
            Some(&json!("preserved"))
        );
        let p = SamantelNormalizer::normalize(&c.packages[0], 42).unwrap();
        assert_eq!(
            p.metadata.source_url.as_deref(),
            Some(samantel_package_url())
        );
        assert_eq!(
            p.metadata.original_description.as_deref(),
            Some("1 روزه،1 گیگابایت")
        );
    }
}
