import type { Locale, TKey } from "../i18n";
import { t } from "../i18n";
import { formatTime } from "../lib/format";
import type { CacheStatusDto, Operator } from "../lib/types";
export function StatusPanel({
  locale,
  status,
  onRefresh,
}: {
  locale: Locale;
  status: CacheStatusDto | null;
  onRefresh: (op: Operator) => void;
}) {
  return (
    <section className="panel">
      <h2>{t(locale, "status.title")}</h2>
      <div className="status-grid">
        {status?.operators.map((op) => (
          <article className="status-card" key={op.operator}>
            <b>{t(locale, `operator.${op.operator}` as TKey)}</b>
            <span
              className={
                op.refreshing
                  ? "updating"
                  : op.freshness === "stale"
                    ? "stale"
                    : op.available
                      ? "available"
                      : "unavailable"
              }
            >
              {op.refreshing
                ? t(locale, "status.updating")
                : op.freshness === "stale"
                  ? t(locale, "status.stale")
                  : op.available
                    ? t(locale, "status.available")
                    : t(locale, "status.unavailable")}
            </span>
            <small>
              {op.packageCount} · {t(locale, "status.updated")}{" "}
              {formatTime(locale, op.lastSuccessfulUpdateUnixSeconds)}
            </small>
            <button
              className="secondary"
              onClick={() => onRefresh(op.operator)}
            >
              {t(locale, "action.refreshAll")}
            </button>
          </article>
        ))}
      </div>
    </section>
  );
}
