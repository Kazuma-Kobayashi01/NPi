/* design/settings/rows-detail.jsx — Settings → Appearance's custom row
   `uiDetail` (window.OMP_SETTINGS_ROWS): the five interface-detail steps
   (app/ui-detail.js). Picking one writes the step and its tweak bundle. */

function _SRD_Detail({ row, ctx, query }) {
  const { SettingsRow: Row } = window;
  const D = window.OMP_UI_DETAIL;
  const cur = D.clamp(ctx.t.uiDetail);
  const pick = (l) => ctx.setTweak({ uiDetail: l.level, ...l.patch });
  return (
    <Row stack label={row.label} desc={D.levelOf(cur).note} query={query}>
      <div className="st-detail" role="radiogroup" aria-label="Interface detail">
        {D.LEVELS.map(l => (
          <button key={l.level} type="button" role="radio" aria-checked={cur === l.level}
            className={`st-detail-step${cur === l.level ? " on" : ""}`} onClick={() => pick(l)}>
            <span className="st-detail-n">{l.level}</span>
            <span className="st-detail-name">{l.name}</span>
          </button>
        ))}
      </div>
    </Row>
  );
}

Object.assign(window.OMP_SETTINGS_ROWS = window.OMP_SETTINGS_ROWS || {}, { uiDetail: _SRD_Detail });
