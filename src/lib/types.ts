export type Operator = "mci" | "irancell" | "rightel" | "samantel";
export type Freshness = "fresh" | "stale";
export type PackageKind = "internet_only" | "combined";
export type SimType = "prepaid" | "postpaid" | "tdlte" | "data_sim" | "other";
export type Validity =
  | "other"
  | "unknown"
  | { days: number }
  | { hours: number };
export type PackageSort =
  | "price_ascending"
  | "price_descending"
  | "data_ascending"
  | "data_descending"
  | "validity_ascending"
  | "validity_descending"
  | "best_value"
  | "newest";
export type RecommendationStrategy =
  | "best_value"
  | "highest_volume"
  | "best_monthly"
  | "cheapest_useful"
  | "best_night"
  | "best_combined";

export interface Money {
  amount: number;
  currency: "irr" | "toman";
}
export interface DataAllowance {
  amountBytes: number | null;
  unlimited: boolean;
  kind: string;
  description: string | null;
}
export interface VoiceAllowance {
  minutes: number | null;
  unlimited: boolean;
}
export interface SmsAllowance {
  count: number | null;
  unlimited: boolean;
}
export interface PackageDto {
  id: string;
  operator: Operator;
  externalId: string;
  name: string;
  price: Money | null;
  validity: Validity;
  dataAllowances: DataAllowance[];
  voice: VoiceAllowance | null;
  sms: SmsAllowance | null;
  simTypes: SimType[];
  packageKind: PackageKind;
  availability: "available" | "unavailable" | "unknown";
  fetchedAtUnixSeconds: number | null;
}
export interface AppErrorDto {
  kind: string;
  message: string;
}
export interface OperatorStatusDto {
  operator: Operator;
  available: boolean;
  packageCount: number;
  freshness: Freshness | null;
  lastSuccessfulUpdateUnixSeconds: number | null;
  lastError: AppErrorDto | null;
  refreshing: boolean;
}
export interface CacheStatusDto {
  operators: OperatorStatusDto[];
  refreshInProgress: boolean;
}
export interface OperatorRefreshDto extends OperatorStatusDto {
  status: string;
  error: AppErrorDto | null;
}
export interface RefreshResultDto {
  operators: OperatorRefreshDto[];
}
export interface PackageFilter {
  operators: Operator[];
  simTypes: SimType[];
  minPrice: Money | null;
  maxPrice: Money | null;
  minGeneralDataBytes: number | null;
  minTotalUsableDataBytes: number | null;
  validity: string | null;
  packageKinds: PackageKind[];
  includeCombined: boolean;
  trafficKinds: string[];
}
export interface PackageQuery {
  searchText: string | null;
  filter: PackageFilter;
  sort: PackageSort | null;
}
export interface RecommendationContext {
  filters: {
    operators: Operator[];
    simTypes: SimType[];
    packageKinds: PackageKind[];
    minPrice: Money | null;
    maxPrice: Money | null;
    minGeneralDataBytes: number | null;
    maxGeneralDataBytes: number | null;
    validity: Validity | null;
    generalInternetOnly: boolean;
    includeCombined: boolean;
  };
  budget: Money | null;
  preferredValidity: Validity | null;
  requiredGeneralDataBytes: number | null;
  includeCombined: boolean | null;
  limit: number | null;
}
export interface RecommendationMetrics {
  priceIrr: number | null;
  generalDataBytes: number | null;
  nightDataBytes: number | null;
  hasUnlimitedGeneralData: boolean;
  hasUnlimitedNightData: boolean;
  validityDays: number | null;
  packageKind: PackageKind | null;
  hasVoice: boolean;
  hasSms: boolean;
  valueRatio: { priceIrr: number; dataBytes: number } | null;
  trafficKind: string | null;
}
export interface Recommendation {
  strategy: RecommendationStrategy;
  packageId: string;
  rank: number;
  score: unknown;
  metrics: RecommendationMetrics;
  reasons: Array<{ kind: string; [key: string]: unknown }>;
}
export interface RecommendationSet {
  strategy: RecommendationStrategy;
  inputCount: number;
  filteredCount: number;
  eligibleCount: number;
  results: Recommendation[];
}
