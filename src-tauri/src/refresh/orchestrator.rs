use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use thiserror::Error;

use crate::{
    cache::{
        all_operators, now_unix_seconds, CacheError, CacheFreshness, CacheStore, CachedSnapshot,
        OperatorSnapshot,
    },
    domain::{operator::Operator, package::InternetPackage},
    normalizers::{validate_package, NormalizationError},
};

pub trait Collector: Send + Sync {
    fn collect(&self, operator: Operator) -> Result<CollectedPackages, CollectorError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectedPackages {
    pub fetched_at_unix_seconds: i64,
    pub raw_record_count: usize,
    pub packages: Vec<InternetPackage>,
    pub normalization_failures: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CollectorError {
    #[error("collector failed: {0}")]
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RefreshError {
    #[error("collector failed: {0}")]
    Collector(String),
    #[error("normalization failed: {0}")]
    Normalization(String),
    #[error("candidate validation failed: {0}")]
    CandidateValidation(String),
    #[error("candidate health check failed: {0}")]
    Health(String),
    #[error("cache persistence failed: {0}")]
    Persistence(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperatorRefreshStatus {
    Updated,
    FallbackStale,
    MissingData,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperatorRefreshResult {
    pub operator: Operator,
    pub status: OperatorRefreshStatus,
    pub snapshot: Option<OperatorSnapshot>,
    pub freshness: Option<CacheFreshness>,
    pub error: Option<RefreshError>,
}

#[derive(Debug, Clone)]
pub struct RefreshOrchestrator<C> {
    cache: CacheStore,
    collector: Arc<C>,
    locks: Arc<BTreeMap<Operator, Mutex<()>>>,
}

impl<C: Collector> RefreshOrchestrator<C> {
    pub fn new(cache: CacheStore, collector: C) -> Self {
        Self {
            cache,
            collector: Arc::new(collector),
            locks: Arc::new(
                all_operators()
                    .into_iter()
                    .map(|op| (op, Mutex::new(())))
                    .collect(),
            ),
        }
    }

    pub fn load_startup(
        &self,
        now: i64,
    ) -> Vec<(Operator, Result<Option<CachedSnapshot>, CacheError>)> {
        self.cache.load_all(now)
    }

    pub fn refresh_operator(&self, operator: Operator, now: i64) -> OperatorRefreshResult {
        let previous = self.cache.load(operator, now).ok().flatten();
        let collected = match self.collector.collect(operator) {
            Ok(collected) => collected,
            Err(error) => {
                return fallback(
                    operator,
                    previous,
                    RefreshError::Collector(error.to_string()),
                )
            }
        };
        let candidate = match build_candidate(operator, now, collected, previous.as_ref()) {
            Ok(snapshot) => snapshot,
            Err(error) => return fallback(operator, previous, error),
        };

        let lock = self.locks.get(&operator).expect("operator lock exists");
        let _guard = lock.lock().expect("operator refresh lock poisoned");
        let previous_after_wait = self.cache.load(operator, now).ok().flatten();
        if let Err(error) = assess_health(
            &candidate,
            previous_after_wait.as_ref().map(|p| &p.snapshot),
            candidate.packages.len(),
            0,
        ) {
            return fallback(operator, previous_after_wait.or(previous), error);
        }
        if let Err(error) = self.cache.commit(&candidate) {
            return fallback(
                operator,
                previous_after_wait.or(previous),
                RefreshError::Persistence(error.to_string()),
            );
        }
        OperatorRefreshResult {
            operator,
            status: OperatorRefreshStatus::Updated,
            snapshot: Some(candidate),
            freshness: Some(CacheFreshness::Fresh),
            error: None,
        }
    }

    pub fn refresh_all(&self, now: i64) -> Vec<OperatorRefreshResult> {
        all_operators()
            .into_iter()
            .map(|op| self.refresh_operator(op, now))
            .collect()
    }
}

fn build_candidate(
    operator: Operator,
    now: i64,
    collected: CollectedPackages,
    previous: Option<&CachedSnapshot>,
) -> Result<OperatorSnapshot, RefreshError> {
    let snapshot = OperatorSnapshot {
        operator,
        fetched_at_unix_seconds: collected.fetched_at_unix_seconds,
        stored_at_unix_seconds: now,
        packages: collected.packages,
    };
    validate_candidate(&snapshot).map_err(|e| RefreshError::CandidateValidation(e.to_string()))?;
    assess_health(
        &snapshot,
        previous.map(|p| &p.snapshot),
        collected.raw_record_count,
        collected.normalization_failures,
    )?;
    Ok(snapshot)
}

fn validate_candidate(snapshot: &OperatorSnapshot) -> Result<(), NormalizationError> {
    for package in &snapshot.packages {
        validate_package(package)?;
    }
    Ok(())
}

fn assess_health(
    snapshot: &OperatorSnapshot,
    previous: Option<&OperatorSnapshot>,
    raw_count: usize,
    normalization_failures: usize,
) -> Result<(), RefreshError> {
    if snapshot.packages.is_empty() {
        return Err(RefreshError::Health("empty candidate dataset".into()));
    }
    let mut ids = std::collections::BTreeSet::new();
    for package in &snapshot.packages {
        if package.operator != snapshot.operator {
            return Err(RefreshError::Health("operator mismatch".into()));
        }
        if !ids.insert(package.id.clone()) {
            return Err(RefreshError::Health(
                "duplicate canonical package identity".into(),
            ));
        }
    }
    if raw_count > 0 && normalization_failures > snapshot.packages.len() {
        return Err(RefreshError::Normalization(
            "more normalization failures than accepted packages".into(),
        ));
    }
    if previous.is_some_and(|p| !p.packages.is_empty()) && snapshot.packages.is_empty() {
        return Err(RefreshError::Health(
            "empty candidate would replace known non-empty catalog".into(),
        ));
    }
    Ok(())
}

fn fallback(
    operator: Operator,
    previous: Option<CachedSnapshot>,
    error: RefreshError,
) -> OperatorRefreshResult {
    match previous {
        Some(cached) => OperatorRefreshResult {
            operator,
            status: OperatorRefreshStatus::FallbackStale,
            snapshot: Some(cached.snapshot),
            freshness: Some(cached.freshness),
            error: Some(error),
        },
        None => OperatorRefreshResult {
            operator,
            status: OperatorRefreshStatus::MissingData,
            snapshot: None,
            freshness: None,
            error: Some(error),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        cache::{operator_path_for_tests, CacheStore},
        domain::{
            allowance::{DataAllowance, DataAllowanceKind},
            money::Money,
            package::*,
        },
    };
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicUsize, Ordering},
        thread,
    };

    fn temp_dir(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("bastehyab-{name}-{}", now_unix_seconds()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }
    fn pkg(op: Operator, id: &str) -> InternetPackage {
        InternetPackage {
            id: PackageId::canonical(op, id),
            operator: op,
            external_id: id.into(),
            name: format!("pkg {id}"),
            price: Some(Money::irr(1000)),
            validity: Validity::Days(30),
            data_allowances: vec![DataAllowance::finite(DataAllowanceKind::General, 1024)],
            voice: None,
            sms: None,
            sim_types: vec![SimType::Prepaid],
            package_kind: PackageKind::InternetOnly,
            availability: Availability::Available,
            purchase: PurchaseInfo::default(),
            metadata: PackageMetadata::default(),
        }
    }
    fn snap(op: Operator, id: &str, at: i64) -> OperatorSnapshot {
        OperatorSnapshot {
            operator: op,
            fetched_at_unix_seconds: at,
            stored_at_unix_seconds: at,
            packages: vec![pkg(op, id)],
        }
    }

    #[derive(Clone)]
    struct Fake {
        map: Arc<BTreeMap<Operator, Result<CollectedPackages, CollectorError>>>,
        calls: Arc<AtomicUsize>,
    }
    impl Fake {
        fn one(op: Operator, result: Result<CollectedPackages, CollectorError>) -> Self {
            Self {
                map: Arc::new([(op, result)].into_iter().collect()),
                calls: Arc::new(AtomicUsize::new(0)),
            }
        }
    }
    impl Collector for Fake {
        fn collect(&self, operator: Operator) -> Result<CollectedPackages, CollectorError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.map
                .get(&operator)
                .cloned()
                .unwrap_or_else(|| Err(CollectorError::Failed("missing fake".into())))
        }
    }
    fn collected(op: Operator, id: &str, at: i64) -> CollectedPackages {
        CollectedPackages {
            fetched_at_unix_seconds: at,
            raw_record_count: 1,
            packages: vec![pkg(op, id)],
            normalization_failures: 0,
        }
    }

    #[test]
    fn cache_miss() {
        let store = CacheStore::new(temp_dir("miss"));
        assert!(store.load(Operator::Mci, 10).unwrap().is_none());
    }
    #[test]
    fn valid_cache_load_and_round_trip() {
        let root = temp_dir("valid");
        let store = CacheStore::new(root);
        let s = snap(Operator::Mci, "a", 10);
        store.commit(&s).unwrap();
        let loaded = store.load(Operator::Mci, 20).unwrap().unwrap();
        assert_eq!(loaded.snapshot, s);
        assert_eq!(loaded.freshness, CacheFreshness::Fresh);
    }
    #[test]
    fn stale_cache() {
        let root = temp_dir("stale");
        let store = CacheStore::new(root);
        store.commit(&snap(Operator::Mci, "a", 1)).unwrap();
        assert_eq!(
            store
                .load(Operator::Mci, 30_000)
                .unwrap()
                .unwrap()
                .freshness,
            CacheFreshness::Stale
        );
    }
    #[test]
    fn corrupt_cache_is_error() {
        let root = temp_dir("corrupt");
        fs::write(operator_path_for_tests(&root, Operator::Mci), b"{").unwrap();
        assert!(store_err(CacheStore::new(root).load(Operator::Mci, 1)).contains("serialization"));
    }
    #[test]
    fn unsupported_schema() {
        let root = temp_dir("schema");
        fs::write(operator_path_for_tests(&root, Operator::Mci), r#"{"schemaVersion":999,"operator":"mci","fetchedAtUnixSeconds":1,"storedAtUnixSeconds":1,"packages":[]}"#).unwrap();
        assert!(matches!(
            CacheStore::new(root).load(Operator::Mci, 1),
            Err(CacheError::UnsupportedSchema { .. })
        ));
    }
    #[test]
    fn operator_mismatch() {
        let mut s = snap(Operator::Mci, "a", 1);
        s.packages[0].operator = Operator::Irancell;
        assert!(matches!(
            CacheStore::new(temp_dir("mismatch")).commit(&s),
            Err(CacheError::OperatorMismatch { .. })
        ));
    }
    #[test]
    fn atomic_replacement_keeps_old_on_failed_commit() {
        let root = temp_dir("atomic");
        let store = CacheStore::new(root.clone());
        store.commit(&snap(Operator::Mci, "old", 1)).unwrap();
        let mut bad = snap(Operator::Mci, "bad", 2);
        bad.packages[0].external_id = "different".into();
        assert!(store.commit(&bad).is_err());
        assert_eq!(
            store
                .load(Operator::Mci, 3)
                .unwrap()
                .unwrap()
                .snapshot
                .packages[0]
                .external_id,
            "old"
        );
    }
    #[test]
    fn startup_loading_is_independent_for_missing_and_corrupt_caches() {
        let root = temp_dir("startup");
        let store = CacheStore::new(root.clone());
        store.commit(&snap(Operator::Mci, "m", 1)).unwrap();
        store.commit(&snap(Operator::Irancell, "i", 1)).unwrap();
        fs::write(
            operator_path_for_tests(&root, Operator::Rightel),
            b"not json",
        )
        .unwrap();
        let results = store.load_all(2);
        assert!(results
            .iter()
            .any(|(op, r)| *op == Operator::Mci && r.as_ref().unwrap().is_some()));
        assert!(results
            .iter()
            .any(|(op, r)| *op == Operator::Irancell && r.as_ref().unwrap().is_some()));
        assert!(results
            .iter()
            .any(|(op, r)| *op == Operator::Rightel && r.is_err()));
        assert!(results
            .iter()
            .any(|(op, r)| *op == Operator::Samantel && r.as_ref().unwrap().is_none()));
    }

    #[test]
    fn successful_refresh() {
        let root = temp_dir("refresh-ok");
        let orch = RefreshOrchestrator::new(
            CacheStore::new(root),
            Fake::one(Operator::Mci, Ok(collected(Operator::Mci, "new", 10))),
        );
        let r = orch.refresh_operator(Operator::Mci, 11);
        assert_eq!(r.status, OperatorRefreshStatus::Updated);
        assert_eq!(r.snapshot.unwrap().packages[0].external_id, "new");
    }
    #[test]
    fn collector_failure_falls_back() {
        let root = temp_dir("collector-fail");
        let store = CacheStore::new(root);
        store.commit(&snap(Operator::Mci, "old", 1)).unwrap();
        let orch = RefreshOrchestrator::new(
            store,
            Fake::one(Operator::Mci, Err(CollectorError::Failed("boom".into()))),
        );
        let r = orch.refresh_operator(Operator::Mci, 30_000);
        assert_eq!(r.status, OperatorRefreshStatus::FallbackStale);
        assert_eq!(r.snapshot.unwrap().packages[0].external_id, "old");
    }
    #[test]
    fn normalization_failure() {
        let orch = RefreshOrchestrator::new(
            CacheStore::new(temp_dir("norm-fail")),
            Fake::one(
                Operator::Mci,
                Ok(CollectedPackages {
                    fetched_at_unix_seconds: 1,
                    raw_record_count: 10,
                    packages: vec![pkg(Operator::Mci, "a")],
                    normalization_failures: 9,
                }),
            ),
        );
        let r = orch.refresh_operator(Operator::Mci, 2);
        assert!(matches!(r.error, Some(RefreshError::Normalization(_))));
    }
    #[test]
    fn candidate_validation_failure() {
        let mut p = pkg(Operator::Mci, "a");
        p.name = "".into();
        let orch = RefreshOrchestrator::new(
            CacheStore::new(temp_dir("validation-fail")),
            Fake::one(
                Operator::Mci,
                Ok(CollectedPackages {
                    fetched_at_unix_seconds: 1,
                    raw_record_count: 1,
                    packages: vec![p],
                    normalization_failures: 0,
                }),
            ),
        );
        let r = orch.refresh_operator(Operator::Mci, 2);
        assert!(matches!(
            r.error,
            Some(RefreshError::CandidateValidation(_))
        ));
    }
    #[test]
    fn persistence_failure_where_testable() {
        let root = temp_dir("persist-fail");
        let file_root = root.join("file");
        fs::write(&file_root, b"not dir").unwrap();
        let orch = RefreshOrchestrator::new(
            CacheStore::new(file_root),
            Fake::one(Operator::Mci, Ok(collected(Operator::Mci, "a", 1))),
        );
        let r = orch.refresh_operator(Operator::Mci, 2);
        assert!(matches!(r.error, Some(RefreshError::Persistence(_))));
    }
    #[test]
    fn last_known_good_fallback_empty_rejection() {
        let root = temp_dir("lkg");
        let store = CacheStore::new(root);
        store.commit(&snap(Operator::Mci, "old", 1)).unwrap();
        let orch = RefreshOrchestrator::new(
            store,
            Fake::one(
                Operator::Mci,
                Ok(CollectedPackages {
                    fetched_at_unix_seconds: 2,
                    raw_record_count: 0,
                    packages: vec![],
                    normalization_failures: 0,
                }),
            ),
        );
        let r = orch.refresh_operator(Operator::Mci, 3);
        assert_eq!(r.status, OperatorRefreshStatus::FallbackStale);
        assert_eq!(r.snapshot.unwrap().packages[0].external_id, "old");
    }
    #[test]
    fn independent_multi_operator_refresh() {
        let mut map = BTreeMap::new();
        map.insert(Operator::Mci, Ok(collected(Operator::Mci, "m", 1)));
        map.insert(Operator::Irancell, Err(CollectorError::Failed("x".into())));
        map.insert(Operator::Rightel, Ok(collected(Operator::Rightel, "r", 1)));
        map.insert(
            Operator::Samantel,
            Ok(collected(Operator::Samantel, "s", 1)),
        );
        let orch = RefreshOrchestrator::new(
            CacheStore::new(temp_dir("multi")),
            Fake {
                map: Arc::new(map),
                calls: Arc::new(AtomicUsize::new(0)),
            },
        );
        let rs = orch.refresh_all(2);
        assert_eq!(
            rs.iter()
                .filter(|r| r.status == OperatorRefreshStatus::Updated)
                .count(),
            3
        );
        assert_eq!(
            rs.iter()
                .filter(|r| r.status == OperatorRefreshStatus::MissingData)
                .count(),
            1
        );
    }
    #[test]
    fn concurrent_refresh_safety() {
        let orch = Arc::new(RefreshOrchestrator::new(
            CacheStore::new(temp_dir("concurrent")),
            Fake::one(Operator::Mci, Ok(collected(Operator::Mci, "a", 1))),
        ));
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let o = orch.clone();
                thread::spawn(move || o.refresh_operator(Operator::Mci, 2))
            })
            .collect();
        for h in handles {
            assert!(matches!(
                h.join().unwrap().status,
                OperatorRefreshStatus::Updated
            ));
        }
        assert!(orch.cache.load(Operator::Mci, 2).unwrap().is_some());
    }
    fn store_err<T: std::fmt::Debug>(r: Result<T, CacheError>) -> String {
        r.unwrap_err().to_string()
    }
}
