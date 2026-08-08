use super::*;
use crate::domain::{
    allowance::{DataAllowance, DataAllowanceKind},
    money::Money,
    package::{Availability, PackageId, PackageMetadata, PurchaseInfo},
};

const GIB: u64 = 1024 * 1024 * 1024;

fn pkg(
    id: &str,
    operator: Operator,
    name: &str,
    price: Option<u64>,
    general_gib: Option<u64>,
) -> InternetPackage {
    InternetPackage {
        id: PackageId::canonical(operator, id),
        operator,
        external_id: id.to_string(),
        name: name.to_string(),
        price: price.map(Money::irr),
        validity: Validity::Days(30),
        data_allowances: vec![match general_gib {
            Some(gib) => DataAllowance::finite(DataAllowanceKind::General, gib * GIB),
            None => DataAllowance::unknown(DataAllowanceKind::General),
        }],
        voice: None,
        sms: None,
        sim_types: vec![SimType::Prepaid],
        package_kind: PackageKind::InternetOnly,
        availability: Availability::Available,
        purchase: PurchaseInfo::default(),
        metadata: PackageMetadata::default(),
    }
}

fn combined(mut package: InternetPackage) -> InternetPackage {
    package.package_kind = PackageKind::Combined;
    package
}
fn validity(mut package: InternetPackage, validity: Validity) -> InternetPackage {
    package.validity = validity;
    package
}
fn desc(mut package: InternetPackage, text: &str) -> InternetPackage {
    package.metadata.original_description = Some(text.to_string());
    package
}
fn fetched(mut package: InternetPackage, fetched_at: i64) -> InternetPackage {
    package.metadata.fetched_at_unix_seconds = Some(fetched_at);
    package
}
fn with_allowance(mut package: InternetPackage, allowance: DataAllowance) -> InternetPackage {
    package.data_allowances.push(allowance);
    package
}

#[test]
fn filters_by_operator_without_operator_specific_logic() {
    let packages = vec![
        pkg("m", Operator::Mci, "MCI", Some(10), Some(1)),
        pkg("i", Operator::Irancell, "Irancell", Some(10), Some(1)),
    ];
    let query = PackageQuery {
        filter: PackageFilter {
            operators: vec![Operator::Irancell],
            ..Default::default()
        },
        ..Default::default()
    };
    assert_eq!(
        query_packages(&packages, &query)
            .into_iter()
            .map(|p| p.operator)
            .collect::<Vec<_>>(),
        vec![Operator::Irancell]
    );
}

#[test]
fn filters_price_range_and_excludes_unknown_prices_when_price_is_required() {
    let packages = vec![
        pkg("cheap", Operator::Mci, "cheap", Some(50), Some(1)),
        pkg("ok", Operator::Mci, "ok", Some(100), Some(1)),
        pkg("unknown", Operator::Mci, "unknown", None, Some(1)),
    ];
    let query = PackageQuery {
        filter: PackageFilter {
            min_price: Some(Money::irr(80)),
            max_price: Some(Money::irr(120)),
            ..Default::default()
        },
        ..Default::default()
    };
    assert_eq!(ids(query_packages(&packages, &query)), vec!["mci:ok"]);
}

#[test]
fn filters_general_data_without_counting_restricted_traffic() {
    let restricted = with_allowance(
        pkg(
            "restricted",
            Operator::Mci,
            "restricted",
            Some(100),
            Some(10),
        ),
        DataAllowance::finite(DataAllowanceKind::Night, 10 * GIB),
    );
    let general = pkg("general", Operator::Mci, "general", Some(100), Some(20));
    let query = PackageQuery {
        filter: PackageFilter {
            min_general_data_bytes: Some(20 * GIB),
            ..Default::default()
        },
        ..Default::default()
    };
    assert_eq!(
        ids(query_packages(&[restricted, general], &query)),
        vec!["mci:general"]
    );
}

#[test]
fn total_usable_data_filter_counts_restricted_only_when_explicit() {
    let package = with_allowance(
        pkg("total", Operator::Mci, "total", Some(100), Some(10)),
        DataAllowance::finite(DataAllowanceKind::Night, 10 * GIB),
    );
    let query = PackageQuery {
        filter: PackageFilter {
            min_total_usable_data_bytes: Some(20 * GIB),
            ..Default::default()
        },
        ..Default::default()
    };
    assert_eq!(ids(query_packages(&[package], &query)), vec!["mci:total"]);
}

#[test]
fn filters_validity_buckets_and_unknown_validity_explicitly() {
    let packages = vec![
        validity(
            pkg("daily", Operator::Mci, "daily", Some(1), Some(1)),
            Validity::Days(1),
        ),
        validity(
            pkg("unknown", Operator::Mci, "unknown", Some(1), Some(1)),
            Validity::Unknown,
        ),
    ];
    let daily = PackageQuery {
        filter: PackageFilter {
            validity: Some(ValidityFilter::Daily),
            ..Default::default()
        },
        ..Default::default()
    };
    let unknown = PackageQuery {
        filter: PackageFilter {
            validity: Some(ValidityFilter::Unknown),
            ..Default::default()
        },
        ..Default::default()
    };
    assert_eq!(ids(query_packages(&packages, &daily)), vec!["mci:daily"]);
    assert_eq!(
        ids(query_packages(&packages, &unknown)),
        vec!["mci:unknown"]
    );
}

#[test]
fn filters_package_kind_and_combined_visibility() {
    let packages = vec![
        pkg("internet", Operator::Mci, "internet", Some(1), Some(1)),
        combined(pkg("combined", Operator::Mci, "combined", Some(1), Some(1))),
    ];
    let query = PackageQuery {
        filter: PackageFilter {
            include_combined: false,
            ..Default::default()
        },
        ..Default::default()
    };
    assert_eq!(ids(query_packages(&packages, &query)), vec!["mci:internet"]);
}

#[test]
fn empty_inputs_and_no_matches_are_normal_results() {
    let query = PackageQuery {
        filter: PackageFilter {
            operators: vec![Operator::Rightel],
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(query_packages(&[], &PackageQuery::default()).is_empty());
    assert!(query_packages(&[pkg("m", Operator::Mci, "mci", Some(1), Some(1))], &query).is_empty());
}

#[test]
fn searches_english_persian_case_and_partial_text() {
    let packages = vec![
        desc(
            pkg("i", Operator::Irancell, "Monthly LTE", Some(1), Some(1)),
            "بسته اینترنت ماهانه",
        ),
        pkg("m", Operator::Mci, "Other", Some(1), Some(1)),
    ];
    for term in ["monthly", "LTE", "اینترنت", "ماه"] {
        let query = PackageQuery {
            search_text: Some(term.to_string()),
            ..Default::default()
        };
        assert_eq!(ids(query_packages(&packages, &query)), vec!["irancell:i"]);
    }
}

#[test]
fn sorts_by_price_data_validity_newest_and_tie_breaks_deterministically() {
    let packages = vec![
        fetched(
            validity(
                pkg("b", Operator::Mci, "b", Some(100), Some(2)),
                Validity::Days(7),
            ),
            2,
        ),
        fetched(
            validity(
                pkg("a", Operator::Mci, "a", Some(50), Some(1)),
                Validity::Days(30),
            ),
            1,
        ),
    ];
    assert_eq!(
        ids(sorted(&packages, PackageSort::PriceAscending)),
        vec!["mci:a", "mci:b"]
    );
    assert_eq!(
        ids(sorted(&packages, PackageSort::DataDescending)),
        vec!["mci:b", "mci:a"]
    );
    assert_eq!(
        ids(sorted(&packages, PackageSort::ValidityDescending)),
        vec!["mci:a", "mci:b"]
    );
    assert_eq!(
        ids(sorted(&packages, PackageSort::Newest)),
        vec!["mci:b", "mci:a"]
    );
}

#[test]
fn best_value_sort_reuses_recommendation_value_ordering() {
    let packages = vec![
        pkg("bad", Operator::Mci, "bad", Some(100), Some(10)),
        pkg("good", Operator::Mci, "good", Some(160), Some(20)),
    ];
    assert_eq!(
        ids(sorted(&packages, PackageSort::BestValue)),
        vec!["mci:good", "mci:bad"]
    );
}

#[test]
fn pipeline_search_filter_sort_is_deterministic() {
    let packages = vec![
        pkg(
            "expensive",
            Operator::Mci,
            "Monthly net",
            Some(200),
            Some(5),
        ),
        pkg("cheap", Operator::Mci, "Monthly net", Some(100), Some(5)),
        pkg("skip", Operator::Irancell, "Daily net", Some(10), Some(1)),
    ];
    let query = PackageQuery {
        search_text: Some("monthly".to_string()),
        filter: PackageFilter {
            operators: vec![Operator::Mci],
            ..Default::default()
        },
        sort: Some(PackageSort::PriceAscending),
    };
    assert_eq!(
        ids(query_packages(&packages, &query)),
        vec!["mci:cheap", "mci:expensive"]
    );
}

fn sorted(packages: &[InternetPackage], sort: PackageSort) -> Vec<InternetPackage> {
    query_packages(
        packages,
        &PackageQuery {
            sort: Some(sort),
            ..Default::default()
        },
    )
}

fn ids(packages: Vec<InternetPackage>) -> Vec<String> {
    packages.into_iter().map(|p| p.id.0).collect()
}
