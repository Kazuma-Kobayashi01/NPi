// The quick-ask window's page (src-tauri/src/quick_ask.rs). Plain script: this window has no bundler,
// and it may only call quickask_submit and quickask_hide.
(function () {
  const TEXT = {
    "en-US": { ask: "What can I help you with today?", new: "New Chat", current: "Current chat" },
    "zh-CN": { ask: "有什么可以帮你的？", new: "新对话", current: "当前对话" },
    "zh-TW": { ask: "有什麼可以幫你的？", new: "新對話", current: "目前對話" },
    "ja-JP": { ask: "何をお手伝いしましょうか？", new: "新しいチャット", current: "現在のチャット" },
  };
  let prefs = {};
  try { prefs = JSON.parse(localStorage.getItem("omp-desktop:tweaks") || "{}"); } catch { /* defaults */ }
  const lang = TEXT[prefs.language] ? prefs.language
    : /^zh-(tw|hk)/i.test(navigator.language) ? "zh-TW" : /^zh/i.test(navigator.language) ? "zh-CN"
    : /^ja/i.test(navigator.language) ? "ja-JP" : "en-US";
  const t = TEXT[lang];
  const root = document.documentElement;
  if (prefs.theme === "light") root.dataset.theme = "light";
  const backdrop = new URLSearchParams(location.search).get("backdrop");
  if (backdrop === "glass" || backdrop === "vibrancy") root.dataset.backdrop = backdrop;
  if (/^#[0-9a-f]{3,8}$/i.test(prefs.accent || "")) root.style.setProperty("--accent", prefs.accent);

  const input = document.getElementById("q");
  const send = document.getElementById("send");
  const targetButton = document.getElementById("target");
  const menu = document.getElementById("menu");
  let target = "new";
  const invoke = (name, args) => window.__TAURI__.core.invoke(name, args);

  input.placeholder = t.ask;
  menu.querySelector('[data-target="new"]').textContent = t.new;
  menu.querySelector('[data-target="current"]').textContent = t.current;
  const showTarget = () => {
    document.getElementById("target-label").textContent = t[target];
    menu.querySelectorAll("li").forEach(li => li.classList.toggle("on", li.dataset.target === target));
  };
  const setMenu = open => { menu.hidden = !open; targetButton.setAttribute("aria-expanded", String(open)); };
  const submit = async () => {
    const text = input.value.trim();
    if (!text) return;
    try { await invoke("quickask_submit", { text, target }); input.value = ""; send.disabled = true; } catch (error) { console.error(error); }
  };

  input.addEventListener("input", () => { send.disabled = input.value.trim() === ""; });
  input.addEventListener("keydown", e => {
    if (e.key === "Enter" && !e.isComposing && !e.shiftKey) { e.preventDefault(); submit(); }
  });
  document.addEventListener("keydown", e => {
    if (e.key !== "Escape") return;
    e.preventDefault();
    if (!menu.hidden) setMenu(false); else invoke("quickask_hide");
  });
  send.addEventListener("click", submit);
  targetButton.addEventListener("click", () => setMenu(menu.hidden));
  menu.addEventListener("click", e => {
    const li = e.target.closest("li");
    if (!li) return;
    target = li.dataset.target; showTarget(); setMenu(false); input.focus();
  });
  document.addEventListener("mousedown", e => { if (!menu.hidden && !e.target.closest(".target")) setMenu(false); });
  const focus = () => { setMenu(false); input.focus(); input.select(); };
  window.addEventListener("focus", focus);
  window.__TAURI__?.event?.listen("quickask://focus", focus);
  showTarget();
  input.focus();
})();
