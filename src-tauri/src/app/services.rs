use super::state::AppRuntimeState;
use crate::{
    app::dto::*,
    cache::{all_operators, now_unix_seconds, CacheFreshness, CacheStore},
    collectors::OperatorCollectors,
    domain::{operator::Operator, package::InternetPackage, recommendation::RecommendationContext},
    filtering::{PackageQuery, PackageSearchService},
    recommendations,
    refresh::orchestrator::RefreshOrchestrator,
};

#[derive(Debug, Clone)]
pub struct ApplicationServices {
    pub state: AppRuntimeState,
    refresh: RefreshOrchestrator<OperatorCollectors>,
    filter: PackageSearchService,
}

impl ApplicationServices {
    pub fn initialize(cache: CacheStore) -> Self {
        let state = AppRuntimeState::new();
        let refresh = RefreshOrchestrator::new(cache, OperatorCollectors::new());
        state.initialize_from_cache(refresh.load_startup(now_unix_seconds()));
        Self {
            state,
            refresh,
            filter: PackageSearchService::new(),
        }
    }
    pub fn package_service(&self) -> PackageService {
        PackageService {
            state: self.state.clone(),
        }
    }
    pub fn refresh_service(&self) -> RefreshService {
        RefreshService {
            state: self.state.clone(),
            refresh: self.refresh.clone(),
        }
    }
    pub fn recommendation_service(&self) -> RecommendationService {
        RecommendationService {
            state: self.state.clone(),
        }
    }
    pub fn filter_service(&self) -> FilterService {
        FilterService {
            state: self.state.clone(),
            filter: self.filter.clone(),
        }
    }
}

#[derive(Clone)]
pub struct PackageService {
    state: AppRuntimeState,
}
impl PackageService {
    pub fn all(&self) -> Vec<PackageDto> {
        self.state.packages().iter().map(PackageDto::from).collect()
    }
    pub fn by_operator(&self, operator: Operator) -> Vec<PackageDto> {
        self.state.snapshot(operator).map_or_else(Vec::new, |s| {
            s.snapshot.packages.iter().map(PackageDto::from).collect()
        })
    }
    pub fn details(&self, id: String) -> Result<PackageDetailsDto, AppErrorDto> {
        self.state
            .packages()
            .iter()
            .find(|p| p.id.0 == id)
            .map(|p| PackageDetailsDto {
                package: PackageDto::from(p),
            })
            .ok_or(AppErrorDto {
                kind: AppErrorKind::InvalidRequest,
                message: "package id was not found".into(),
            })
    }
}

#[derive(Clone)]
pub struct RefreshService {
    state: AppRuntimeState,
    refresh: RefreshOrchestrator<OperatorCollectors>,
}
impl RefreshService {
    pub fn refresh_operator(&self, operator: Operator) -> RefreshResultDto {
        println!("refresh started for {}", operator.as_str());
        self.state.set_refreshing(Some(operator), true);
        let result = self.refresh.refresh_operator(operator, now_unix_seconds());
        if let Some(snapshot) = result.snapshot.clone() {
            self.state.upsert_snapshot(
                snapshot,
                result.freshness.unwrap_or(CacheFreshness::Stale),
                result.error.clone().map(AppErrorDto::from),
            );
        }
        self.state.set_refreshing(Some(operator), false);
        RefreshResultDto {
            operators: vec![result.into()],
        }
    }
    pub fn refresh_all(&self) -> RefreshResultDto {
        println!("refresh all started");
        self.state.set_refreshing(None, true);
        let now = now_unix_seconds();
        let handles = all_operators()
            .into_iter()
            .map(|operator| {
                self.state.set_refreshing(Some(operator), true);
                let refresh = self.refresh.clone();
                std::thread::spawn(move || refresh.refresh_operator(operator, now))
            })
            .collect::<Vec<_>>();
        let mut dto = Vec::new();
        for (operator, handle) in all_operators().into_iter().zip(handles) {
            let result = match handle.join() {
                Ok(result) => result,
                Err(_) => crate::refresh::orchestrator::OperatorRefreshResult {
                    operator,
                    status: crate::refresh::orchestrator::OperatorRefreshStatus::MissingData,
                    snapshot: None,
                    freshness: None,
                    error: Some(crate::refresh::orchestrator::RefreshError::Collector(
                        "refresh worker failed".into(),
                    )),
                },
            };
            if let Some(snapshot) = result.snapshot.clone() {
                self.state.upsert_snapshot(
                    snapshot,
                    result.freshness.unwrap_or(CacheFreshness::Stale),
                    result.error.clone().map(AppErrorDto::from),
                );
            }
            self.state.set_refreshing(Some(operator), false);
            dto.push(result.into());
        }
        self.state.set_refreshing(None, false);
        RefreshResultDto { operators: dto }
    }
}

#[derive(Clone)]
pub struct RecommendationService {
    state: AppRuntimeState,
}
impl RecommendationService {
    pub fn all(&self, context: RecommendationContext) -> Vec<RecommendationSetDto> {
        recommendations::get_recommendations(&self.state.packages(), &context)
    }
    pub fn by_strategy(
        &self,
        strategy: RecommendationStrategyDto,
        context: RecommendationContextDto,
    ) -> RecommendationSetDto {
        recommendations::recommend(&self.state.packages(), strategy, &context)
    }
}

#[derive(Clone)]
pub struct FilterService {
    state: AppRuntimeState,
    filter: PackageSearchService,
}
impl FilterService {
    pub fn apply(&self, query: PackageQuery) -> Vec<PackageDto> {
        self.filter
            .query(&self.state.packages(), &query)
            .iter()
            .map(PackageDto::from)
            .collect()
    }
    pub fn search(&self, text: String) -> Vec<PackageDto> {
        self.apply(PackageQuery {
            search_text: Some(text),
            ..PackageQuery::default()
        })
    }
    pub fn sort(&self, sort: PackageSortDto) -> Vec<PackageDto> {
        self.apply(PackageQuery {
            sort: Some(sort),
            ..PackageQuery::default()
        })
    }
}

pub fn cache_status(state: &AppRuntimeState) -> CacheStatusDto {
    let (snapshots, refresh_in_progress, refreshing) = state.operator_states();
    let operators = all_operators()
        .into_iter()
        .map(|operator| {
            let snap = snapshots.iter().find(|s| s.snapshot.operator == operator);
            OperatorStatusDto {
                operator,
                available: snap.is_some(),
                package_count: snap.map_or(0, |s| s.snapshot.packages.len()),
                freshness: snap.map(|s| s.freshness),
                last_successful_update_unix_seconds: snap
                    .map(|s| s.snapshot.fetched_at_unix_seconds),
                last_error: snap.and_then(|s| s.last_error.clone()),
                refreshing: refreshing.get(&operator).copied().unwrap_or(false),
            }
        })
        .collect();
    CacheStatusDto {
        operators,
        refresh_in_progress,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        allowance::{DataAllowance, DataAllowanceKind},
        money::Money,
        package::*,
    };
    fn pkg(op: Operator, id: &str, price: u64) -> InternetPackage {
        InternetPackage {
            id: PackageId::canonical(op, id),
            operator: op,
            external_id: id.into(),
            name: format!("Package {id}"),
            price: Some(Money::irr(price)),
            validity: Validity::Days(30),
            data_allowances: vec![DataAllowance::finite(
                DataAllowanceKind::General,
                1024 * 1024 * 1024,
            )],
            voice: None,
            sms: None,
            sim_types: vec![SimType::Prepaid],
            package_kind: PackageKind::InternetOnly,
            availability: Availability::Available,
            purchase: PurchaseInfo::default(),
            metadata: PackageMetadata {
                fetched_at_unix_seconds: Some(1),
                ..Default::default()
            },
        }
    }
    fn state_with_package() -> AppRuntimeState {
        let s = AppRuntimeState::new();
        s.upsert_snapshot(
            crate::cache::OperatorSnapshot {
                operator: Operator::Mci,
                fetched_at_unix_seconds: 1,
                stored_at_unix_seconds: 1,
                packages: vec![pkg(Operator::Mci, "a", 1000)],
            },
            CacheFreshness::Fresh,
            None,
        );
        s
    }
    #[test]
    fn package_retrieval() {
        let service = PackageService {
            state: state_with_package(),
        };
        assert_eq!(service.all().len(), 1);
        assert_eq!(service.by_operator(Operator::Mci).len(), 1);
        assert!(service.details("mci:a".into()).is_ok());
    }
    #[test]
    fn filtering_service_searches() {
        let service = FilterService {
            state: state_with_package(),
            filter: PackageSearchService::new(),
        };
        assert_eq!(service.search("package".into()).len(), 1);
    }
    #[test]
    fn recommendation_service_returns_sets() {
        let service = RecommendationService {
            state: state_with_package(),
        };
        assert!(!service.all(RecommendationContext::default()).is_empty());
    }
    #[test]
    fn state_cache_status() {
        let state = state_with_package();
        assert_eq!(
            cache_status(&state)
                .operators
                .iter()
                .filter(|o| o.available)
                .count(),
            1
        );
    }
}
