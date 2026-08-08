import { invoke } from "@tauri-apps/api/core";
import type {
  CacheStatusDto,
  Operator,
  PackageDto,
  PackageQuery,
  RecommendationContext,
  RecommendationSet,
  RefreshResultDto,
} from "./types";

export interface AppHealth {
  status: "ok";
  appName: string;
}
export async function getAppHealth(): Promise<AppHealth> {
  return invoke<AppHealth>("app_health");
}
export async function getPackages(): Promise<PackageDto[]> {
  return invoke<PackageDto[]>("get_packages");
}
export async function applyPackageFilters(
  query: PackageQuery,
): Promise<PackageDto[]> {
  return invoke<PackageDto[]>("apply_package_filters", { query });
}
export async function getRecommendations(
  context: RecommendationContext,
): Promise<RecommendationSet[]> {
  return invoke<RecommendationSet[]>("get_recommendations", { context });
}
export async function getCacheStatus(): Promise<CacheStatusDto> {
  return invoke<CacheStatusDto>("get_cache_status");
}
export async function refreshAllOperators(): Promise<RefreshResultDto> {
  return invoke<RefreshResultDto>("refresh_all_operators");
}
export async function refreshOperator(
  operator: Operator,
): Promise<RefreshResultDto> {
  return invoke<RefreshResultDto>("refresh_operator", { operator });
}
