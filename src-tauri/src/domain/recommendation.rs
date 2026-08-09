use serde::{Deserialize, Serialize};

use super::{
    allowance::DataAllowanceKind,
    money::Money,
    operator::Operator,
    package::{PackageId, PackageKind, SimType, Validity},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecommendationStrategy {
    BestValue,
    HighestVolume,
    BestMonthly,
    CheapestUseful,
    BestNight,
    BestCombined,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecommendationContext {
    pub filters: PackageFilters,
    pub budget: Option<Money>,
    pub preferred_validity: Option<Validity>,
    pub required_general_data_bytes: Option<u64>,
    pub include_combined: Option<bool>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageFilters {
    pub operators: Vec<Operator>,
    pub sim_types: Vec<SimType>,
    pub package_kinds: Vec<PackageKind>,
    pub min_price: Option<Money>,
    pub max_price: Option<Money>,
    pub min_general_data_bytes: Option<u64>,
    pub max_general_data_bytes: Option<u64>,
    pub validity: Option<Validity>,
    pub general_internet_only: bool,
    pub include_combined: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecommendationSet {
    pub strategy: RecommendationStrategy,
    pub input_count: usize,
    pub filtered_count: usize,
    pub eligible_count: usize,
    pub results: Vec<Recommendation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Recommendation {
    pub strategy: RecommendationStrategy,
    pub package_id: PackageId,
    pub rank: usize,
    pub score: RecommendationScore,
    pub metrics: RecommendationMetrics,
    pub reasons: Vec<RecommendationReason>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RecommendationScore {
    FreeGeneralData,
    UnlimitedGeneralData,
    UnlimitedNightData,

    Ratio {
        numerator: u128,
        denominator: u128,
    },

    Bytes {
        value: u64,
    },

    Price {
        value: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecommendationMetrics {
    pub price_irr: Option<u64>,
    pub general_data_bytes: Option<u64>,
    pub night_data_bytes: Option<u64>,
    pub has_unlimited_general_data: bool,
    pub has_unlimited_night_data: bool,
    pub validity_days: Option<u32>,
    pub package_kind: Option<PackageKind>,
    pub has_voice: bool,
    pub has_sms: bool,
    pub value_ratio: Option<ValueRatio>,
    pub traffic_kind: Option<DataAllowanceKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValueRatio {
    pub price_irr: u64,
    pub data_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RecommendationReason {
    BestValueRatio,
    HighestGeneralData,
    BestMonthlyOption,
    CheapestUsefulOption,
    BestNightTraffic,
    BestCombinedBenefits,
    GeneralDataBytes { bytes: u64 },
    NightDataBytes { bytes: u64 },
    UnlimitedGeneralData,
    UnlimitedNightData,
    PriceIrr { amount: u64 },
    ValidityDays { days: u32 },
    IncludesVoice,
    IncludesSms,
    CombinedPackage,
}

impl Default for PackageFilters {
    fn default() -> Self {
        Self {
            operators: Vec::new(),
            sim_types: Vec::new(),
            package_kinds: Vec::new(),
            min_price: None,
            max_price: None,
            min_general_data_bytes: None,
            max_general_data_bytes: None,
            validity: None,
            general_internet_only: false,
            include_combined: true,
        }
    }
}
