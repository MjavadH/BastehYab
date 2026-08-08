import { useEffect, useState } from "react";
import { getAppHealth } from "../lib/tauri";
import { directionFor, t, type Locale } from "../i18n";

export function App() {
  const [locale] = useState<Locale>("fa");
  const [coreConnected, setCoreConnected] = useState<boolean | null>(null);

  useEffect(() => {
    document.documentElement.lang = locale;
    document.documentElement.dir = directionFor(locale);
  }, [locale]);

  useEffect(() => {
    getAppHealth()
      .then((health) => setCoreConnected(health.status === "ok"))
      .catch(() => setCoreConnected(false));
  }, []);

  const ipcStatus = coreConnected === null
    ? t(locale, "status.ipcIdle")
    : coreConnected
      ? t(locale, "status.ipcReady")
      : t(locale, "status.ipcUnavailable");

  return (
    <main className="app-shell">
      <section className="hero" aria-labelledby="app-title">
        <p className="eyebrow">{t(locale, "status.foundation")}</p>
        <h1 id="app-title">{t(locale, "app.title")}</h1>
        <p>{t(locale, "app.subtitle")}</p>
        <p className="core-status">{ipcStatus}</p>
      </section>
      <nav className="workspace-grid" aria-label="BastehYab workspace">
        <a href="#packages">{t(locale, "nav.browse")}</a>
        <a href="#filters">{t(locale, "nav.filters")}</a>
        <a href="#recommendations">{t(locale, "nav.recommendations")}</a>
        <a href="#status">{t(locale, "nav.status")}</a>
        <a href="#settings">{t(locale, "nav.settings")}</a>
      </nav>
    </main>
  );
}
