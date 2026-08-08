//! Query layer for canonical internet package browsing.
//!
//! Processing order is intentionally fixed and documented for future UI/Tauri callers:
//! dataset -> text search -> structured filters -> sorting. Pagination is left to callers;
//! this module returns owned collections because operator package catalogs are small.

use std::cmp::Ordering;

use serde::{Deserialize, Serialize};

use crate::{
    domain::{
        allowance::DataAllowanceKind,
        money::{Currency, Money},
        operator::Operator,
        package::{InternetPackage, PackageKind, SimType, Validity},
    },
    recommendations,
};

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageQuery {
    pub search_text: Option<String>,
    pub filter: PackageFilter,
    pub sort: Option<PackageSort>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageFilter {
    pub operators: Vec<Operator>,
    pub sim_types: Vec<SimType>,
    pub min_price: Option<Money>,
    pub max_price: Option<Money>,
    pub min_general_data_bytes: Option<u64>,
    pub min_total_usable_data_bytes: Option<u64>,
    pub validity: Option<ValidityFilter>,
    pub package_kinds: Vec<PackageKind>,
    pub include_combined: bool,
    pub traffic_kinds: Vec<DataAllowanceKind>,
}

impl Default for PackageFilter {
    fn default() -> Self {
        Self {
            operators: Vec::new(),
            sim_types: Vec::new(),
            min_price: None,
            max_price: None,
            min_general_data_bytes: None,
            min_total_usable_data_bytes: None,
            validity: None,
            package_kinds: Vec::new(),
            include_combined: true,
            traffic_kinds: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidityFilter {
    Daily,
    Weekly,
    Monthly,
    LongTerm,
    Exact(Validity),
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackageSort {
    PriceAscending,
    PriceDescending,
    DataAscending,
    DataDescending,
    ValidityAscending,
    ValidityDescending,
    BestValue,
    Newest,
}

#[derive(Debug, Clone, Default)]
pub struct PackageSearchService;

impl PackageSearchService {
    pub fn new() -> Self {
        Self
    }

    pub fn query(
        &self,
        packages: &[InternetPackage],
        query: &PackageQuery,
    ) -> Vec<InternetPackage> {
        query_packages(packages, query)
    }

    pub fn recommendation_candidates(
        &self,
        packages: &[InternetPackage],
        filter: &PackageFilter,
    ) -> Vec<InternetPackage> {
        query_packages(
            packages,
            &PackageQuery {
                search_text: None,
                filter: filter.clone(),
                sort: None,
            },
        )
    }
}

pub fn query_packages(packages: &[InternetPackage], query: &PackageQuery) -> Vec<InternetPackage> {
    let search = query.search_text.as_deref().and_then(normalize_search_text);
    let mut results = packages
        .iter()
        .filter(|package| {
            search
                .as_ref()
                .is_none_or(|needle| matches_search(package, needle))
        })
        .filter(|package| matches_filter(package, &query.filter))
        .cloned()
        .collect::<Vec<_>>();
    if let Some(sort) = query.sort {
        sort_packages(&mut results, sort);
    } else {
        sort_packages(&mut results, PackageSort::Newest);
    }
    results
}

pub fn sort_packages(packages: &mut [InternetPackage], sort: PackageSort) {
    if sort == PackageSort::BestValue {
        recommendations::sort_by_best_value(packages);
        return;
    }
    packages.sort_by(|a, b| compare_for_sort(a, b, sort));
}

pub fn matches_filter(package: &InternetPackage, filter: &PackageFilter) -> bool {
    if !filter.operators.is_empty() && !filter.operators.contains(&package.operator) {
        return false;
    }
    if !filter.sim_types.is_empty()
        && !package
            .sim_types
            .iter()
            .any(|s| filter.sim_types.contains(s))
    {
        return false;
    }
    if !filter.package_kinds.is_empty() && !filter.package_kinds.contains(&package.package_kind) {
        return false;
    }
    if !filter.include_combined && package.package_kind == PackageKind::Combined {
        return false;
    }
    if filter
        .validity
        .is_some_and(|validity| !matches_validity(package.validity, validity))
    {
        return false;
    }
    if !matches_price(package.price, filter.min_price, filter.max_price) {
        return false;
    }
    if filter.min_general_data_bytes.is_some_and(|min| {
        finite_data(package, DataAllowanceKind::General).is_none_or(|bytes| bytes < min)
    }) {
        return false;
    }
    if filter
        .min_total_usable_data_bytes
        .is_some_and(|min| total_usable_data(package).is_none_or(|bytes| bytes < min))
    {
        return false;
    }
    if !filter.traffic_kinds.is_empty()
        && !filter.traffic_kinds.iter().all(|kind| {
            package
                .data_allowances
                .iter()
                .any(|a| a.kind == *kind && (a.unlimited || a.amount_bytes.is_some()))
        })
    {
        return false;
    }
    true
}

fn compare_for_sort(a: &InternetPackage, b: &InternetPackage, sort: PackageSort) -> Ordering {
    let primary = match sort {
        PackageSort::PriceAscending => compare_optional_asc(price(a), price(b)),
        PackageSort::PriceDescending => compare_optional_desc(price(a), price(b)),
        PackageSort::DataAscending => compare_optional_asc(
            finite_data(a, DataAllowanceKind::General),
            finite_data(b, DataAllowanceKind::General),
        ),
        PackageSort::DataDescending => compare_optional_desc(
            finite_data(a, DataAllowanceKind::General),
            finite_data(b, DataAllowanceKind::General),
        ),
        PackageSort::ValidityAscending => compare_optional_asc(validity_days(a), validity_days(b)),
        PackageSort::ValidityDescending => {
            compare_optional_desc(validity_days(a), validity_days(b))
        }
        PackageSort::Newest => compare_optional_desc(
            a.metadata.fetched_at_unix_seconds,
            b.metadata.fetched_at_unix_seconds,
        ),
        PackageSort::BestValue => Ordering::Equal,
    };
    primary
        .then_with(|| compare_optional_asc(price(a), price(b)))
        .then_with(|| {
            compare_optional_desc(
                finite_data(a, DataAllowanceKind::General),
                finite_data(b, DataAllowanceKind::General),
            )
        })
        .then_with(|| compare_optional_desc(validity_days(a), validity_days(b)))
        .then_with(|| stable_identity(a).cmp(&stable_identity(b)))
}

fn matches_price(price: Option<Money>, min: Option<Money>, max: Option<Money>) -> bool {
    let Some(price) = price.filter(|m| m.currency == Currency::Irr) else {
        return min.is_none() && max.is_none();
    };
    min.is_none_or(|m| m.currency == Currency::Irr && price.amount >= m.amount)
        && max.is_none_or(|m| m.currency == Currency::Irr && price.amount <= m.amount)
}

fn matches_validity(validity: Validity, filter: ValidityFilter) -> bool {
    match filter {
        ValidityFilter::Daily => matches!(validity, Validity::Hours(1..=24) | Validity::Days(1)),
        ValidityFilter::Weekly => matches!(validity, Validity::Days(7..=13)),
        ValidityFilter::Monthly => matches!(validity, Validity::Days(28..=31)),
        ValidityFilter::LongTerm => matches!(validity, Validity::Days(32..)),
        ValidityFilter::Exact(expected) => validity == expected,
        ValidityFilter::Unknown => validity == Validity::Unknown,
    }
}

fn matches_search(package: &InternetPackage, needle: &str) -> bool {
    searchable_text(package)
        .iter()
        .any(|text| text.contains(needle))
}

fn searchable_text(package: &InternetPackage) -> Vec<String> {
    let mut text = vec![
        normalize_for_search(&package.name),
        normalize_for_search(package.operator.as_str()),
    ];
    if let Some(description) = &package.metadata.original_description {
        text.push(normalize_for_search(description));
    }
    if let Some(code) = &package.metadata.offer_code {
        text.push(normalize_for_search(code));
    }
    if let Some(code) = &package.metadata.regulatory_code {
        text.push(normalize_for_search(code));
    }
    for allowance in &package.data_allowances {
        if let Some(description) = &allowance.description {
            text.push(normalize_for_search(description));
        }
    }
    text
}

fn normalize_search_text(input: &str) -> Option<String> {
    let normalized = normalize_for_search(input);
    (!normalized.is_empty()).then_some(normalized)
}

fn normalize_for_search(input: &str) -> String {
    input
        .chars()
        .flat_map(char::to_lowercase)
        .map(normalize_search_char)
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn normalize_search_char(c: char) -> char {
    match c {
        '\u{200c}' | '\u{a0}' => ' ',
        'ي' => 'ی',
        'ك' => 'ک',
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
    }
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

fn total_usable_data(package: &InternetPackage) -> Option<u64> {
    package
        .data_allowances
        .iter()
        .filter(|a| !a.unlimited)
        .try_fold(0u64, |acc, a| {
            a.amount_bytes.map(|bytes| acc.saturating_add(bytes))
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
fn compare_optional_asc<T: Ord>(a: Option<T>, b: Option<T>) -> Ordering {
    match (a, b) {
        (Some(a), Some(b)) => a.cmp(&b),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}
fn compare_optional_desc<T: Ord>(a: Option<T>, b: Option<T>) -> Ordering {
    compare_optional_asc(b, a)
}

#[cfg(test)]
mod tests;
