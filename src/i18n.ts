// Interface strings, shared with the native menu through locales/*.json.

import en from "../locales/en.json";
import it from "../locales/it.json";

export type Key = keyof typeof en;

let table: Record<Key, string> = en;
let locale = "en";

export function setLang(lang: string): void {
  locale = lang === "it" ? "it" : "en";
  table = locale === "it" ? it : en;
  document.documentElement.lang = locale;
}

export function t(key: Key, vars: Record<string, string | number> = {}): string {
  return (table[key] ?? en[key]).replace(/\{(\w+)\}/g, (_, name: string) =>
    String(vars[name] ?? ""),
  );
}

/** Picks the `.one` or `.other` form of `base` and fills in `{n}`. */
export function plural(base: "stats.added" | "stats.removed" | "fold.show", n: number): string {
  const key = `${base}.${n === 1 ? "one" : "other"}` as Key;
  return t(key, { n: formatNumber(n) });
}

export function formatNumber(n: number): string {
  return n.toLocaleString(locale);
}

/** Fills `data-i18n` (text) and `data-i18n-title` (tooltip and label) attributes. */
export function translate(root: ParentNode): void {
  for (const el of root.querySelectorAll<HTMLElement>("[data-i18n]")) {
    el.textContent = t(el.dataset.i18n as Key);
  }
  for (const el of root.querySelectorAll<HTMLElement>("[data-i18n-title]")) {
    const text = t(el.dataset.i18nTitle as Key);
    el.title = text;
    el.setAttribute("aria-label", text);
  }
}
