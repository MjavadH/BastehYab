use serde::{Deserialize, Serialize};

use crate::{
    cache::CacheFreshness,
    domain::{
        allowance::{DataAllowance, SmsAllowance, VoiceAllowance},
        money::Money,
        operator::Operator,
        package::{
            Availability, InternetPackage, PackageId, PackageKind, PurchaseInfo, SimType, Validity,
        },
        recommendation::{RecommendationContext, RecommendationSet, RecommendationStrategy},
    },
    filtering::{PackageFilter, PackageQuery, PackageSort},
    refresh::orchestrator::{OperatorRefreshResult, OperatorRefreshStatus, RefreshError},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageDto {
    pub id: String,
    pub operator: Operator,
    pub external_id: String,
    pub name: String,
    pub price: Option<Money>,
    pub validity: Validity,
    pub data_allowances: Vec<DataAllowance>,
    pub voice: Option<VoiceAllowance>,
    pub sms: Option<SmsAllowance>,
    pub sim_types: Vec<SimType>,
    pub package_kind: PackageKind,
    pub availability: Availability,
    pub purchase: PurchaseInfo,
    pub fetched_at_unix_seconds: Option<i64>,
}

impl From<&InternetPackage> for PackageDto {
    fn from(package: &InternetPackage) -> Self {
        Self {
            id: package.id.0.clone(),
            operator: package.operator,
            external_id: package.external_id.clone(),
            name: package.name.clone(),
            price: package.price,
            validity: package.validity,
            data_allowances: package.data_allowances.clone(),
            voice: package.voice.clone(),
            sms: package.sms.clone(),
            sim_types: package.sim_types.clone(),
            package_kind: package.package_kind,
            availability: package.availability,
            purchase: package.purchase.clone(),
            fetched_at_unix_seconds: package.metadata.fetched_at_unix_seconds,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageDetailsDto {
    pub package: PackageDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshResultDto {
    pub operators: Vec<OperatorRefreshDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperatorRefreshDto {
    pub operator: Operator,
    pub status: OperatorRefreshStatus,
    pub package_count: usize,
    pub freshness: Option<CacheFreshness>,
    pub last_successful_update_unix_seconds: Option<i64>,
    pub error: Option<AppErrorDto>,
}

impl From<OperatorRefreshResult> for OperatorRefreshDto {
    fn from(result: OperatorRefreshResult) -> Self {
        let package_count = result.snapshot.as_ref().map_or(0, |s| s.packages.len());
        let last_successful_update_unix_seconds =
            result.snapshot.as_ref().map(|s| s.fetched_at_unix_seconds);
        Self {
            operator: result.operator,
            status: result.status,
            package_count,
            freshness: result.freshness,
            last_successful_update_unix_seconds,
            error: result.error.map(AppErrorDto::from),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheStatusDto {
    pub operators: Vec<OperatorStatusDto>,
    pub refresh_in_progress: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperatorStatusDto {
    pub operator: Operator,
    pub available: bool,
    pub package_count: usize,
    pub freshness: Option<CacheFreshness>,
    pub last_successful_update_unix_seconds: Option<i64>,
    pub last_error: Option<AppErrorDto>,
    pub refreshing: bool,
}

pub type PackageQueryDto = PackageQuery;
pub type PackageFilterDto = PackageFilter;
pub type PackageSortDto = PackageSort;
pub type RecommendationContextDto = RecommendationContext;
pub type RecommendationSetDto = RecommendationSet;
pub type RecommendationStrategyDto = RecommendationStrategy;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppErrorDto {
    pub kind: AppErrorKind,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppErrorKind {
    InvalidRequest,
    RefreshFailure,
    OperatorUnavailable,
    CacheUnavailable,
    InternalFailure,
}

impl From<RefreshError> for AppErrorDto {
    fn from(value: RefreshError) -> Self {
        Self {
            kind: AppErrorKind::RefreshFailure,
            message: value.to_string(),
        }
    }
}

pub fn package_id(value: String) -> PackageId {
    PackageId(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::refresh::orchestrator::OperatorRefreshStatus;

    #[test]
    fn error_serializes_as_frontend_safe_contract() {
        let json = serde_json::to_value(AppErrorDto {
            kind: AppErrorKind::InvalidRequest,
            message: "bad input".into(),
        })
        .unwrap();
        assert_eq!(json["kind"], "invalid_request");
        assert_eq!(json["message"], "bad input");
    }

    #[test]
    fn refresh_status_serializes_without_debug_strings() {
        let json = serde_json::to_value(OperatorRefreshDto {
            operator: Operator::Mci,
            status: OperatorRefreshStatus::FallbackStale,
            package_count: 1,
            freshness: Some(CacheFreshness::Stale),
            last_successful_update_unix_seconds: Some(10),
            error: None,
        })
        .unwrap();
        assert_eq!(json["status"], "fallback_stale");
        assert_eq!(json["operator"], "mci");
    }
}
