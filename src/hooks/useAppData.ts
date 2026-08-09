import { useCallback, useEffect, useState } from "react";
import {
  applyPackageFilters,
  getCacheStatus,
  getPackages,
  getRecommendations,
  refreshAllOperators,
  refreshOperator,
} from "../lib/tauri";
import type {
  CacheStatusDto,
  Operator,
  PackageDto,
  PackageQuery,
  RecommendationSet,
} from "../lib/types";
import {
  emptyFilter,
  packageQuery,
  recommendationContext,
} from "../services/contracts";

export function useAppData() {
  const [packages, setPackages] = useState<PackageDto[]>([]);
  const [recommendations, setRecommendations] = useState<RecommendationSet[]>(
    [],
  );
  const [status, setStatus] = useState<CacheStatusDto | null>(null);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [error, setError] = useState(false);
  const [query, setQuery] = useState<PackageQuery>(
    packageQuery("", emptyFilter(), "newest"),
  );
  const load = useCallback(async () => {
    setLoading(true);
    setError(false);

    const results = await Promise.allSettled([
      getPackages(),
      getRecommendations(recommendationContext()),
      getCacheStatus(),
    ]);

    const [packagesResult, recommendationsResult, statusResult] = results;

    if (packagesResult.status === "fulfilled") {
      setPackages(packagesResult.value);
    } else {
      console.error("packages loading failed:", packagesResult.reason);
    }

    if (recommendationsResult.status === "fulfilled") {
      setRecommendations(recommendationsResult.value);
    } else {
      console.error(
          "recommendations loading failed:",
          recommendationsResult.reason,
      );
    }

    if (statusResult.status === "fulfilled") {
      setStatus(statusResult.value);
    } else {
      console.error("cache status loading failed:", statusResult.reason);
    }

    setError(packagesResult.status === "rejected");

    setLoading(false);
  }, []);
  const runQuery = useCallback(async (next: PackageQuery) => {
    setQuery(next);
    setLoading(true);
    setError(false);
    try {
      setPackages(await applyPackageFilters(next));
    } catch {
      setError(true);
    } finally {
      setLoading(false);
    }
  }, []);
  const refreshAll = useCallback(async () => {
    setRefreshing(true);
    try {
      await refreshAllOperators();
      await load();
    } finally {
      setRefreshing(false);
    }
  }, [load]);
  const refreshOne = useCallback(
    async (operator: Operator) => {
      setRefreshing(true);
      try {
        await refreshOperator(operator);
        await load();
      } finally {
        setRefreshing(false);
      }
    },
    [load],
  );
  useEffect(() => {
    void load();
  }, [load]);
  return {
    packages,
    recommendations,
    status,
    loading,
    refreshing,
    error,
    query,
    runQuery,
    refreshAll,
    refreshOne,
    reload: load,
  };
}
