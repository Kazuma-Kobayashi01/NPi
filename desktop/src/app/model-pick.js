// Model picking, pure: vendor names, grouping by vendor, and which models the
// small picker shows. Exposes `window.OMP_MODELS`; an IIFE (AGENTS.md "IIFE
// rule"), dependency-free.
//
// A model is `{ provider, id, name, ... }`; its key is `provider/id` (the
// same as omp's `selector` without a thinking suffix, and `modelKey` in
// adapter.js).
(function () {
  // omp's provider ids → the vendor's name. Several providers can belong to
  // one vendor (an API and a subscription login, say); they group together.
  const VENDORS = {
    anthropic: "Anthropic",
    openai: "OpenAI",
    "openai-codex": "OpenAI",
    google: "Google",
    "google-antigravity": "Google",
    "google-gemini-cli": "Google",
    "google-vertex": "Google",
    gemini: "Google",
    deepseek: "DeepSeek",
    xai: "xAI",
    mistral: "Mistral",
    groq: "Groq",
    openrouter: "OpenRouter",
    ollama: "Ollama",
    local: "Local",
    cerebras: "Cerebras",
    "github-copilot": "GitHub Copilot",
    "amazon-bedrock": "Amazon",
    bedrock: "Amazon",
    azure: "Azure",
    moonshot: "Moonshot",
    kimi: "Kimi",
    zai: "Z.AI",
    minimax: "MiniMax",
    "alibaba-cn": "Alibaba(China)",
    alibaba: "Alibaba (International)",
    dashscope: "Alibaba(China)",
  };

  const titleCase = (s) => s.replace(/\b[a-z]/g, (c) => c.toUpperCase());

  /** The vendor's display name for a provider id (`openai-codex` → `OpenAI`);
   *  an unknown id is shown title-cased (`my-gateway` → `My Gateway`). */
  function vendorLabel(provider) {
    const p = String(provider ?? "").trim();
    if (!p) return "Other";
    return VENDORS[p] ?? titleCase(p.replace(/[-_]+/g, " "));
  }

  const keyOf = (m) => `${m.provider}/${m.id}`;

  /**
   * The models the small picker lists: every favourite that is available,
   * sorted by vendor name and then model name. With no favourites, the
   * current model first and then the most recently used, so the picker is
   * never empty while models exist. The current model is always included, so
   * what is selected is always visible.
   */
  function pickerModels(models, favoriteKeys, currentKey, recentKeys) {
    const byKey = new Map(models.map((m) => [keyOf(m), m]));
    const out = [];
    const seen = new Set();
    const add = (key) => {
      const m = byKey.get(key);
      if (m && !seen.has(key)) { seen.add(key); out.push(m); }
    };
    const favorites = (favoriteKeys ?? []).filter((k) => byKey.has(k));
    if (favorites.length > 0) {
      favorites.forEach(add);
      add(currentKey);
      const name = (m) => String(m.name ?? m.id);
      return out.sort((a, b) => vendorLabel(a.provider).localeCompare(vendorLabel(b.provider), "en")
        || name(a).localeCompare(name(b), "en", { numeric: true }));
    }
    add(currentKey);
    (recentKeys ?? []).forEach(add);
    return out;
  }

  /** `[{ vendor, models }]`, vendors and their models in alphabetical order. */
  function groupByVendor(models) {
    const groups = new Map();
    for (const m of models) {
      const vendor = vendorLabel(m.provider);
      if (!groups.has(vendor)) groups.set(vendor, []);
      groups.get(vendor).push(m);
    }
    const byName = (a, b) => String(a.name ?? a.id).localeCompare(String(b.name ?? b.id), "en", { numeric: true });
    return [...groups.entries()]
      .sort(([a], [b]) => a.localeCompare(b, "en"))
      .map(([vendor, list]) => ({ vendor, models: [...list].sort(byName) }));
  }

  /** `favoriteKeys` with `key` added (at the end) or removed. */
  function toggleFavorite(favoriteKeys, key) {
    const list = Array.isArray(favoriteKeys) ? favoriteKeys : [];
    return list.includes(key) ? list.filter((k) => k !== key) : [...list, key];
  }

  // The model roster (src-tauri model_config.rs, omp's config/model-roster.ts): four tiers subagents
  // are scheduled onto, then the arbiter of a multi-expert review.
  const ROSTER = [
    ["flagship", "Flagship", "Design decisions, hard debugging, judgment calls"],
    ["high", "High", "Substantial implementation and analysis, careful review"],
    ["standard", "Standard", "Routine, well-specified work"],
    ["economy", "Economy", "Search, reading, summaries, bulk work"],
    ["arbiter", "Arbiter", "Judges a multi-expert review (Academic); the flagship when unset"],
  ];
  // Built-in roles that follow a tier while they have no model of their own.
  const FOLLOWING = [["smol", "Fast (smol)", "economy"], ["task", "Subtask (task)", "standard"], ["slow", "Thinking (slow)", "flagship"]];

  /**
   * What Settings shows of `roster` (`{ role: selector | null }`): `rows` for the tiers and the
   * arbiter, `overrides` for the built-in roles set apart from their tier, and `oneVendor` when
   * every set tier is from the same vendor (a multi-expert review then has little to compare).
   */
  function rosterView(roster) {
    const r = roster ?? {};
    const rows = ROSTER.map(([role, label, use]) => ({ role, label, use, selector: r[role] || "" }));
    const overrides = FOLLOWING.filter(([role]) => r[role]).map(([role, label, tier]) => ({ role, label, tier, selector: r[role] }));
    const vendors = new Set(rows.slice(0, 4).filter(x => x.selector).map(x => vendorLabel(x.selector.split("/")[0])));
    const set = rows.slice(0, 4).filter(x => x.selector).length;
    return { rows, overrides, oneVendor: set >= 2 && vendors.size === 1 };
  }

  window.OMP_MODELS = { vendorLabel, keyOf, pickerModels, groupByVendor, toggleFavorite, rosterView };
})();
