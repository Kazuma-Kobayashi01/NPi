/* ═════════════════════════════════════════════════════════════════════
   design/terminal-panel.jsx — one tab's terminal panel: a head (shell,
   folder, exit state, close) and the node xterm mounts into. It is the
   stage's right-most column (its width is the `term` pane, resized by the
   handle app-live.jsx puts on its left edge). Hidden tabs and collapsed
   panels stay mounted (`hidden`), so the xterm keeps its output;
   app/use-terminal.jsx owns the xterm and hands over `mountRef`;
   `onContextMenu` is the body's right-click (copy the selection).
   `TerminalPanels` renders the panels of every tab for app-live.jsx.
   ═════════════════════════════════════════════════════════════════════ */

const { Icon: _TP_Icon } = window;
const {
  isExited: _TP_isExited, entryOf: _TP_entryOf, shellLabel: _TP_shellLabel,
} = window.OMP_TERMINAL;

function TerminalPanel({ entry, hidden, onClose, mountRef, onContextMenu, shellLabel, cwd }) {
  return (
    <section className="term-panel" hidden={hidden}>
      <div className="term-head">
        <_TP_Icon name="terminal" size={11} color="var(--fg-3)" />
        <span className="term-head-shell">{shellLabel}</span>
        {cwd && <span className="term-head-cwd" title={cwd}>{cwd}</span>}
        {_TP_isExited(entry) && (
          <span className="term-head-exited" title={entry.error ?? undefined}>
            {entry.error != null ? "failed to start"
              : `exited${entry.status.exitCode == null ? "" : ` (${entry.status.exitCode})`}`}
          </span>
        )}
        <button type="button" className="btn icon ghost term-head-close" title="Hide terminal" onClick={onClose}>
          <_TP_Icon name="close" size={9} />
        </button>
      </div>
      <div className="term-body" ref={mountRef} onContextMenu={onContextMenu} />
    </section>
  );
}

/**
 * One panel per tab that has ever opened its terminal (`created`; a pending
 * or failed first spawn included). Only the active tab's open panel shows;
 * the others stay mounted but hidden, so a shell's output keeps arriving
 * off-screen and its xterm survives a hide. `terminal` is useTerminal's
 * return value, `activeId` the active tab's id.
 */
function TerminalPanels({ sessions, activeId, terminal }) {
  return (
    <>
      {sessions.map((s) => {
        const entry = _TP_entryOf(terminal.state, s.id);
        if (!entry.created) return null;
        return (
          <TerminalPanel key={s.id}
            entry={entry}
            hidden={s.id !== activeId || !entry.open}
            onClose={() => terminal.close(s.id)}
            mountRef={terminal.mountFor(s.id)}
            onContextMenu={(e) => { if (terminal.copySelection(s.id)) e.preventDefault(); }}
            shellLabel={_TP_shellLabel(entry.status?.shell)}
            cwd={s.path}
          />
        );
      })}
    </>
  );
}

Object.assign(window, { TerminalPanels });
