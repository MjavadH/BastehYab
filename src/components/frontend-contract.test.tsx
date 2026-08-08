import { PackageCard } from "./PackageCard";
import { FiltersPanel } from "./FiltersPanel";
import { Recommendations } from "./Recommendations";
import { Comparison } from "./Comparison";
import {
  packageQuery,
  emptyFilter,
  recommendationContext,
} from "../services/contracts";
import type { PackageDto, RecommendationSet } from "../lib/types";

const samplePackage: PackageDto = {
  id: "mci:test",
  operator: "mci",
  externalId: "test",
  name: "Test package",
  price: { amount: 100000, currency: "irr" },
  validity: { days: 30 },
  dataAllowances: [
    {
      amountBytes: 10 * 1024 ** 3,
      unlimited: false,
      kind: "general",
      description: null,
    },
  ],
  voice: { minutes: 100, unlimited: false },
  sms: null,
  simTypes: ["prepaid"],
  packageKind: "combined",
  availability: "available",
  fetchedAtUnixSeconds: 1,
};
const stalePackage: PackageDto = {
  ...samplePackage,
  id: "irancell:stale",
  operator: "irancell",
  fetchedAtUnixSeconds: null,
};
const recommendationSet: RecommendationSet = {
  strategy: "best_value",
  inputCount: 2,
  filteredCount: 2,
  eligibleCount: 1,
  results: [
    {
      strategy: "best_value",
      packageId: samplePackage.id,
      rank: 1,
      score: { kind: "ratio", numerator: 100000, denominator: 10 * 1024 ** 3 },
      metrics: {
        priceIrr: 100000,
        generalDataBytes: 10 * 1024 ** 3,
        nightDataBytes: null,
        hasUnlimitedGeneralData: false,
        hasUnlimitedNightData: false,
        validityDays: 30,
        packageKind: "combined",
        hasVoice: true,
        hasSms: false,
        valueRatio: { priceIrr: 100000, dataBytes: 10 * 1024 ** 3 },
        trafficKind: "general",
      },
      reasons: [{ kind: "best_value_ratio" }],
    },
  ],
};

export const componentContracts = {
  packageCard: (
    <PackageCard pkg={samplePackage} locale="en" labels={["Best"]} />
  ),
  filters: (
    <FiltersPanel
      locale="fa"
      query={packageQuery("", emptyFilter(), "newest")}
      onChange={() => undefined}
    />
  ),
  recommendations: (
    <Recommendations
      locale="en"
      sets={[recommendationSet]}
      packages={[samplePackage]}
    />
  ),
  comparison: (
    <Comparison
      locale="fa"
      packages={[samplePackage, stalePackage]}
      onRemove={() => undefined}
    />
  ),
  emptyComparison: (
    <Comparison locale="en" packages={[]} onRemove={() => undefined} />
  ),
  recommendationContext: recommendationContext(),
};
