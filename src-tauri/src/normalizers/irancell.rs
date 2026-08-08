use serde_json::Value;
use thiserror::Error;

use crate::{
    collectors::irancell::{irancell_page_url, RawIrancellPackage},
    domain::{
        allowance::{DataAllowance, DataAllowanceKind},
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
        let name = clean_text(
            raw.title
                .as_deref()
                .ok_or(IrancellNormalizationError::EmptyName)?,
        )
        .ok_or(IrancellNormalizationError::EmptyName)?;
        let price = raw.price.as_ref().map(parse_price).transpose()?;
        let mut data_allowances = Vec::new();
        if let Some(volume) = &raw.volume {
            data_allowances.push(parse_volume(volume)?);
        }
        for benefit in &raw.extra_benefits {
            if let Some(a) = parse_extra_benefit(benefit) {
                data_allowances.push(a?);
            }
        }
        if data_allowances.is_empty() {
            data_allowances.push(DataAllowance::unknown(DataAllowanceKind::Other));
        }
        let validity = raw
            .validity
            .as_ref()
            .map(parse_validity)
            .unwrap_or(Validity::Unknown);
        let sim_types = parse_sim_types(raw.category.as_deref());
        let package_kind = if raw.extra_benefits.is_empty() {
            PackageKind::InternetOnly
        } else {
            PackageKind::Combined
        };
        let availability = parse_availability(raw.availability.as_ref());
        let purchase = PurchaseInfo {
            official_url: raw
                .purchase_url
                .clone()
                .or_else(|| Some(irancell_page_url().into())),
            ussd_code: None,
        };
        let package = InternetPackage {
            id: canonical_package_id(Operator::Irancell, &external_id),
            operator: Operator::Irancell,
            external_id,
            name,
            price,
            validity,
            data_allowances,
            voice: None,
            sms: None,
            sim_types,
            package_kind,
            availability,
            purchase,
            metadata: PackageMetadata {
                fetched_at_unix_seconds: Some(fetched_at),
                source_url: Some("https://irancell.ir/e/products/5e16bf95d11fd7209ba56b20".into()),
                regulatory_code: None,
                offer_code: None,
                original_description: raw.title.clone(),
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
    // Irancell retail pages typically present toman amounts. Do not infer IRR from bare strings.
    money_from_toman(amount).map_err(|_| IrancellNormalizationError::InvalidPrice)
}

fn parse_volume(value: &Value) -> Result<DataAllowance, IrancellNormalizationError> {
    let text = value_to_text(value).ok_or(IrancellNormalizationError::InvalidVolume)?;
    parse_allowance_text(&text, DataAllowanceKind::General)
}

fn parse_extra_benefit(value: &Value) -> Option<Result<DataAllowance, IrancellNormalizationError>> {
    let text = value_to_text(value)?;
    let kind = classify_allowance(&text);
    if mentions_data(&text) {
        Some(parse_allowance_text(&text, kind))
    } else {
        None
    }
}

fn parse_allowance_text(
    text: &str,
    kind: DataAllowanceKind,
) -> Result<DataAllowance, IrancellNormalizationError> {
    let t = normalize_digits(text).to_lowercase();
    if t.contains("نامحدود") || t.contains("unlimited") {
        return Ok(DataAllowance::unlimited(kind));
    }
    let unit = if t.contains("گیگ") || t.contains("gb") {
        DataUnit::Gib
    } else if t.contains("مگ") || t.contains("mb") {
        DataUnit::Mib
    } else {
        return Ok(unknown_with_description(kind, text));
    };
    let number = first_number(&t).ok_or(IrancellNormalizationError::InvalidVolume)?;
    let mut a = DataAllowance::finite(
        kind,
        decimal_data_bytes(&number, unit).map_err(|_| IrancellNormalizationError::InvalidVolume)?,
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

fn parse_sim_types(category: Option<&str>) -> Vec<SimType> {
    let Some(c) = category else {
        return vec![SimType::Other];
    };
    let c = c.to_lowercase();
    let mut sims = Vec::new();
    if c.contains("pre") || c.contains("اعتباری") {
        sims.push(SimType::Prepaid);
    }
    if c.contains("post") || c.contains("دائمی") {
        sims.push(SimType::Postpaid);
    }
    if c.contains("td") {
        sims.push(SimType::Tdlte);
    }
    if sims.is_empty() {
        sims.push(SimType::Other);
    }
    sims
}

fn parse_availability(value: Option<&Value>) -> Availability {
    match value {
        Some(Value::Bool(true)) => Availability::Available,
        Some(Value::Bool(false)) => Availability::Unavailable,
        Some(v) => value_to_text(v)
            .map(|s| s.to_lowercase())
            .map_or(Availability::Unknown, |s| {
                if s.contains("unavailable") || s.contains("inactive") || s.contains("غیرفعال")
                {
                    Availability::Unavailable
                } else if s.contains("available") || s.contains("active") || s.contains("فعال")
                {
                    Availability::Available
                } else {
                    Availability::Unknown
                }
            }),
        None => Availability::Unknown,
    }
}

fn classify_allowance(text: &str) -> DataAllowanceKind {
    let t = text.to_lowercase();
    if t.contains("شب") || t.contains("night") {
        DataAllowanceKind::Night
    } else if t.contains("داخلی") || t.contains("domestic") {
        DataAllowanceKind::Domestic
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
fn unknown_with_description(kind: DataAllowanceKind, text: &str) -> DataAllowance {
    let mut a = DataAllowance::unknown(kind);
    a.description = clean_text(text);
    a
}
fn value_to_text(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => clean_text(s),
        Value::Number(n) => Some(n.to_string()),
        Value::Object(map) => map
            .get("value")
            .or_else(|| map.get("text"))
            .or_else(|| map.get("title"))
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
    use serde_json::json;

    fn raw() -> RawIrancellPackage {
        RawIrancellPackage {
            id: Some("pkg-1".into()),
            title: Some("بسته ترکیبی".into()),
            price: Some(json!(120000)),
            volume: Some(json!("10 گیگابایت")),
            validity: Some(json!("30 روز")),
            category: Some("prepaid".into()),
            extra_benefits: vec![],
            restrictions: vec![],
            availability: Some(json!(true)),
            purchase_url: None,
            unknown_fields: Default::default(),
        }
    }

    #[test]
    fn normalizes_general_data_price_validity_and_identity() {
        let p = IrancellNormalizer::normalize(&raw(), 7).unwrap();
        assert_eq!(p.external_id, "pkg-1");
        assert_eq!(p.operator, Operator::Irancell);
        assert_eq!(p.price.unwrap().amount, 1_200_000);
        assert_eq!(p.validity, Validity::Days(30));
        assert_eq!(p.data_allowances[0].kind, DataAllowanceKind::General);
    }
    #[test]
    fn combined_package_preserves_special_traffic_separately() {
        let mut r = raw();
        r.extra_benefits = vec![json!("100 گیگابایت اینترنت شبانه هدیه")];
        let p = IrancellNormalizer::normalize(&r, 7).unwrap();
        assert_eq!(p.package_kind, PackageKind::Combined);
        assert!(p
            .data_allowances
            .iter()
            .any(|a| a.kind == DataAllowanceKind::Night));
        assert_eq!(p.data_allowances[0].kind, DataAllowanceKind::General);
    }
    #[test]
    fn missing_fields_are_explicit_errors_or_unknowns() {
        let mut r = raw();
        r.id = None;
        assert!(matches!(
            IrancellNormalizer::normalize(&r, 1),
            Err(IrancellNormalizationError::MissingId)
        ));
        let mut r = raw();
        r.validity = None;
        r.availability = None;
        let p = IrancellNormalizer::normalize(&r, 1).unwrap();
        assert_eq!(p.validity, Validity::Unknown);
        assert_eq!(p.availability, Availability::Unknown);
    }
    #[test]
    fn malformed_volume_fails_without_guessing() {
        let mut r = raw();
        r.volume = Some(json!("ده گیگ"));
        assert!(matches!(
            IrancellNormalizer::normalize(&r, 1),
            Err(IrancellNormalizationError::InvalidVolume)
        ));
    }
}
