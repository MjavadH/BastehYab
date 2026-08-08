use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

use crate::app::dto::AppErrorDto;
use crate::cache::{CacheFreshness, CachedSnapshot, OperatorSnapshot};
use crate::domain::{operator::Operator, package::InternetPackage};

#[derive(Debug, Clone)]
pub struct AppRuntimeState {
    inner: Arc<RwLock<RuntimeInner>>,
}

#[derive(Debug, Clone, Default)]
struct RuntimeInner {
    snapshots: BTreeMap<Operator, OperatorRuntimeSnapshot>,
    refresh_in_progress: bool,
    operator_refreshing: BTreeMap<Operator, bool>,
}

#[derive(Debug, Clone)]
pub struct OperatorRuntimeSnapshot {
    pub snapshot: OperatorSnapshot,
    pub freshness: CacheFreshness,
    pub last_error: Option<AppErrorDto>,
}

impl Default for AppRuntimeState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppRuntimeState {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(RuntimeInner::default())),
        }
    }
    pub fn initialize_from_cache(
        &self,
        cached: Vec<(
            Operator,
            Result<Option<CachedSnapshot>, crate::cache::CacheError>,
        )>,
    ) {
        let mut inner = self.inner.write().expect("runtime state lock poisoned");
        for (operator, result) in cached {
            match result {
                Ok(Some(cached)) => {
                    inner.snapshots.insert(
                        operator,
                        OperatorRuntimeSnapshot {
                            snapshot: cached.snapshot,
                            freshness: cached.freshness,
                            last_error: None,
                        },
                    );
                }
                Ok(None) => {}
                Err(error) => {
                    eprintln!("cache load failed for {}: {}", operator.as_str(), error);
                }
            }
        }
    }
    pub fn packages(&self) -> Vec<InternetPackage> {
        self.inner
            .read()
            .expect("runtime state lock poisoned")
            .snapshots
            .values()
            .flat_map(|s| s.snapshot.packages.clone())
            .collect()
    }
    pub fn snapshot(&self, operator: Operator) -> Option<OperatorRuntimeSnapshot> {
        self.inner
            .read()
            .expect("runtime state lock poisoned")
            .snapshots
            .get(&operator)
            .cloned()
    }
    pub fn upsert_snapshot(
        &self,
        snapshot: OperatorSnapshot,
        freshness: CacheFreshness,
        error: Option<AppErrorDto>,
    ) {
        self.inner
            .write()
            .expect("runtime state lock poisoned")
            .snapshots
            .insert(
                snapshot.operator,
                OperatorRuntimeSnapshot {
                    snapshot,
                    freshness,
                    last_error: error,
                },
            );
    }
    pub fn set_refreshing(&self, operator: Option<Operator>, refreshing: bool) {
        let mut inner = self.inner.write().expect("runtime state lock poisoned");
        inner.refresh_in_progress = refreshing;
        if let Some(op) = operator {
            inner.operator_refreshing.insert(op, refreshing);
        }
    }
    pub fn operator_states(
        &self,
    ) -> (Vec<OperatorRuntimeSnapshot>, bool, BTreeMap<Operator, bool>) {
        let inner = self.inner.read().expect("runtime state lock poisoned");
        (
            inner.snapshots.values().cloned().collect(),
            inner.refresh_in_progress,
            inner.operator_refreshing.clone(),
        )
    }
}
