import type { Locale, TKey } from "../i18n";
import { t } from "../i18n";
import type {
  Operator,
  PackageKind,
  PackageQuery,
  PackageSort,
} from "../lib/types";
import { emptyFilter, packageQuery } from "../services/contracts";
const operators: Operator[] = ["mci", "irancell", "rightel", "samantel"];
export function FiltersPanel({
  locale,
  query,
  onChange,
}: {
  locale: Locale;
  query: PackageQuery;
  onChange: (q: PackageQuery) => void;
}) {
  const filter = query.filter;
  const setOps = (op: Operator) =>
    onChange({
      ...query,
      filter: {
        ...filter,
        operators: filter.operators.includes(op)
          ? filter.operators.filter((o) => o !== op)
          : [...filter.operators, op],
      },
    });
  return (
    <form className="filters" onSubmit={(e) => e.preventDefault()}>
      <label>
        {t(locale, "filters.search")}
        <input
          value={query.searchText ?? ""}
          placeholder={t(locale, "filters.searchPlaceholder")}
          onChange={(e) =>
            onChange(packageQuery(e.target.value, filter, query.sort))
          }
        />
      </label>
      <fieldset>
        <legend>{t(locale, "filters.operator")}</legend>
        {operators.map((op) => (
          <label className="check" key={op}>
            <input
              type="checkbox"
              checked={filter.operators.includes(op)}
              onChange={() => setOps(op)}
            />
            {t(locale, `operator.${op}` as TKey)}
          </label>
        ))}
      </fieldset>
      <label>
        {t(locale, "filters.priceMin")}
        <input
          type="number"
          onChange={(e) =>
            onChange({
              ...query,
              filter: {
                ...filter,
                minPrice: e.target.value
                  ? { amount: Number(e.target.value), currency: "irr" }
                  : null,
              },
            })
          }
        />
      </label>
      <label>
        {t(locale, "filters.priceMax")}
        <input
          type="number"
          onChange={(e) =>
            onChange({
              ...query,
              filter: {
                ...filter,
                maxPrice: e.target.value
                  ? { amount: Number(e.target.value), currency: "irr" }
                  : null,
              },
            })
          }
        />
      </label>
      <label>
        {t(locale, "filters.minData")}
        <input
          type="number"
          onChange={(e) =>
            onChange({
              ...query,
              filter: {
                ...filter,
                minGeneralDataBytes: e.target.value
                  ? Number(e.target.value) * 1024 ** 3
                  : null,
              },
            })
          }
        />
      </label>
      <label>
        {t(locale, "filters.kind")}
        <select
          onChange={(e) =>
            onChange({
              ...query,
              filter: {
                ...filter,
                packageKinds: e.target.value
                  ? [e.target.value as PackageKind]
                  : [],
              },
            })
          }
        >
          <option value="">{t(locale, "common.all")}</option>
          <option value="internet_only">
            {t(locale, "kind.internet_only")}
          </option>
          <option value="combined">{t(locale, "kind.combined")}</option>
        </select>
      </label>
      <label className="check">
        <input
          type="checkbox"
          checked={filter.includeCombined}
          onChange={(e) =>
            onChange({
              ...query,
              filter: { ...filter, includeCombined: e.target.checked },
            })
          }
        />
        {t(locale, "filters.combined")}
      </label>
      <label>
        {t(locale, "filters.sort")}
        <select
          value={query.sort ?? "newest"}
          onChange={(e) =>
            onChange({ ...query, sort: e.target.value as PackageSort })
          }
        >
          <option value="newest">{t(locale, "sort.newest")}</option>
          <option value="best_value">{t(locale, "sort.best_value")}</option>
          <option value="price_ascending">
            {t(locale, "sort.price_ascending")}
          </option>
          <option value="data_descending">
            {t(locale, "sort.data_descending")}
          </option>
        </select>
      </label>
      <button
        type="button"
        className="secondary"
        onClick={() => onChange(packageQuery("", emptyFilter(), "newest"))}
      >
        {t(locale, "action.clear")}
      </button>
    </form>
  );
}
