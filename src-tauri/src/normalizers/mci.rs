use crate::{
    collectors::mci::{mci_page_url, RawMCIPackage},
    domain::{
        allowance::{DataAllowance, DataAllowanceKind, SmsAllowance, VoiceAllowance},
        operator::Operator,
        package::{
            Availability, InternetPackage, PackageKind, PackageMetadata, PurchaseInfo, SimType,
            Validity,
        },
    },
    normalizers::{
        canonical_package_id, clean_text, decimal_data_bytes, local_time, money_from_toman,
        normalize_digits, time_window, validate_package, DataUnit, NormalizationError,
    },
};
use serde_json::Value;
use thiserror::Error;
#[derive(Debug, Clone, Copy)]
pub struct MCINormalizer;
#[derive(Debug, Error, PartialEq, Eq)]
pub enum MCINormalizationError {
    #[error("missing or invalid MCI package id")]
    MissingId,
    #[error("empty MCI package name")]
    EmptyName,
    #[error("invalid MCI package price")]
    InvalidPrice,
    #[error("invalid MCI data volume")]
    InvalidVolume,
    #[error("canonical package validation failed: {0}")]
    Validation(#[from] NormalizationError),
}
impl MCINormalizer {
    pub fn normalize(
        raw: &RawMCIPackage,
        fetched_at: i64,
    ) -> Result<InternetPackage, MCINormalizationError> {
        let external_id = clean_text(
            raw.id
                .as_deref()
                .or(raw.ussd_code.as_deref())
                .ok_or(MCINormalizationError::MissingId)?,
        )
        .ok_or(MCINormalizationError::MissingId)?;
        let name = clean_text(
            raw.title
                .as_deref()
                .ok_or(MCINormalizationError::EmptyName)?,
        )
        .ok_or(MCINormalizationError::EmptyName)?;
        let price = raw.price.as_ref().map(parse_price).transpose()?;
        let mut data_allowances = Vec::new();
        if let Some(v) = &raw.volume {
            data_allowances.extend(parse_allowance_text(
                &value_to_text(v).ok_or(MCINormalizationError::InvalidVolume)?,
            )?)
        }
        for v in raw.extra_benefits.iter().chain(raw.restrictions.iter()) {
            if let Some(t) = value_to_text(v) {
                if mentions_data(&t) {
                    data_allowances.extend(parse_allowance_text(&t)?);
                }
            }
        }
        if data_allowances.is_empty() {
            data_allowances.push(DataAllowance::unknown(DataAllowanceKind::Other));
        }
        let has_non_data = raw
            .extra_benefits
            .iter()
            .filter_map(value_to_text)
            .any(|t| mentions_voice(&t) || mentions_sms(&t) || !mentions_data(&t));
        let voice = parse_voice(raw);
        let sms = parse_sms(raw);
        let package = InternetPackage {
            id: canonical_package_id(Operator::Mci, &external_id),
            operator: Operator::Mci,
            external_id,
            name: name.clone(),
            price,
            validity: raw
                .validity
                .as_ref()
                .map(parse_validity)
                .unwrap_or(Validity::Unknown),
            data_allowances,
            voice: voice.clone(),
            sms: sms.clone(),
            sim_types: parse_sim_types(raw.category.as_deref()),
            package_kind: if has_non_data || voice.is_some() || sms.is_some() {
                PackageKind::Combined
            } else {
                PackageKind::InternetOnly
            },
            availability: parse_availability(raw.availability.as_ref()),
            purchase: PurchaseInfo {
                official_url: raw
                    .purchase_url
                    .clone()
                    .or_else(|| Some(mci_page_url().into())),
                ussd_code: raw.ussd_code.clone(),
            },
            metadata: PackageMetadata {
                fetched_at_unix_seconds: Some(fetched_at),
                source_url: Some(mci_page_url().into()),
                regulatory_code: None,
                offer_code: raw.id.clone(),
                original_description: Some(name),
            },
        };
        validate_package(&package)?;
        Ok(package)
    }
}
fn parse_price(v: &Value) -> Result<crate::domain::money::Money, MCINormalizationError> {
    let t = value_to_text(v).ok_or(MCINormalizationError::InvalidPrice)?;
    let n = normalize_digits(&t).replace([',', '٬'], "");
    let digits: String = n.chars().filter(|c| c.is_ascii_digit()).collect();
    let amount: u64 = digits
        .parse()
        .map_err(|_| MCINormalizationError::InvalidPrice)?;
    money_from_toman(amount).map_err(|_| MCINormalizationError::InvalidPrice)
}
fn parse_allowance_text(text: &str) -> Result<Vec<DataAllowance>, MCINormalizationError> {
    if text.contains('+') {
        return text.split('+').map(parse_allowance_part).collect();
    }
    Ok(vec![parse_allowance_part(text)?])
}
fn parse_allowance_part(text: &str) -> Result<DataAllowance, MCINormalizationError> {
    let mut kind = classify_allowance(text);
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
    let num = first_number(&t).ok_or(MCINormalizationError::InvalidVolume)?;
    if t.contains("2am") || t.contains("2 am") || t.contains("شبانه") {
        kind = DataAllowanceKind::Night;
    }
    let mut a = DataAllowance::finite(
        kind,
        decimal_data_bytes(&num, unit).map_err(|_| MCINormalizationError::InvalidVolume)?,
    );
    a.description = clean_text(text);
    if kind == DataAllowanceKind::Night && (t.contains("2am") || t.contains("2 am")) {
        a.time_window = Some(time_window(local_time(2, 0)?, local_time(7, 0)?));
    }
    Ok(a)
}
fn parse_validity(v: &Value) -> Validity {
    let Some(t) = value_to_text(v) else {
        return Validity::Unknown;
    };
    let s = normalize_digits(&t).to_lowercase();
    let Some(n) = first_number(&s).and_then(|x| x.parse::<u32>().ok()) else {
        return Validity::Unknown;
    };
    if s.contains("ساعت") || s.contains("hour") {
        Validity::Hours(n)
    } else if s.contains("ماه") || s.contains("month") {
        Validity::Days(n.saturating_mul(30))
    } else if s.contains("روز") || s.contains("day") {
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
    let mut out = Vec::new();
    if c.contains("pre") || c.contains("اعتباری") {
        out.push(SimType::Prepaid)
    }
    if c.contains("post") || c.contains("دائمی") {
        out.push(SimType::Postpaid)
    }
    if out.is_empty() {
        out.push(SimType::Other)
    }
    out
}
fn parse_availability(v: Option<&Value>) -> Availability {
    match v {
        Some(Value::Bool(true)) => Availability::Available,
        Some(Value::Bool(false)) => Availability::Unavailable,
        Some(x) => value_to_text(x)
            .map(|s| s.to_lowercase())
            .map_or(Availability::Unknown, |s| {
                if s.contains("inactive") || s.contains("غیرفعال") {
                    Availability::Unavailable
                } else if s.contains("active") || s.contains("فعال") {
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
    if t.contains("شب") || t.contains("night") || t.contains("2am") {
        DataAllowanceKind::Night
    } else if t.contains("داخلی") || t.contains("domestic") {
        DataAllowanceKind::Domestic
    } else if t.contains("هدیه") || t.contains("gift") {
        DataAllowanceKind::Gift
    } else {
        DataAllowanceKind::General
    }
}
fn parse_voice(raw: &RawMCIPackage) -> Option<VoiceAllowance> {
    raw.extra_benefits
        .iter()
        .filter_map(value_to_text)
        .find(|t| mentions_voice(t))
        .map(|t| VoiceAllowance {
            minutes: first_number(&normalize_digits(&t)).and_then(|n| n.parse().ok()),
            unlimited: t.contains("نامحدود"),
        })
}
fn parse_sms(raw: &RawMCIPackage) -> Option<SmsAllowance> {
    raw.extra_benefits
        .iter()
        .filter_map(value_to_text)
        .find(|t| mentions_sms(t))
        .map(|t| SmsAllowance {
            count: first_number(&normalize_digits(&t)).and_then(|n| n.parse().ok()),
            unlimited: t.contains("نامحدود"),
        })
}
fn mentions_voice(t: &str) -> bool {
    let l = t.to_lowercase();
    l.contains("دقیقه") || l.contains("مکالمه") || l.contains("voice") || l.contains("minute")
}
fn mentions_sms(t: &str) -> bool {
    let l = t.to_lowercase();
    l.contains("پیامک") || l.contains("sms")
}
fn mentions_data(t: &str) -> bool {
    let l = t.to_lowercase();
    l.contains("gb")
        || l.contains("mb")
        || l.contains("گیگ")
        || l.contains("مگ")
        || l.contains("نامحدود")
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
fn unknown_with_description(kind: DataAllowanceKind, text: &str) -> DataAllowance {
    let mut a = DataAllowance::unknown(kind);
    a.description = clean_text(text);
    a
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::collectors::mci::parse_page;
    #[test]
    fn normalizes_monthly_daily_combined_and_restricted() {
        let c = parse_page(
            include_bytes!("../../tests/fixtures/mci/page1.json"),
            "fixture",
        )
        .unwrap();
        let p = MCINormalizer::normalize(&c.packages[0], 10).unwrap();
        assert_eq!(p.operator, Operator::Mci);
        assert_eq!(p.validity, Validity::Days(1));
        assert!(p
            .data_allowances
            .iter()
            .any(|a| a.kind == DataAllowanceKind::General));
        assert!(p
            .data_allowances
            .iter()
            .any(|a| a.kind == DataAllowanceKind::Night));
        let p2 = MCINormalizer::normalize(&c.packages[1], 10).unwrap();
        assert_eq!(p2.validity, Validity::Days(30));
        assert_eq!(p2.package_kind, PackageKind::Combined);
        assert!(p2.voice.is_some());
    }
}
