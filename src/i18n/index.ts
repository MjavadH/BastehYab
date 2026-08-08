import en from "./en.json";
import fa from "./fa.json";

export type Locale = "fa" | "en";
type Messages = typeof en;
type MessageKey = keyof Messages;

const messages: Record<Locale, Messages> = { en, fa };

export function directionFor(locale: Locale): "rtl" | "ltr" {
  return locale === "fa" ? "rtl" : "ltr";
}

export function t(locale: Locale, key: MessageKey): string {
  return messages[locale][key] ?? messages.en[key];
}
