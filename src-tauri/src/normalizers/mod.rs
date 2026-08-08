//! Operator-independent normalization helpers and validation for canonical packages.

use thiserror::Error;

use crate::domain::{
    allowance::{DataAllowance, LocalTime, TimeWindow},
    money::Money,
    operator::Operator,
    package::{InternetPackage, PackageId},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataUnit {
    Bytes,
    Kib,
    Mib,
    Gib,
    Tib,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum NormalizationError {
    #[error("numeric conversion overflowed")]
    Overflow,
    #[error("invalid decimal quantity")]
    InvalidDecimal,
    #[error("invalid local time {hour:02}:{minute:02}")]
    InvalidLocalTime { hour: u8, minute: u8 },
    #[error("package id must equal canonical operator/external id")]
    NonCanonicalPackageId,
    #[error("external id is required")]
    MissingExternalId,
    #[error("package name is required")]
    MissingName,
    #[error("at least one data allowance is required")]
    MissingDataAllowance,
    #[error("allowance cannot be both finite and unlimited")]
    FiniteAndUnlimitedAllowance,
}

pub fn money_from_irr(amount: u64) -> Money {
    Money::irr(amount)
}

pub fn money_from_toman(amount: u64) -> Result<Money, NormalizationError> {
    amount
        .checked_mul(10)
        .map(Money::irr)
        .ok_or(NormalizationError::Overflow)
}

pub fn data_bytes(amount: u64, unit: DataUnit) -> Result<u64, NormalizationError> {
    let multiplier = match unit {
        DataUnit::Bytes => 1,
        DataUnit::Kib => 1024,
        DataUnit::Mib => 1024_u64.pow(2),
        DataUnit::Gib => 1024_u64.pow(3),
        DataUnit::Tib => 1024_u64.pow(4),
    };
    amount
        .checked_mul(multiplier)
        .ok_or(NormalizationError::Overflow)
}

pub fn decimal_data_bytes(input: &str, unit: DataUnit) -> Result<u64, NormalizationError> {
    let normalized = normalize_digits(input).replace(',', ".");
    let (whole, frac) = normalized
        .split_once('.')
        .unwrap_or((normalized.as_str(), ""));
    if whole.is_empty()
        || !whole.chars().all(|c| c.is_ascii_digit())
        || !frac.chars().all(|c| c.is_ascii_digit())
    {
        return Err(NormalizationError::InvalidDecimal);
    }
    let whole = whole
        .parse::<u64>()
        .map_err(|_| NormalizationError::InvalidDecimal)?;
    let multiplier = data_bytes(1, unit)?;
    let whole_bytes = whole
        .checked_mul(multiplier)
        .ok_or(NormalizationError::Overflow)?;
    if frac.is_empty() {
        return Ok(whole_bytes);
    }
    let scale = 10_u64
        .checked_pow(frac.len() as u32)
        .ok_or(NormalizationError::Overflow)?;
    let frac = frac
        .parse::<u64>()
        .map_err(|_| NormalizationError::InvalidDecimal)?;
    let frac_bytes = frac
        .checked_mul(multiplier)
        .ok_or(NormalizationError::Overflow)?
        / scale;
    whole_bytes
        .checked_add(frac_bytes)
        .ok_or(NormalizationError::Overflow)
}

pub fn canonical_package_id(operator: Operator, external_id: &str) -> PackageId {
    PackageId::canonical(operator, external_id)
}

pub fn clean_text(input: &str) -> Option<String> {
    let normalized = input
        .replace('\u{200c}', " ")
        .replace(['\n', '\t', '\r', '\u{a0}'], " ");
    let collapsed = normalized.split_whitespace().collect::<Vec<_>>().join(" ");
    (!collapsed.is_empty()).then_some(collapsed)
}

pub fn normalize_digits(input: &str) -> String {
    input
        .chars()
        .map(|c| match c {
            '۰' | '٠' => '0',
            '۱' | '١' => '1',
            '۲' | '٢' => '2',
            '۳' | '٣' => '3',
            '۴' | '٤' => '4',
            '۵' | '٥' => '5',
            '۶' | '٦' => '6',
            '۷' | '٧' => '7',
            '۸' | '٨' => '8',
            '۹' | '٩' => '9',
            _ => c,
        })
        .collect()
}

pub fn local_time(hour: u8, minute: u8) -> Result<LocalTime, NormalizationError> {
    if hour <= 23 && minute <= 59 {
        Ok(LocalTime { hour, minute })
    } else {
        Err(NormalizationError::InvalidLocalTime { hour, minute })
    }
}

pub fn time_window(start: LocalTime, end: LocalTime) -> TimeWindow {
    TimeWindow { start, end }
}

pub fn validate_package(package: &InternetPackage) -> Result<(), NormalizationError> {
    if package.external_id.trim().is_empty() {
        return Err(NormalizationError::MissingExternalId);
    }
    if package.name.trim().is_empty() {
        return Err(NormalizationError::MissingName);
    }
    if package.id != PackageId::canonical(package.operator, &package.external_id) {
        return Err(NormalizationError::NonCanonicalPackageId);
    }
    if package.data_allowances.is_empty() {
        return Err(NormalizationError::MissingDataAllowance);
    }
    for allowance in &package.data_allowances {
        validate_allowance(allowance)?;
    }
    Ok(())
}

fn validate_allowance(allowance: &DataAllowance) -> Result<(), NormalizationError> {
    if allowance.amount_bytes.is_some() && allowance.unlimited {
        Err(NormalizationError::FiniteAndUnlimitedAllowance)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{allowance::*, package::*};

    fn fixture() -> InternetPackage {
        InternetPackage {
            id: canonical_package_id(Operator::Irancell, "42"),
            operator: Operator::Irancell,
            external_id: "42".into(),
            name: "package".into(),
            price: Some(money_from_toman(150_000).unwrap()),
            validity: Validity::Days(30),
            data_allowances: vec![DataAllowance::finite(
                DataAllowanceKind::General,
                data_bytes(10, DataUnit::Gib).unwrap(),
            )],
            voice: None,
            sms: None,
            sim_types: vec![SimType::Prepaid],
            package_kind: PackageKind::InternetOnly,
            availability: Availability::Available,
            purchase: PurchaseInfo::default(),
            metadata: PackageMetadata::default(),
        }
    }

    #[test]
    fn money_conversion_uses_irr() {
        assert_eq!(money_from_toman(150_000).unwrap(), Money::irr(1_500_000));
        assert_eq!(money_from_irr(1_500_000), Money::irr(1_500_000));
    }
    #[test]
    fn finite_data_conversion_uses_binary_units() {
        assert_eq!(data_bytes(1, DataUnit::Gib).unwrap(), 1_073_741_824);
        assert_eq!(
            decimal_data_bytes("۱.۵", DataUnit::Gib).unwrap(),
            1_610_612_736
        );
    }
    #[test]
    fn unknown_and_unlimited_are_distinct() {
        assert_ne!(
            DataAllowance::unknown(DataAllowanceKind::General),
            DataAllowance::unlimited(DataAllowanceKind::General)
        );
    }
    #[test]
    fn restricted_allowances_stay_separate() {
        let p = InternetPackage {
            data_allowances: vec![
                DataAllowance::finite(DataAllowanceKind::General, 10),
                DataAllowance::finite(DataAllowanceKind::Night, 10),
                DataAllowance::finite(DataAllowanceKind::Domestic, 10),
                DataAllowance::unlimited(DataAllowanceKind::Night),
            ],
            ..fixture()
        };
        assert_eq!(p.data_allowances.len(), 4);
        assert_eq!(p.data_allowances[1].kind, DataAllowanceKind::Night);
        assert!(p.data_allowances[3].unlimited);
    }
    #[test]
    fn app_specific_gift_and_time_window_are_representable() {
        let mut a = DataAllowance::finite(DataAllowanceKind::ApplicationSpecific, 100);
        a.description = Some("video".into());
        a.time_window = Some(time_window(
            local_time(1, 0).unwrap(),
            local_time(7, 0).unwrap(),
        ));
        let gift = DataAllowance::unknown(DataAllowanceKind::Gift);
        assert_eq!(a.kind, DataAllowanceKind::ApplicationSpecific);
        assert_eq!(gift.amount_bytes, None);
    }
    #[test]
    fn combined_voice_sms_package_is_valid() {
        let p = InternetPackage {
            voice: Some(VoiceAllowance {
                minutes: Some(100),
                unlimited: false,
            }),
            sms: Some(SmsAllowance {
                count: None,
                unlimited: true,
            }),
            package_kind: PackageKind::Combined,
            ..fixture()
        };
        assert!(validate_package(&p).is_ok());
    }
    #[test]
    fn validity_preserves_unknown_and_days() {
        assert_ne!(Validity::Unknown, Validity::Days(30));
        assert_eq!(Validity::Hours(24), Validity::Hours(24));
    }
    #[test]
    fn serialization_round_trip_is_stable() {
        let p = fixture();
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains("dataAllowances"));
        assert_eq!(serde_json::from_str::<InternetPackage>(&json).unwrap(), p);
    }
    #[test]
    fn invalid_domain_state_is_rejected() {
        let mut p = fixture();
        p.data_allowances[0].unlimited = true;
        assert_eq!(
            validate_package(&p),
            Err(NormalizationError::FiniteAndUnlimitedAllowance)
        );
        p = fixture();
        p.id = PackageId("wrong".into());
        assert_eq!(
            validate_package(&p),
            Err(NormalizationError::NonCanonicalPackageId)
        );
    }
    #[test]
    fn stable_package_identity_uses_operator_and_external_id() {
        assert_eq!(
            canonical_package_id(Operator::Rightel, "328"),
            PackageId("rightel:328".into())
        );
        assert_ne!(
            canonical_package_id(Operator::Mci, "328"),
            canonical_package_id(Operator::Rightel, "328")
        );
    }
}
