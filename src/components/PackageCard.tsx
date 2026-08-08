import type { Locale, TKey } from "../i18n";
import { t } from "../i18n";
import {
  formatBytes,
  formatMoney,
  formatValidity,
  generalBytes,
  hasKind,
} from "../lib/format";
import type { PackageDto } from "../lib/types";
export function PackageCard({
  pkg,
  locale,
  selected,
  onCompare,
  labels = [],
}: {
  pkg: PackageDto;
  locale: Locale;
  selected?: boolean;
  onCompare?: () => void;
  labels?: string[];
}) {
  const benefits = [
    pkg.voice && t(locale, "benefits.voice"),
    pkg.sms && t(locale, "benefits.sms"),
    hasKind(pkg.dataAllowances, "night") && t(locale, "benefits.night"),
  ].filter((item): item is string => Boolean(item));
  return (
    <article className="package-card">
      <div className="card-top">
        <span className={`op op-${pkg.operator}`}>
          {t(locale, `operator.${pkg.operator}` as TKey)}
        </span>
        <span>{t(locale, `kind.${pkg.packageKind}` as TKey)}</span>
      </div>
      <h3>{pkg.name}</h3>
      <div className="metrics">
        <b>{formatMoney(locale, pkg.price)}</b>
        <b>{formatBytes(locale, generalBytes(pkg.dataAllowances))}</b>
        <b>{formatValidity(locale, pkg.validity)}</b>
      </div>
      <div className="chips">
        {benefits.map((b) => (
          <span key={b}>{b}</span>
        ))}
        {labels.map((l) => (
          <span className="recommend" key={l}>
            {l}
          </span>
        ))}
      </div>
      {onCompare && (
        <button
          className="secondary"
          aria-pressed={selected}
          onClick={onCompare}
        >
          {selected ? t(locale, "action.remove") : t(locale, "action.compare")}
        </button>
      )}
    </article>
  );
}
