/* app/greetings.js — the line above the prompt on the start page.
   Half the time it is the greeting of the hour (the same for Chat and Code:
   morning, noon, afternoon, evening, past midnight), otherwise one of the
   lines of the current mode. `{name}` is the user's name; without one the
   token goes, with its comma. `greeting({ mode, name, hour, seed })` is pure:
   `seed` in [0, 1) picks the line. Regression: test-greetings.mjs. */
(function () {
  const PERIODS = {
    morning: ["Good morning, {name}", "Rise and shine, {name}", "A fresh morning, {name}", "Morning, {name}. Coffee first?"],
    noon: ["Good noon, {name}", "Lunchtime, {name}?", "Midday already, {name}", "Halfway through the day, {name}"],
    afternoon: ["Good afternoon, {name}", "Afternoon, {name}", "Good to see you, {name}. The afternoon is young", "A quiet afternoon, {name}?"],
    evening: ["Good evening, {name}", "Evening, {name}", "Winding down, {name}?", "The day is nearly done, {name}"],
    midnight: ["Night owl! {name}", "Still up, {name}?", "Burning the midnight oil, {name}", "The world is asleep, {name}"],
  };

  const BY_MODE = {
    chat: [
      "What is on your mind, {name}?", "How can I help, {name}?", "Ask me anything, {name}", "What are you curious about, {name}?",
      "Let's think it through, {name}", "Something to figure out, {name}?", "A question for me, {name}?", "What shall we talk about, {name}?",
      "Need a second opinion, {name}?", "What would you like to write or learn, {name}?", "Tell me the story, {name}", "Where shall we begin, {name}?",
    ],
    code: [
      "What shall we build, {name}?", "Which bug are we hunting, {name}?", "Ready to ship something, {name}?", "Pick a project, {name}",
      "What are we refactoring, {name}?", "What needs fixing, {name}?", "Show me the repo, {name}", "Tests first, {name}?",
      "What is the plan, {name}?", "Let's make it work, {name}", "Which file first, {name}?", "Open a folder and let's begin, {name}",
    ],
  };

  /** The period of an hour (0-23): 5-10 morning, 11-13 noon, 14-17 afternoon, 18-22 evening, else midnight. */
  function periodOf(hour) {
    const h = ((Math.floor(Number(hour)) % 24) + 24) % 24;
    if (h >= 5 && h < 11) return "morning";
    if (h >= 11 && h < 14) return "noon";
    if (h >= 14 && h < 18) return "afternoon";
    if (h >= 18 && h < 23) return "evening";
    return "midnight";
  }

  /** A template with the name filled in, or without it: `, {name}` and ` {name}` go. */
  function fill(template, name) {
    const n = String(name ?? "").trim();
    if (n) return template.replace(/\{name\}/g, n);
    return template.replace(/,\s*\{name\}/g, "").replace(/\s*\{name\}/g, "").trim();
  }

  const pickFrom = (list, r) => list[Math.min(list.length - 1, Math.floor(r * list.length))];

  function greeting({ mode = "code", name = "", hour = 12, seed = 0 } = {}) {
    const s = Math.min(0.999999, Math.max(0, Number(seed) || 0));
    const shared = s < 0.5;
    // Two independent-looking draws from one seed.
    const r = shared ? s * 2 : (s - 0.5) * 2;
    const list = shared ? PERIODS[periodOf(hour)] : BY_MODE[mode === "chat" ? "chat" : "code"];
    return fill(pickFrom(list, r), name);
  }

  window.OMP_GREETINGS = { greeting, periodOf, fill, PERIODS, BY_MODE };
})();
