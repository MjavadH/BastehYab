import { useEffect, useMemo, useState } from "react";
import { directionFor, t, type Locale, type TKey } from "../i18n";
import { Comparison } from "../components/Comparison";
import { FiltersPanel } from "../components/FiltersPanel";
import { PackageCard } from "../components/PackageCard";
import { Recommendations } from "../components/Recommendations";
import { StatusPanel } from "../components/StatusPanel";
import { useAppData } from "../hooks/useAppData";
import { getAppHealth } from "../lib/tauri";

type Page = "dashboard" | "packages" | "compare" | "status" | "settings";
type Theme = "light" | "dark";
const pages: Page[] = [
  "dashboard",
  "packages",
  "compare",
  "status",
  "settings",
];

export function App() {
  const [locale, setLocale] = useState<Locale>("fa");
  const [theme, setTheme] = useState<Theme>("light");
  const [page, setPage] = useState<Page>("dashboard");
  const [coreConnected, setCoreConnected] = useState<boolean | null>(null);
  const [compareIds, setCompareIds] = useState<string[]>([]);
  const data = useAppData();
  useEffect(() => {
    document.documentElement.lang = locale;
    document.documentElement.dir = directionFor(locale);
    document.documentElement.dataset.theme = theme;
  }, [locale, theme]);
  useEffect(() => {
    getAppHealth()
      .then((h) => setCoreConnected(h.status === "ok"))
      .catch(() => setCoreConnected(false));
  }, []);
  const compared = useMemo(
    () => data.packages.filter((p) => compareIds.includes(p.id)),
    [compareIds, data.packages],
  );
  const toggleCompare = (id: string) =>
    setCompareIds((ids) =>
      ids.includes(id)
        ? ids.filter((x) => x !== id)
        : ids.length < 3
          ? [...ids, id]
          : ids,
    );
  const ipcStatus =
    coreConnected === null
      ? t(locale, "status.ipcIdle")
      : coreConnected
        ? t(locale, "status.ipcReady")
        : t(locale, "status.ipcUnavailable");
  return (
    <div className="desktop-shell">
      <aside className="sidebar">
        <div>
          <p className="eyebrow">{t(locale, "status.foundation")}</p>
          <h1>{t(locale, "app.title")}</h1>
          <p>{t(locale, "app.subtitle")}</p>
          <small>{ipcStatus}</small>
        </div>
        <nav aria-label="Main navigation">
          {pages.map((p) => (
            <button
              className={page === p ? "active" : ""}
              key={p}
              onClick={() => setPage(p)}
            >
              {t(locale, `nav.${p}` as TKey)}
            </button>
          ))}
        </nav>
        <button onClick={data.refreshAll} disabled={data.refreshing}>
          {data.refreshing
            ? t(locale, "status.updating")
            : t(locale, "action.refreshAll")}
        </button>
      </aside>
      <main className="content">
        {data.error && (
          <div className="error" role="alert">
            <b>{t(locale, "packages.error")}</b>
            <button onClick={data.reload}>{t(locale, "action.retry")}</button>
          </div>
        )}
        {page === "dashboard" && (
          <>
            <section className="hero">
              <h2>{t(locale, "dashboard.title")}</h2>
              <p>{t(locale, "dashboard.subtitle")}</p>
            </section>
            {data.loading ? (
              <p className="loading">{t(locale, "packages.loading")}</p>
            ) : (
              <Recommendations
                locale={locale}
                sets={data.recommendations}
                packages={data.packages}
              />
            )}
            <StatusPanel
              locale={locale}
              status={data.status}
              onRefresh={data.refreshOne}
            />
          </>
        )}
        {page === "packages" && (
          <>
            <section className="panel">
              <h2>{t(locale, "packages.title")}</h2>
              <FiltersPanel
                locale={locale}
                query={data.query}
                onChange={data.runQuery}
              />
            </section>
            {data.loading ? (
              <p className="loading">{t(locale, "packages.loading")}</p>
            ) : data.packages.length === 0 ? (
              <p className="empty">{t(locale, "packages.empty")}</p>
            ) : (
              <div className="package-grid">
                {data.packages.map((pkg) => (
                  <PackageCard
                    key={pkg.id}
                    pkg={pkg}
                    locale={locale}
                    selected={compareIds.includes(pkg.id)}
                    onCompare={() => toggleCompare(pkg.id)}
                  />
                ))}
              </div>
            )}
          </>
        )}
        {page === "compare" && (
          <Comparison
            locale={locale}
            packages={compared}
            onRemove={toggleCompare}
          />
        )}
        {page === "status" && (
          <StatusPanel
            locale={locale}
            status={data.status}
            onRefresh={data.refreshOne}
          />
        )}
        {page === "settings" && (
          <section className="panel settings">
            <h2>{t(locale, "settings.title")}</h2>
            <label>
              {t(locale, "settings.language")}
              <select
                value={locale}
                onChange={(e) => setLocale(e.target.value as Locale)}
              >
                <option value="fa">فارسی</option>
                <option value="en">English</option>
              </select>
            </label>
            <label>
              {t(locale, "settings.theme")}
              <select
                value={theme}
                onChange={(e) => setTheme(e.target.value as Theme)}
              >
                <option value="light">{t(locale, "settings.light")}</option>
                <option value="dark">{t(locale, "settings.dark")}</option>
              </select>
            </label>
            <p>{t(locale, "settings.about")}</p>
          </section>
        )}
      </main>
    </div>
  );
}
