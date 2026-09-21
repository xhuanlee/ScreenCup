import { create } from "zustand";

import { api, type Lang } from "../lib/api";
import { en } from "./locales/en";
import { zh } from "./locales/zh";
import type { Translations } from "./locales/zh";

export type { Lang };

/** Dot path to any string leaf, e.g. "record.startHint". */
type KeyPath<T> = T extends object
  ? {
      [K in keyof T & string]: T[K] extends string ? K : `${K}.${KeyPath<T[K]>}`;
    }[keyof T & string]
  : never;

export type TKey = KeyPath<Translations>;

const DICTS: Record<Lang, Translations> = { zh, en };

interface I18nState {
  lang: Lang;
  /** Hydrate from persisted settings. Unknown values keep the current lang. */
  init: (lang: Lang | null | undefined) => void;
  /** Switch language and persist it alongside the other settings. */
  setLang: (lang: Lang) => Promise<void>;
}

export const useI18n = create<I18nState>((set) => ({
  lang: "zh",
  init: (lang) => {
    if (lang === "zh" || lang === "en") set({ lang });
  },
  setLang: async (lang) => {
    set({ lang });
    // Language is a UI nicety: never block the app on persisting it.
    try {
      const settings = await api.getSettings();
      await api.saveSettings({ ...settings, language: lang });
    } catch {
      /* ignore */
    }
  },
}));

function resolve(lang: Lang, key: TKey): string {
  const parts = key.split(".");
  let node: unknown = DICTS[lang];
  for (const part of parts) {
    if (node && typeof node === "object" && part in node) {
      node = (node as Record<string, unknown>)[part];
    } else {
      // Fall back to Chinese, which always has every key.
      node = undefined;
      break;
    }
  }
  if (typeof node === "string") return node;
  let fallback: unknown = DICTS.zh;
  for (const part of parts) {
    fallback = (fallback as Record<string, unknown>)?.[part];
  }
  return typeof fallback === "string" ? fallback : key;
}

/** Replace `{name}` placeholders in a resolved string. */
function interpolate(text: string, vars?: Record<string, string>): string {
  if (!vars) return text;
  return text.replace(/\{(\w+)\}/g, (m, name: string) =>
    Object.prototype.hasOwnProperty.call(vars, name) ? vars[name]! : m,
  );
}

/**
 * Translate outside React (store actions, formatters). Reads the current
 * language without subscribing, so it is safe to call anywhere.
 */
export function tr(key: TKey, vars?: Record<string, string>): string {
  return interpolate(resolve(useI18n.getState().lang, key), vars);
}

/**
 * Translate inside components. Re-renders when the language flips.
 */
export function useT(): (key: TKey, vars?: Record<string, string>) => string {
  const lang = useI18n((s) => s.lang);
  return (key, vars) => interpolate(resolve(lang, key), vars);
}
