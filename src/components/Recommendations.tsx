import type { Locale, TKey } from "../i18n";
import { t } from "../i18n";
import { formatBytes, formatMoney } from "../lib/format";
import type { PackageDto, RecommendationSet } from "../lib/types";
import { PackageCard } from "./PackageCard";
const label: Record<string, TKey> = {
  best_value: "dashboard.bestValue",
  best_monthly: "dashboard.bestMonthly",
  highest_volume: "dashboard.highestVolume",
  cheapest_useful: "sort.price_ascending",
  best_night: "benefits.night",
  best_combined: "kind.combined",
};
export function Recommendations({
  locale,
  sets,
  packages,
}: {
  locale: Locale;
  sets: RecommendationSet[];
  packages: PackageDto[];
}) {
  return (
    <section className="panel">
      <h2>{t(locale, "recommendations.title")}</h2>
      <div className="recommend-grid">
        {sets.map((set) => {
          const rec = set.results[0];
          const pkg = packages.find((p) => p.id === rec?.packageId);
          return (
            <div className="recommendation" key={set.strategy}>
              <h3>{t(locale, label[set.strategy])}</h3>
              {pkg && rec ? (
                <>
                  <PackageCard
                    pkg={pkg}
                    locale={locale}
                    labels={[`#${rec.rank}`]}
                  />
                  <p>
                    <b>{t(locale, "recommendations.explanation")}:</b>{" "}
                    {formatBytes(
                      locale,
                      rec.metrics.generalDataBytes ??
                        rec.metrics.nightDataBytes,
                    )}{" "}
                    ·{" "}
                    {formatMoney(
                      locale,
                      rec.metrics.priceIrr
                        ? { amount: rec.metrics.priceIrr, currency: "irr" }
                        : null,
                    )}{" "}
                    · {set.eligibleCount}/{set.filteredCount}
                  </p>
                </>
              ) : (
                <p>{t(locale, "recommendations.none")}</p>
              )}
            </div>
          );
        })}
      </div>
    </section>
  );
}
