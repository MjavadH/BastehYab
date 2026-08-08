import type { Locale, TKey } from "../i18n";
import { t } from "../i18n";
import {
  formatBytes,
  formatMoney,
  formatValidity,
  generalBytes,
} from "../lib/format";
import type { PackageDto } from "../lib/types";
export function Comparison({
  locale,
  packages,
  onRemove,
}: {
  locale: Locale;
  packages: PackageDto[];
  onRemove: (id: string) => void;
}) {
  return (
    <section className="panel">
      <h2>{t(locale, "compare.title")}</h2>
      {packages.length === 0 ? (
        <p>{t(locale, "compare.empty")}</p>
      ) : (
        <div className="compare-grid">
          {packages.map((p) => (
            <article className="compare-card" key={p.id}>
              <button className="ghost" onClick={() => onRemove(p.id)}>
                ×
              </button>
              <h3>{p.name}</h3>
              <dl>
                <dt>{t(locale, "filters.operator")}</dt>
                <dd>{t(locale, `operator.${p.operator}` as TKey)}</dd>
                <dt>{t(locale, "filters.priceMax")}</dt>
                <dd>{formatMoney(locale, p.price)}</dd>
                <dt>{t(locale, "filters.minData")}</dt>
                <dd>{formatBytes(locale, generalBytes(p.dataAllowances))}</dd>
                <dt>{t(locale, "filters.validity")}</dt>
                <dd>{formatValidity(locale, p.validity)}</dd>
                <dt>{t(locale, "filters.kind")}</dt>
                <dd>{t(locale, `kind.${p.packageKind}` as TKey)}</dd>
              </dl>
            </article>
          ))}
        </div>
      )}
    </section>
  );
}
