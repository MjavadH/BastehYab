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
            raw.purchasable_package_id
                .as_deref()
                .ok_or(RightelNormalizationError::MissingId)?,
        )
        .ok_or(RightelNormalizationError::MissingId)?;

        let name_str = raw.name_fa.clone().ok_or(RightelNormalizationError::EmptyName)?;
        let name = clean_text(&name_str).ok_or(RightelNormalizationError::EmptyName)?;
        let price = raw.price.as_ref().map(parse_price).transpose()?;

        let mut data_allowances = Vec::new();
        data_allowances.push(extract_traffic(&name_str)?);

        let validity = extract_validity(&name_str);

        let mut sim_types = Vec::new();
        match raw.product_type.as_deref() {
            Some("PREPAID") => sim_types.push(SimType::Prepaid),
            Some("POSTPAID") => sim_types.push(SimType::Postpaid),
            _ => sim_types.push(SimType::Other),
        }

        let package = InternetPackage {
            id: canonical_package_id(Operator::Rightel, &external_id),
            operator: Operator::Rightel,
            external_id,
            name: name.clone(),
            price,
            validity,
            data_allowances,
            voice: None,
            sms: None,
            sim_types,
            package_kind: PackageKind::InternetOnly,
            availability: Availability::Unknown,
            purchase: PurchaseInfo {
                official_url: Some(rightel_page_url().into()),
                ussd_code: None,
            },
            metadata: PackageMetadata {
                fetched_at_unix_seconds: Some(fetched_at),
                source_url: Some(
                    "https://portal-api.rightel.ir/extra-package/api/v1/extra-package-direct/web-site/purchasable-package"
                        .into(),
                ),
                regulatory_code: None,
                offer_code: raw.offer_code.clone(),
                original_description: Some(name),
            },
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
fn extract_traffic(text: &str) -> Result<DataAllowance, RightelNormalizationError> {
    let t = normalize_digits(text).to_lowercase();

    let (unit, unit_pos) = if let Some(idx) = t.find("گیگ") {
        (DataUnit::Gib, idx)
    } else if let Some(idx) = t.find("gb") {
        (DataUnit::Gib, idx)
    } else if let Some(idx) = t.find("مگ") {
        (DataUnit::Mib, idx)
    } else if let Some(idx) = t.find("mb") {
        (DataUnit::Mib, idx)
    } else if t.contains("نامحدود") || t.contains("unlimited") {
        let mut a = DataAllowance::unlimited(DataAllowanceKind::General);
        a.description = clean_text(text);
        return Ok(a);
    } else {
        let mut a = DataAllowance::unknown(DataAllowanceKind::General);
        a.description = clean_text(text);
        return Ok(a);
    };

    let before_unit = &t[..unit_pos];
    let num_str = before_unit
        .split(|c: char| !(c.is_ascii_digit() || c == '.' || c == ','))
        .filter(|s| !s.is_empty())
        .last()
        .ok_or(RightelNormalizationError::InvalidTraffic)?;

    let mut a = DataAllowance::finite(
        DataAllowanceKind::General,
        decimal_data_bytes(num_str, unit).map_err(|_| RightelNormalizationError::InvalidTraffic)?,
    );
    a.description = clean_text(text);
    Ok(a)
}
fn extract_validity(text: &str) -> Validity {
    let t = normalize_digits(text).to_lowercase();
    let mut keyword_pos = None;
    let mut is_hours = false;

    if let Some(idx) = t.find("روز") {
        keyword_pos = Some(idx);
    } else if let Some(idx) = t.find("day") {
        keyword_pos = Some(idx);
    } else if let Some(idx) = t.find("ساعت") {
        keyword_pos = Some(idx);
        is_hours = true;
    } else if let Some(idx) = t.find("hour") {
        keyword_pos = Some(idx);
        is_hours = true;
    }

    if let Some(pos) = keyword_pos {
        let before_kw = &t[..pos];
        if let Some(num_str) = before_kw
            .split(|c: char| !c.is_ascii_digit())
            .filter(|s| !s.is_empty())
            .last()
        {
            if let Ok(n) = num_str.parse::<u32>() {
                return if is_hours { Validity::Hours(n) } else { Validity::Days(n) };
            }
        }
    }
    Validity::Unknown
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
    use serde_json::{json, Map};

    fn raw(name: &str, prod: &str) -> RawRightelPackage {
        RawRightelPackage {
            purchasable_package_id: Some("123".into()),
            name_fa: Some(name.into()),
            name_en: None,
            price: Some(json!(100000)),
            product_type: Some(prod.into()),
            offer_code: Some("OFF1".into()),
            filters: vec![],
            channel_categories: vec![],
            unknown_fields: Map::new(),
        }
    }

    #[test]
    fn normalizes_rightel_package_correctly() {
        let r = raw("30 روزه 10 گیگابایت", "PREPAID");
        let p = RightelNormalizer::normalize(&r, 1).unwrap();

        assert_eq!(p.operator, Operator::Rightel);
        assert_eq!(p.id.0, "rightel:123");
        assert_eq!(p.validity, Validity::Days(30));
        assert_eq!(p.sim_types, vec![SimType::Prepaid]);
        assert_eq!(p.data_allowances[0].amount_bytes, Some(10 * 1024 * 1024 * 1024)); // 10 GiB
        assert_eq!(p.metadata.offer_code.as_deref(), Some("OFF1"));
    }

    #[test]
    fn handles_megabytes_and_hours() {
        let r = raw("1 روزه 100 مگابایت", "POSTPAID");
        let p = RightelNormalizer::normalize(&r, 1).unwrap();
        assert_eq!(p.validity, Validity::Days(1));
        assert_eq!(p.sim_types, vec![SimType::Postpaid]);
        assert_eq!(p.data_allowances[0].amount_bytes, Some(100 * 1024 * 1024));
    }

    #[test]
    fn missing_identity_is_error() {
        let mut r = raw("1 روزه 100 مگابایت", "PREPAID");
        r.purchasable_package_id = None;
        assert!(matches!(
            RightelNormalizer::normalize(&r, 1),
            Err(RightelNormalizationError::MissingId)
        ));
    }
}