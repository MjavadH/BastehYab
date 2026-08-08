import type { Locale } from "../i18n";
import type { DataAllowance, Money, Validity } from "./types";

const gb = 1024 ** 3;
export const generalBytes = (items: DataAllowance[]) =>
  items.find((a) => a.kind === "general")?.amountBytes ?? null;
export const hasKind = (items: DataAllowance[], kind: string) =>
  items.some((a) => a.kind === kind && (a.unlimited || a.amountBytes));
export function formatMoney(locale: Locale, money: Money | null): string {
  if (!money) return locale === "fa" ? "نامشخص" : "Unknown";
  return `${new Intl.NumberFormat(locale).format(money.amount)} ${money.currency === "irr" ? (locale === "fa" ? "ریال" : "IRR") : locale === "fa" ? "تومان" : "toman"}`;
}
export function formatBytes(locale: Locale, bytes: number | null): string {
  if (bytes == null) return locale === "fa" ? "نامشخص" : "Unknown";
  return `${new Intl.NumberFormat(locale, { maximumFractionDigits: 1 }).format(bytes / gb)} ${locale === "fa" ? "گیگابایت" : "GB"}`;
}
export function formatValidity(locale: Locale, validity: Validity): string {
  if (typeof validity === "string")
    return locale === "fa"
      ? validity === "unknown"
        ? "نامشخص"
        : "متفرقه"
      : validity;
  if ("days" in validity)
    return locale === "fa"
      ? `${new Intl.NumberFormat(locale).format(validity.days)} روز`
      : `${validity.days} days`;
  return locale === "fa"
    ? `${new Intl.NumberFormat(locale).format(validity.hours)} ساعت`
    : `${validity.hours} hours`;
}
export function formatTime(locale: Locale, unix: number | null): string {
  if (!unix) return locale === "fa" ? "بدون داده" : "No data";
  return new Intl.DateTimeFormat(locale, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(unix * 1000);
}
