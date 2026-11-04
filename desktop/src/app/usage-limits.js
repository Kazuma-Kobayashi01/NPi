// app/usage-limits.js — the subscription bars in Settings → Usage, from `usage_limits`
// (model_config.rs): names, the order of the bars and when each resets.
(function () {
  const NAMES = { anthropic: "Claude", "openai-codex": "ChatGPT", "google-antigravity": "Antigravity", "github-copilot": "GitHub Copilot", gemini: "Gemini", kimi: "Kimi", cursor: "Cursor" };

  const providerName = id => NAMES[id] ?? id;

  /** A limit's name: the provider's prefix goes, and a label that does not say its window gets it. */
  function limitLabel(limit, provider) {
    const name = providerName(provider);
    const bare = limit.label.startsWith(`${name} `) ? limit.label.slice(name.length + 1) : limit.label;
    if (/\d|hour|day|week|month/i.test(bare) || !limit.windowId) return bare;
    return `${bare} · ${limit.windowId}`;
  }

  /** How much is used, 0 to 1; null when the provider says neither way (credits, say). */
  function usedOf(limit) {
    if (typeof limit.usedFraction === "number") return Math.max(0, Math.min(1, limit.usedFraction));
    return typeof limit.remainingFraction === "number" ? Math.max(0, Math.min(1, 1 - limit.remainingFraction)) : null;
  }

  /** `{ key, time }` for "Resets …": the time alone today, the weekday and time within a week, else the date. */
  function resetText(resetsAt, now, locale) {
    if (!resetsAt) return null;
    const when = new Date(resetsAt);
    const time = when.toLocaleTimeString(locale, { hour: "numeric", minute: "2-digit" });
    if (when.toDateString() === new Date(now).toDateString()) return { key: "Resets at {time}", time };
    const days = (resetsAt - now) / 86400000;
    const day = days < 7
      ? when.toLocaleDateString(locale, { weekday: "long" })
      : when.toLocaleDateString(locale, { month: "short", day: "numeric" });
    return { key: "Resets {time}", time: `${day} ${time}` };
  }

  /** One account as bars, shortest window first. A quota two model families share is reported twice: it shows once. */
  function bars(account, now, locale) {
    const seen = new Set();
    const shown = (account.limits ?? []).filter(l => {
      const key = `${l.label}|${l.windowId}|${l.resetsAt}`;
      if (usedOf(l) === null || seen.has(key)) return false;
      seen.add(key);
      return true;
    });
    return shown
      .sort((a, b) => (a.durationMs ?? Infinity) - (b.durationMs ?? Infinity))
      .map((l, index) => ({
        key: `${index}:${l.label}`,
        label: limitLabel(l, account.provider),
        percent: Math.round(usedOf(l) * 100),
        status: l.status ?? "unknown",
        reset: resetText(l.resetsAt, now, locale),
      }));
  }

  /** The heading of an account: its service and plan. */
  function heading(account) {
    const plan = account.plan ? ` ${account.plan[0].toUpperCase()}${account.plan.slice(1)}` : "";
    return `${providerName(account.provider)}${plan}`;
  }

  window.OMP_USAGE_LIMITS = { providerName, limitLabel, usedOf, resetText, bars, heading };
})();
