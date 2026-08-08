use std::cmp::Ordering;

use crate::domain::{
    allowance::DataAllowanceKind,
    money::Currency,
    package::{Availability, InternetPackage, PackageKind, Validity},
    recommendation::{
        PackageFilters, Recommendation, RecommendationContext, RecommendationMetrics,
        RecommendationReason, RecommendationScore, RecommendationSet, RecommendationStrategy,
        ValueRatio,
    },
};

pub const DEFAULT_RECOMMENDATION_LIMIT: usize = 3;
const MONTHLY_VALIDITY_DAYS: u32 = 30;

pub fn get_recommendations(
    packages: &[InternetPackage],
    context: &RecommendationContext,
) -> Vec<RecommendationSet> {
    [
        RecommendationStrategy::BestValue,
        RecommendationStrategy::HighestVolume,
        RecommendationStrategy::BestMonthly,
        RecommendationStrategy::CheapestUseful,
        RecommendationStrategy::BestNight,
        RecommendationStrategy::BestCombined,
    ]
    .into_iter()
    .map(|strategy| recommend(packages, strategy, context))
    .collect()
}

pub fn get_best_packages(
    packages: &[InternetPackage],
    context: &RecommendationContext,
) -> RecommendationSet {
    recommend(packages, RecommendationStrategy::BestValue, context)
}

pub fn compare_packages<'a>(
    packages: &'a [InternetPackage],
    ids: &[&str],
) -> Vec<&'a InternetPackage> {
    let mut selected = packages
        .iter()
        .filter(|package| ids.iter().any(|id| package.id.0 == *id))
        .collect::<Vec<_>>();
    selected.sort_by(|a, b| stable_identity(a).cmp(&stable_identity(b)));
    selected
}

pub fn recommend(
    packages: &[InternetPackage],
    strategy: RecommendationStrategy,
    context: &RecommendationContext,
) -> RecommendationSet {
    let limit = context.limit.unwrap_or(DEFAULT_RECOMMENDATION_LIMIT);
    let filters = effective_filters(context);
    let filtered = packages
        .iter()
        .filter(|package| matches_filters(package, &filters))
        .collect::<Vec<_>>();
    let filtered_count = filtered.len();
    let mut candidates = filtered
        .into_iter()
        .filter_map(|package| evaluate(package, strategy))
        .collect::<Vec<_>>();
    candidates.sort_by(|a, b| compare_candidates(a, b, strategy));
    let eligible_count = candidates.len();
    let results = candidates
        .into_iter()
        .take(limit)
        .enumerate()
        .map(|(index, candidate)| candidate.into_recommendation(strategy, index + 1))
        .collect();

    RecommendationSet {
        strategy,
        input_count: packages.len(),
        filtered_count,
        eligible_count,
        results,
    }
}

fn effective_filters(context: &RecommendationContext) -> PackageFilters {
    let mut filters = context.filters.clone();
    if let Some(budget) = context.budget {
        filters.max_price = Some(budget);
    }
    if let Some(validity) = context.preferred_validity {
        filters.validity = Some(validity);
    }
    if let Some(required) = context.required_general_data_bytes {
        filters.min_general_data_bytes = Some(required);
    }
    if let Some(include_combined) = context.include_combined {
        filters.include_combined = include_combined;
    }
    filters
}

fn matches_filters(package: &InternetPackage, filters: &PackageFilters) -> bool {
    if !filters.operators.is_empty() && !filters.operators.contains(&package.operator) {
        return false;
    }
    if !filters.sim_types.is_empty()
        && !package
            .sim_types
            .iter()
            .any(|sim| filters.sim_types.contains(sim))
    {
        return false;
    }
    if !filters.package_kinds.is_empty() && !filters.package_kinds.contains(&package.package_kind) {
        return false;
    }
    if !filters.include_combined && package.package_kind == PackageKind::Combined {
        return false;
    }
    if let Some(validity) = filters.validity {
        if package.validity != validity {
            return false;
        }
    }
    if let Some(price) = package.price {
        if price.currency != Currency::Irr {
            return false;
        }
        if filters
            .min_price
            .is_some_and(|min| price.amount < min.amount)
            || filters
                .max_price
                .is_some_and(|max| price.amount > max.amount)
        {
            return false;
        }
    } else if filters.min_price.is_some() || filters.max_price.is_some() {
        return false;
    }
    let general = finite_data(package, DataAllowanceKind::General);
    if filters
        .min_general_data_bytes
        .is_some_and(|min| general.is_none_or(|bytes| bytes < min))
        || filters
            .max_general_data_bytes
            .is_some_and(|max| general.is_some_and(|bytes| bytes > max))
    {
        return false;
    }
    if filters.general_internet_only
        && package
            .data_allowances
            .iter()
            .any(|a| a.kind != DataAllowanceKind::General)
    {
        return false;
    }
    true
}

#[derive(Clone)]
struct Candidate<'a> {
    package: &'a InternetPackage,
    metrics: RecommendationMetrics,
    score: RecommendationScore,
    reasons: Vec<RecommendationReason>,
}

impl Candidate<'_> {
    fn into_recommendation(self, strategy: RecommendationStrategy, rank: usize) -> Recommendation {
        Recommendation {
            strategy,
            package_id: self.package.id.clone(),
            rank,
            score: self.score,
            metrics: self.metrics,
            reasons: self.reasons,
        }
    }
}

fn evaluate(package: &InternetPackage, strategy: RecommendationStrategy) -> Option<Candidate<'_>> {
    if package.availability == Availability::Unavailable {
        return None;
    }
    match strategy {
        RecommendationStrategy::BestValue => evaluate_value(package),
        RecommendationStrategy::HighestVolume => evaluate_highest_volume(package),
        RecommendationStrategy::BestMonthly => (package.validity
            == Validity::Days(MONTHLY_VALIDITY_DAYS))
        .then(|| evaluate_highest_volume(package))?
        .map(|mut c| {
            c.reasons.insert(0, RecommendationReason::BestMonthlyOption);
            c
        }),
        RecommendationStrategy::CheapestUseful => evaluate_cheapest(package),
        RecommendationStrategy::BestNight => evaluate_night(package),
        RecommendationStrategy::BestCombined => (package.package_kind == PackageKind::Combined)
            .then(|| evaluate_value(package).or_else(|| evaluate_highest_volume(package)))?,
    }
}

fn evaluate_value(package: &InternetPackage) -> Option<Candidate<'_>> {
    let price = package.price?.amount;
    let general = finite_data(package, DataAllowanceKind::General)?;
    if general == 0 {
        return None;
    }
    let mut metrics = metrics(package);
    metrics.value_ratio = (price > 0).then_some(ValueRatio {
        price_irr: price,
        data_bytes: general,
    });
    Some(Candidate {
        package,
        metrics,
        score: if price == 0 {
            RecommendationScore::FreeGeneralData
        } else {
            RecommendationScore::Ratio {
                numerator: general as u128,
                denominator: price as u128,
            }
        },
        reasons: base_reasons(
            package,
            RecommendationReason::BestValueRatio,
            DataAllowanceKind::General,
        ),
    })
}

fn evaluate_highest_volume(package: &InternetPackage) -> Option<Candidate<'_>> {
    if has_unlimited(package, DataAllowanceKind::General) {
        return Some(Candidate {
            package,
            metrics: metrics(package),
            score: RecommendationScore::UnlimitedGeneralData,
            reasons: vec![
                RecommendationReason::HighestGeneralData,
                RecommendationReason::UnlimitedGeneralData,
            ],
        });
    }
    let general = finite_data(package, DataAllowanceKind::General)?;
    (general > 0).then(|| Candidate {
        package,
        metrics: metrics(package),
        score: RecommendationScore::Bytes(general),
        reasons: base_reasons(
            package,
            RecommendationReason::HighestGeneralData,
            DataAllowanceKind::General,
        ),
    })
}

fn evaluate_cheapest(package: &InternetPackage) -> Option<Candidate<'_>> {
    let price = package.price?.amount;
    let general = finite_data(package, DataAllowanceKind::General).unwrap_or(0);
    if general == 0 || has_only_restricted_data(package) {
        return None;
    }
    Some(Candidate {
        package,
        metrics: metrics(package),
        score: RecommendationScore::Price(price),
        reasons: base_reasons(
            package,
            RecommendationReason::CheapestUsefulOption,
            DataAllowanceKind::General,
        ),
    })
}

fn evaluate_night(package: &InternetPackage) -> Option<Candidate<'_>> {
    if has_unlimited(package, DataAllowanceKind::Night) {
        return Some(Candidate {
            package,
            metrics: metrics(package),
            score: RecommendationScore::UnlimitedNightData,
            reasons: vec![
                RecommendationReason::BestNightTraffic,
                RecommendationReason::UnlimitedNightData,
            ],
        });
    }
    let night = finite_data(package, DataAllowanceKind::Night)?;
    (night > 0).then(|| Candidate {
        package,
        metrics: metrics(package),
        score: RecommendationScore::Bytes(night),
        reasons: base_reasons(
            package,
            RecommendationReason::BestNightTraffic,
            DataAllowanceKind::Night,
        ),
    })
}

fn compare_candidates(
    a: &Candidate<'_>,
    b: &Candidate<'_>,
    strategy: RecommendationStrategy,
) -> Ordering {
    match strategy {
        RecommendationStrategy::BestValue | RecommendationStrategy::BestCombined => {
            compare_value(a, b)
        }
        RecommendationStrategy::HighestVolume | RecommendationStrategy::BestMonthly => {
            compare_volume(a, b, DataAllowanceKind::General)
        }
        RecommendationStrategy::CheapestUseful => compare_cheapest(a, b),
        RecommendationStrategy::BestNight => compare_volume(a, b, DataAllowanceKind::Night),
    }
}

fn compare_value(a: &Candidate<'_>, b: &Candidate<'_>) -> Ordering {
    match (&a.score, &b.score) {
        (RecommendationScore::FreeGeneralData, RecommendationScore::FreeGeneralData) => {
            Ordering::Equal
        }
        (RecommendationScore::FreeGeneralData, _) => Ordering::Less,
        (_, RecommendationScore::FreeGeneralData) => Ordering::Greater,
        (
            RecommendationScore::Ratio {
                numerator: ad,
                denominator: ap,
            },
            RecommendationScore::Ratio {
                numerator: bd,
                denominator: bp,
            },
        ) => (bd * ap).cmp(&(ad * bp)),
        _ => Ordering::Equal,
    }
    .then_with(|| {
        finite_data(b.package, DataAllowanceKind::General)
            .cmp(&finite_data(a.package, DataAllowanceKind::General))
    })
    .then_with(|| price(a.package).cmp(&price(b.package)))
    .then_with(|| validity_days(b.package).cmp(&validity_days(a.package)))
    .then_with(|| stable_identity(a.package).cmp(&stable_identity(b.package)))
}

fn compare_volume(a: &Candidate<'_>, b: &Candidate<'_>, kind: DataAllowanceKind) -> Ordering {
    let unlimited = match (&a.score, &b.score) {
        (
            RecommendationScore::UnlimitedGeneralData | RecommendationScore::UnlimitedNightData,
            RecommendationScore::UnlimitedGeneralData | RecommendationScore::UnlimitedNightData,
        ) => Ordering::Equal,
        (
            RecommendationScore::UnlimitedGeneralData | RecommendationScore::UnlimitedNightData,
            _,
        ) => Ordering::Less,
        (
            _,
            RecommendationScore::UnlimitedGeneralData | RecommendationScore::UnlimitedNightData,
        ) => Ordering::Greater,
        _ => Ordering::Equal,
    };
    unlimited
        .then_with(|| finite_data(b.package, kind).cmp(&finite_data(a.package, kind)))
        .then_with(|| price(a.package).cmp(&price(b.package)))
        .then_with(|| compare_ratio_optional(a.package, b.package))
        .then_with(|| validity_days(b.package).cmp(&validity_days(a.package)))
        .then_with(|| stable_identity(a.package).cmp(&stable_identity(b.package)))
}

fn compare_cheapest(a: &Candidate<'_>, b: &Candidate<'_>) -> Ordering {
    price(a.package)
        .cmp(&price(b.package))
        .then_with(|| {
            finite_data(b.package, DataAllowanceKind::General)
                .cmp(&finite_data(a.package, DataAllowanceKind::General))
        })
        .then_with(|| validity_days(b.package).cmp(&validity_days(a.package)))
        .then_with(|| compare_ratio_optional(a.package, b.package))
        .then_with(|| stable_identity(a.package).cmp(&stable_identity(b.package)))
}

fn compare_ratio_optional(a: &InternetPackage, b: &InternetPackage) -> Ordering {
    match (
        price(a),
        finite_data(a, DataAllowanceKind::General),
        price(b),
        finite_data(b, DataAllowanceKind::General),
    ) {
        (Some(ap), Some(ad), Some(bp), Some(bd)) if ad > 0 && bd > 0 && ap > 0 && bp > 0 => {
            (bd as u128 * ap as u128).cmp(&(ad as u128 * bp as u128))
        }
        (Some(_), Some(_), None, _) | (Some(_), Some(_), _, None) => Ordering::Less,
        (None, _, Some(_), Some(_)) | (_, None, Some(_), Some(_)) => Ordering::Greater,
        _ => Ordering::Equal,
    }
}

fn metrics(package: &InternetPackage) -> RecommendationMetrics {
    RecommendationMetrics {
        price_irr: price(package),
        general_data_bytes: finite_data(package, DataAllowanceKind::General),
        night_data_bytes: finite_data(package, DataAllowanceKind::Night),
        has_unlimited_general_data: has_unlimited(package, DataAllowanceKind::General),
        has_unlimited_night_data: has_unlimited(package, DataAllowanceKind::Night),
        validity_days: validity_days(package),
        package_kind: Some(package.package_kind),
        has_voice: package.voice.is_some(),
        has_sms: package.sms.is_some(),
        value_ratio: None,
        traffic_kind: None,
    }
}

fn base_reasons(
    package: &InternetPackage,
    main: RecommendationReason,
    kind: DataAllowanceKind,
) -> Vec<RecommendationReason> {
    let mut reasons = vec![main];
    if let Some(bytes) = finite_data(package, kind) {
        reasons.push(match kind {
            DataAllowanceKind::Night => RecommendationReason::NightDataBytes { bytes },
            _ => RecommendationReason::GeneralDataBytes { bytes },
        });
    }
    if let Some(amount) = price(package) {
        reasons.push(RecommendationReason::PriceIrr { amount });
    }
    if let Some(days) = validity_days(package) {
        reasons.push(RecommendationReason::ValidityDays { days });
    }
    if package.package_kind == PackageKind::Combined {
        reasons.push(RecommendationReason::CombinedPackage);
    }
    if package.voice.is_some() {
        reasons.push(RecommendationReason::IncludesVoice);
    }
    if package.sms.is_some() {
        reasons.push(RecommendationReason::IncludesSms);
    }
    reasons
}

fn finite_data(package: &InternetPackage, kind: DataAllowanceKind) -> Option<u64> {
    package
        .data_allowances
        .iter()
        .filter(|a| a.kind == kind && !a.unlimited)
        .try_fold(0u64, |acc, a| {
            a.amount_bytes.map(|bytes| acc.saturating_add(bytes))
        })
}

fn has_unlimited(package: &InternetPackage, kind: DataAllowanceKind) -> bool {
    package
        .data_allowances
        .iter()
        .any(|a| a.kind == kind && a.unlimited)
}

fn has_only_restricted_data(package: &InternetPackage) -> bool {
    !package.data_allowances.iter().any(|a| {
        a.kind == DataAllowanceKind::General
            && (a.unlimited || a.amount_bytes.is_some_and(|bytes| bytes > 0))
    })
}

fn price(package: &InternetPackage) -> Option<u64> {
    package
        .price
        .filter(|m| m.currency == Currency::Irr)
        .map(|m| m.amount)
}
fn validity_days(package: &InternetPackage) -> Option<u32> {
    if let Validity::Days(days) = package.validity {
        Some(days)
    } else {
        None
    }
}
fn stable_identity(package: &InternetPackage) -> (&str, &str) {
    (package.operator.as_str(), package.id.0.as_str())
}

#[cfg(test)]
mod tests;
