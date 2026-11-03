// app/notify.js — the pure side of OS notifications (docs/architecture/
// notifications.md): which Settings switches Rust gets, the language of the
// banner, and how the Settings row words the permission state. Rust decides
// whether a notification goes out (src-tauri/src/notify/); this only maps
// tweaks to its config and permission states to text.
//
// Exposes `window.OMP_NOTIFY`; IIFE per the project rule. Regression:
// test-notify.mjs.
(function () {
  /** The languages Rust has banner text for (notify/policy.rs `Lang`). */
  const LANGS = Object.freeze(["zh-CN", "zh-TW", "en-US", "ja-JP"]);

  /** The banner language: the chosen interface language when Rust speaks it
   *  (`"auto"`, or one it does not, follows the system language), else English. */
  function resolveLang(pref, systemLang) {
    if (LANGS.includes(pref)) return pref;
    const tag = String(systemLang ?? "").toLowerCase();
    if (tag.startsWith("ja")) return "ja-JP";
    if (tag.startsWith("zh")) return /(tw|hk|mo|hant)/.test(tag) ? "zh-TW" : "zh-CN";
    return "en-US";
  }

  /** The `notify_configure` arguments for a tweaks object; a missing or
   *  malformed value never turns a switch on (category switches default on,
   *  the master switch off). */
  function configFrom(tweaks, systemLang) {
    const on = (v, dflt) => (typeof v === "boolean" ? v : dflt);
    return {
      enabled: on(tweaks?.notifications, false),
      done: on(tweaks?.notifyDone, true),
      ask: on(tweaks?.notifyAsk, true),
      failed: on(tweaks?.notifyFailed, true),
      lang: resolveLang(tweaks?.language, systemLang),
    };
  }

  /** What the Settings row says about the system permission (`notify_status`). */
  function describe(status) {
    switch (status) {
      case "granted": return { text: "Allowed by the system", tone: "ok", canTest: true, canAsk: false };
      case "denied": return { text: "Blocked. Allow NPi Desktop in System Settings → Notifications.", tone: "warn", canTest: false, canAsk: false };
      case "notDetermined": return { text: "Not asked yet. Turning this on asks the system for permission.", tone: "idle", canTest: false, canAsk: true };
      case "unsupported": return { text: "Not available in an unpackaged dev build.", tone: "warn", canTest: false, canAsk: false };
      default: return { text: "Checking…", tone: "idle", canTest: false, canAsk: false };
    }
  }

  window.OMP_NOTIFY = { LANGS, resolveLang, configFrom, describe };
})();
