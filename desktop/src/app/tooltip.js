/* app/tooltip.js — the hover tip on buttons: a small dark card under the
   pointer's element with the name and, when the title ends in a "(shortcut)",
   the keys beside it. Replaces the native `title` bubble: on hover the title
   moves to `data-tip` (so the browser shows nothing) and comes back on leave.
   One delegated listener; no per-component code. `window.OMP_TIP.parse` is the
   pure part (tested). */
(function () {
  const KEYS = { cmd: "⌘", command: "⌘", meta: "⌘", shift: "⇧", alt: "⌥", option: "⌥", ctrl: "⌃", control: "⌃", enter: "↵", esc: "Esc" };

  /** "Browser (⇧⌘B)" → { name: "Browser", keys: ["⇧", "⌘", "B"] }; no shortcut → keys []. */
  function parse(title) {
    const text = String(title ?? "").trim();
    const m = /^(.*\S)\s*\(([^()]+)\)$/.exec(text);
    if (!m) return { name: text, keys: [] };
    const hint = m[2].trim();
    const looksLikeKeys = /[+⌘⇧⌥⌃↵]/.test(hint) || /^(esc|enter|tab|space|f\d+|[a-z`/,.\\[\]-])$/i.test(hint);
    if (!looksLikeKeys) return { name: text, keys: [] };
    const parts = /[+]/.test(hint) && hint.length > 1 ? hint.split("+") : [hint];
    const keys = [];
    for (const raw of parts) {
      const p = raw.trim();
      if (!p) { keys.push("+"); continue; }
      const k = KEYS[p.toLowerCase()];
      if (k) keys.push(k);
      else if (/^[⌘⇧⌥⌃]+/.test(p)) keys.push(...p.split(""));
      else keys.push(p.length === 1 ? p.toUpperCase() : p);
    }
    return { name: m[1], keys };
  }

  function install() {
    if (typeof document === "undefined" || !document.body) return;
    let tip = null;
    let timer = 0;
    let host = null;

    const hide = () => {
      clearTimeout(timer);
      if (tip) { tip.remove(); tip = null; }
      if (host) {
        const t = host.getAttribute("data-tip");
        if (t != null && !host.hasAttribute("title")) host.setAttribute("title", t);
        host.removeAttribute("data-tip");
        host = null;
      }
    };
    const show = (el, text) => {
      const { name, keys } = parse(text);
      tip = document.createElement("div");
      tip.className = "tip";
      tip.setAttribute("role", "tooltip");
      const label = document.createElement("span");
      label.className = "tip-name";
      label.textContent = name;
      tip.appendChild(label);
      if (keys.length) {
        const k = document.createElement("span");
        k.className = "tip-keys";
        k.textContent = keys.join(" ");
        tip.appendChild(k);
      }
      document.body.appendChild(tip);
      const r = el.getBoundingClientRect();
      const w = tip.offsetWidth;
      const left = Math.max(8, Math.min(window.innerWidth - w - 8, r.left + r.width / 2 - w / 2));
      const below = r.bottom + 8 + tip.offsetHeight < window.innerHeight;
      tip.style.left = `${Math.round(left)}px`;
      tip.style.top = `${Math.round(below ? r.bottom + 8 : r.top - 8 - tip.offsetHeight)}px`;
    };

    document.addEventListener("mouseover", (e) => {
      const el = e.target instanceof Element ? e.target.closest("[title], [data-tip]") : null;
      if (el === host) return;
      hide();
      if (!el) return;
      const text = el.getAttribute("title");
      if (!text) return;
      host = el;
      el.setAttribute("data-tip", text);
      el.removeAttribute("title");
      timer = setTimeout(() => { if (host === el) show(el, text); }, 350);
    }, true);
    for (const type of ["mousedown", "keydown", "scroll", "blur", "contextmenu"]) document.addEventListener(type, hide, true);
    document.addEventListener("mouseleave", hide, true);
  }

  window.OMP_TIP = { parse, install };
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", install);
  else install();
})();
