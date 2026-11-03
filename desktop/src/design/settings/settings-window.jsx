/* design/settings/settings-window.jsx — Settings in its floating window: the
   frame with the traffic lights (a head that drags it, ui/window-chrome.js)
   around the page. It exists while Settings is open, so the window's position
   and fold state last exactly that long. */

function SettingsWindow({ ctx, onClose }) {
  const T = window.OMP_I18N.t;
  const panelRef = React.useRef(null);
  const wc = window.useWindowChrome(onClose, panelRef);
  return (
    <div className="st-overlay" onMouseDown={e => { if (e.target === e.currentTarget) onClose(); }}>
      <div className="st-modal" ref={panelRef} role="dialog" aria-label={T("Settings")}>
        <div className="st-modal-head" {...wc.headProps}>
          {wc.lights}
          <span className="st-modal-title">{T("Settings")}</span>
        </div>
        <div className="st-modal-body">
          <SettingsPage ctx={ctx} />
        </div>
      </div>
    </div>
  );
}

Object.assign(window, { SettingsWindow });
