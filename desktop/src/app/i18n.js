// app/i18n.js — the UI language: `window.OMP_I18N`.
//
// `t(key, vars)`: the string of the current locale, else en-US, else the key
// itself. A key is either the English source text (most of the UI: the
// English text is its own en-US entry, so en-US needs none) or a dotted id
// (`work.thought`) whose en-US text lives in the en-US map. `{name}` in a
// string is replaced from `vars`; there is no plural machinery: write
// strings that read well for any number.
//
// Dictionaries register here from app/i18n/*.js as rows
// `[key, zh-CN, zh-TW, ja-JP]` (`add`) and, for dotted ids, `addEn({ key: text })`.
// One row per key keeps the three translated sets identical by construction.
//
// The choice is the tweak `language`: "auto" or one of LOCALES. `resolve`
// maps it, and `navigator.language` for "auto", to a supported locale;
// `app-live.jsx` calls `setLocale` on every render, so a change re-renders
// the UI at once. The setting changes this interface only, never the model's
// reply language or any system prompt. Regression: test-i18n.mjs.
(function () {
  const LOCALES = Object.freeze(["zh-CN", "zh-TW", "en-US", "ja-JP"]);
  /** Each language written in itself, for the Settings row. */
  const NAMES = Object.freeze({ "zh-CN": "简体中文", "zh-TW": "繁體中文", "en-US": "English (US)", "ja-JP": "日本語" });
  const COLUMN = { "zh-CN": 1, "zh-TW": 2, "ja-JP": 3 };
  const dict = { "zh-CN": {}, "zh-TW": {}, "en-US": {}, "ja-JP": {} };
  let current = "en-US";

  /** A browser language tag as one of LOCALES; anything unknown is en-US. */
  function detect(tag) {
    const s = String(tag ?? "").toLowerCase().replace(/_/g, "-");
    if (s.startsWith("ja")) return "ja-JP";
    if (s.startsWith("zh")) return /(^|-)(tw|hk|mo|hant)(-|$)/.test(s) ? "zh-TW" : "zh-CN";
    return "en-US";
  }

  /** The tweak value (`auto` or a locale) as a supported locale. */
  function resolve(pref, systemTag) {
    if (LOCALES.includes(pref)) return pref;
    const nav = typeof navigator !== "undefined" ? (navigator.languages?.[0] ?? navigator.language) : "";
    return detect(systemTag ?? nav);
  }

  function add(rows) {
    for (const row of rows) {
      for (const loc of Object.keys(COLUMN)) dict[loc][row[0]] = row[COLUMN[loc]];
    }
  }
  const addEn = (map) => Object.assign(dict["en-US"], map);

  const fill = (s, vars) => (vars ? s.replace(/\{(\w+)\}/g, (m, k) => (k in vars ? String(vars[k]) : m)) : s);

  /** Whether `key` has an entry in `locale` (the source text of en-US counts). */
  const has = (key, locale = current) => key in dict[locale] || (locale === "en-US" && !/^[a-z]+(\.[\w-]+)+$/.test(key));

  function t(key, vars) {
    const s = dict[current][key] ?? dict["en-US"][key] ?? key;
    return fill(s, vars);
  }

  function setLocale(locale) {
    current = LOCALES.includes(locale) ? locale : "en-US";
    if (typeof document !== "undefined") document.documentElement.lang = current;
    return current;
  }

  window.OMP_I18N = Object.freeze({
    LOCALES, NAMES, detect, resolve, add, addEn, has, t, setLocale, locale: () => current,
    dictionaries: () => dict,
  });
})();
