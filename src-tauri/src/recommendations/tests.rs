use crate::domain::{
    allowance::{DataAllowance, DataAllowanceKind, SmsAllowance, VoiceAllowance},
    money::Money,
    operator::Operator,
    package::{
        Availability, InternetPackage, PackageId, PackageKind, PackageMetadata, PurchaseInfo,
        SimType, Validity,
    },
    recommendation::{
        PackageFilters, RecommendationContext, RecommendationReason, RecommendationStrategy,
    },
};

use super::{get_recommendations, recommend};

const GB: u64 = 1_000_000_000;

fn pkg(
    id: &str,
    operator: Operator,
    price: Option<u64>,
    days: Option<u32>,
    allowances: Vec<DataAllowance>,
) -> InternetPackage {
    InternetPackage {
        id: PackageId::canonical(operator, id),
        operator,
        external_id: id.to_string(),
        name: id.to_string(),
        price: price.map(Money::irr),
        validity: days.map(Validity::Days).unwrap_or(Validity::Unknown),
        data_allowances: allowances,
        voice: None,
        sms: None,
        sim_types: vec![SimType::Prepaid],
        package_kind: PackageKind::InternetOnly,
        availability: Availability::Available,
        purchase: PurchaseInfo::default(),
        metadata: PackageMetadata::default(),
    }
}

fn general(gb: u64) -> DataAllowance {
    DataAllowance::finite(DataAllowanceKind::General, gb * GB)
}
fn night(gb: u64) -> DataAllowance {
    DataAllowance::finite(DataAllowanceKind::Night, gb * GB)
}
fn domestic(gb: u64) -> DataAllowance {
    DataAllowance::finite(DataAllowanceKind::Domestic, gb * GB)
}
fn ctx(limit: usize) -> RecommendationContext {
    RecommendationContext {
        limit: Some(limit),
        filters: PackageFilters {
            include_combined: true,
            ..Default::default()
        },
        ..Default::default()
    }
}

#[test]
fn best_value_uses_exact_general_data_ratio() {
    let packages = vec![
        pkg(
            "a",
            Operator::Mci,
            Some(100_000),
            Some(30),
            vec![general(10)],
        ),
        pkg(
            "b",
            Operator::Irancell,
            Some(160_000),
            Some(30),
            vec![general(20)],
        ),
        pkg(
            "c",
            Operator::Rightel,
            Some(300_000),
            Some(30),
            vec![general(30)],
        ),
    ];
    let set = recommend(&packages, RecommendationStrategy::BestValue, &ctx(3));
    assert_eq!(set.results[0].package_id, packages[1].id);
    assert_eq!(set.results[0].rank, 1);
    assert!(set.results[0]
        .reasons
        .contains(&RecommendationReason::BestValueRatio));
}

#[test]
fn best_value_ignores_restricted_traffic_and_unknown_price() {
    let packages = vec![
        pkg(
            "restricted",
            Operator::Mci,
            Some(100_000),
            Some(30),
            vec![general(10), night(100)],
        ),
        pkg(
            "general",
            Operator::Irancell,
            Some(150_000),
            Some(30),
            vec![general(20)],
        ),
        pkg(
            "unknown-price",
            Operator::Rightel,
            None,
            Some(30),
            vec![general(100)],
        ),
    ];
    let set = recommend(&packages, RecommendationStrategy::BestValue, &ctx(3));
    assert_eq!(set.results[0].package_id, packages[1].id);
    assert_eq!(set.eligible_count, 2);
}

#[test]
fn highest_volume_separates_validity_and_unlimited() {
    let packages = vec![
        pkg("daily", Operator::Mci, Some(10), Some(1), vec![general(50)]),
        pkg(
            "weekly",
            Operator::Irancell,
            Some(10),
            Some(7),
            vec![general(40)],
        ),
        pkg(
            "unlimited",
            Operator::Rightel,
            Some(10),
            Some(30),
            vec![DataAllowance::unlimited(DataAllowanceKind::General)],
        ),
        pkg(
            "unknown",
            Operator::Samantel,
            Some(10),
            Some(30),
            vec![DataAllowance::unknown(DataAllowanceKind::General)],
        ),
        pkg(
            "domestic",
            Operator::Mci,
            Some(10),
            Some(30),
            vec![general(20), domestic(100)],
        ),
    ];
    let set = recommend(&packages, RecommendationStrategy::HighestVolume, &ctx(5));
    assert_eq!(set.results[0].package_id, packages[2].id);
    assert_eq!(set.results[1].package_id, packages[0].id);
    assert_eq!(set.eligible_count, 4);
}

#[test]
fn best_monthly_requires_exact_thirty_days() {
    let packages = vec![
        pkg("29", Operator::Mci, Some(10), Some(29), vec![general(100)]),
        pkg(
            "30a",
            Operator::Irancell,
            Some(100),
            Some(30),
            vec![general(20)],
        ),
        pkg(
            "30b",
            Operator::Rightel,
            Some(90),
            Some(30),
            vec![general(20)],
        ),
        pkg(
            "31",
            Operator::Samantel,
            Some(10),
            Some(31),
            vec![general(100)],
        ),
    ];
    let set = recommend(&packages, RecommendationStrategy::BestMonthly, &ctx(2));
    assert_eq!(set.eligible_count, 2);
    assert_eq!(set.results[0].package_id, packages[2].id);
    assert!(set.results[0]
        .reasons
        .contains(&RecommendationReason::BestMonthlyOption));
}

#[test]
fn cheapest_useful_filters_price_and_data() {
    let packages = vec![
        pkg(
            "zero-data",
            Operator::Mci,
            Some(1),
            Some(1),
            vec![general(0)],
        ),
        pkg(
            "cheap",
            Operator::Irancell,
            Some(50),
            Some(1),
            vec![general(1)],
        ),
        pkg(
            "unknown-price",
            Operator::Rightel,
            None,
            Some(1),
            vec![general(10)],
        ),
        pkg(
            "night-only",
            Operator::Samantel,
            Some(1),
            Some(1),
            vec![night(100)],
        ),
    ];
    let set = recommend(&packages, RecommendationStrategy::CheapestUseful, &ctx(3));
    assert_eq!(set.eligible_count, 1);
    assert_eq!(set.results[0].package_id, packages[1].id);
}

#[test]
fn best_night_uses_night_traffic_only() {
    let packages = vec![
        pkg(
            "general",
            Operator::Mci,
            Some(10),
            Some(30),
            vec![general(100)],
        ),
        pkg(
            "night",
            Operator::Irancell,
            Some(10),
            Some(30),
            vec![general(1), night(20)],
        ),
        pkg(
            "more-night",
            Operator::Rightel,
            Some(20),
            Some(30),
            vec![night(30)],
        ),
    ];
    let set = recommend(&packages, RecommendationStrategy::BestNight, &ctx(2));
    assert_eq!(set.results[0].package_id, packages[2].id);
    assert!(set.results[0]
        .reasons
        .contains(&RecommendationReason::BestNightTraffic));
}

#[test]
fn combined_packages_are_ranked_by_internet_and_acknowledged() {
    let mut a = pkg(
        "combined",
        Operator::Mci,
        Some(150),
        Some(30),
        vec![general(20)],
    );
    a.package_kind = PackageKind::Combined;
    a.voice = Some(VoiceAllowance {
        minutes: Some(100),
        unlimited: false,
    });
    a.sms = Some(SmsAllowance {
        count: Some(50),
        unlimited: false,
    });
    let b = pkg(
        "internet",
        Operator::Irancell,
        Some(160),
        Some(30),
        vec![general(20)],
    );
    let set = recommend(
        &[a.clone(), b],
        RecommendationStrategy::BestCombined,
        &ctx(3),
    );
    assert_eq!(set.results[0].package_id, a.id);
    assert!(set.results[0]
        .reasons
        .contains(&RecommendationReason::CombinedPackage));
    assert!(set.results[0]
        .reasons
        .contains(&RecommendationReason::IncludesVoice));
    assert!(set.results[0]
        .reasons
        .contains(&RecommendationReason::IncludesSms));
}

#[test]
fn filters_are_applied_before_ranking() {
    let context = RecommendationContext {
        filters: PackageFilters {
            operators: vec![Operator::Rightel],
            max_price: Some(Money::irr(200)),
            validity: Some(Validity::Days(30)),
            include_combined: true,
            ..Default::default()
        },
        ..ctx(3)
    };
    let packages = vec![
        pkg(
            "best-global",
            Operator::Mci,
            Some(10),
            Some(30),
            vec![general(100)],
        ),
        pkg(
            "rightel",
            Operator::Rightel,
            Some(100),
            Some(30),
            vec![general(10)],
        ),
        pkg(
            "too-expensive",
            Operator::Rightel,
            Some(300),
            Some(30),
            vec![general(100)],
        ),
    ];
    let set = recommend(&packages, RecommendationStrategy::BestValue, &context);
    assert_eq!(set.filtered_count, 1);
    assert_eq!(set.results[0].package_id, packages[1].id);
}

#[test]
fn deterministic_tie_breaking_is_input_order_independent() {
    let a = pkg(
        "a",
        Operator::Mci,
        Some(10),
        Some(30),
        vec![DataAllowance::finite(DataAllowanceKind::General, 3)],
    );
    let b = pkg(
        "b",
        Operator::Irancell,
        Some(20),
        Some(30),
        vec![DataAllowance::finite(DataAllowanceKind::General, 6)],
    );
    let c = pkg(
        "c",
        Operator::Rightel,
        Some(30),
        Some(30),
        vec![DataAllowance::finite(DataAllowanceKind::General, 9)],
    );
    let ids1: Vec<_> = recommend(
        &[a.clone(), b.clone(), c.clone()],
        RecommendationStrategy::BestValue,
        &ctx(3),
    )
    .results
    .into_iter()
    .map(|r| r.package_id)
    .collect();
    let ids2: Vec<_> = recommend(&[c, a, b], RecommendationStrategy::BestValue, &ctx(3))
        .results
        .into_iter()
        .map(|r| r.package_id)
        .collect();
    assert_eq!(ids1, ids2);
}

#[test]
fn application_level_recommendations_return_all_strategy_sets() {
    let packages = vec![pkg(
        "a",
        Operator::Mci,
        Some(10),
        Some(30),
        vec![general(10), night(5)],
    )];
    let sets = get_recommendations(&packages, &ctx(1));
    assert_eq!(sets.len(), 6);
    assert!(sets
        .iter()
        .any(|set| set.strategy == RecommendationStrategy::BestValue));
}
