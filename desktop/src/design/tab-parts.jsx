/* ═════════════════════════════════════════════════════════════════════
   tab-parts.jsx — the two small pieces of a tab row the sidebar shares:
   the run-state dot and the `folder · title` label. (The grouped tab-bar
   chip that used to live here went with the tab bar.)
   ═════════════════════════════════════════════════════════════════════ */

const { tabLabel } = window.OMP_PROJECT_NAV;

/** A tab's (or group's) run-state dot; nothing while idle. Shared by the
 *  tab bar and the project sidebar so the titles stay in one place. */
function TabRunDot({ state }) {
  if (!state || state === "idle") return null;
  const title = state === "waiting-user" ? "waiting for you"
    : state === "failed" ? "agent process exited"
    : state === "background" ? "background job running · the agent resumes when it finishes"
    : state === "retrying" ? "retrying a failed request"
    : "running";
  return <span className={`tab-run-dot ${state}`} title={title} />;
}

/** Name of a tab shown on its own (#46): `folder · title` once the title
 *  is no longer the folder name, the folder truncating first. Shared by the
 *  tab bar and the project sidebar; `className` is the caller's name class. */
function TabLabel({ tab, className, title }) {
  const { prefix, title: name } = tabLabel(tab);
  if (!prefix) return <span className={className} title={title}>{name}</span>;
  return (
    <span className={`${className} tab-label`} title={title}>
      {/* A few letters of the folder stay visible however long the title;
          a shorter folder keeps its own width, not a padded 4ch box. */}
      <span className="tab-prefix" style={{ minWidth: `${Math.min(4, prefix.length)}ch` }}>{prefix}</span>
      <span className="tab-sep">·</span>
      <span className="tab-title">{name}</span>
    </span>
  );
}


Object.assign(window, { TabRunDot, TabLabel });
