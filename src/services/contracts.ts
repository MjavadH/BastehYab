import type {
  PackageFilter,
  PackageQuery,
  RecommendationContext,
} from "../lib/types";
export const emptyFilter = (): PackageFilter => ({
  operators: [],
  simTypes: [],
  minPrice: null,
  maxPrice: null,
  minGeneralDataBytes: null,
  minTotalUsableDataBytes: null,
  validity: null,
  packageKinds: [],
  includeCombined: true,
  trafficKinds: [],
});
export const packageQuery = (
  searchText: string,
  filter: PackageFilter,
  sort: PackageQuery["sort"],
): PackageQuery => ({ searchText: searchText.trim() || null, filter, sort });
export const recommendationContext = (): RecommendationContext => ({
  filters: {
    operators: [],
    simTypes: [],
    packageKinds: [],
    minPrice: null,
    maxPrice: null,
    minGeneralDataBytes: null,
    maxGeneralDataBytes: null,
    validity: null,
    generalInternetOnly: false,
    includeCombined: true,
  },
  budget: null,
  preferredValidity: null,
  requiredGeneralDataBytes: null,
  includeCombined: true,
  limit: 1,
});
