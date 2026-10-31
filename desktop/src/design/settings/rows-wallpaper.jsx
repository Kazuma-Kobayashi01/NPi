/* design/settings/rows-wallpaper.jsx — Settings → General's custom row
   `wallpaperArt` (window.OMP_SETTINGS_ROWS): which artwork the chat wallpaper
   shows — "Auto" gives each conversation its own — and who made it. */

function _SRW_Art({ row, ctx, query }) {
  const { SettingsRow: Row } = window;
  const W = window.OMP_WALLPAPERS;
  const art = ctx.t.wallpaperArt ?? "auto";
  const chosen = W.LIST.find(a => a.id === art);
  return (
    <Row label={row.label} query={query}
      desc={chosen ? `${chosen.title} · ${chosen.artist}, ${chosen.year}. Public domain, from Wikimedia Commons.` : row.desc}>
      <div className="st-modelsel">
        <select value={chosen ? art : "auto"} aria-label="Wallpaper artwork" onChange={e => ctx.setTweak("wallpaperArt", e.target.value)}>
          <option value="auto">A different one for each conversation</option>
          {W.LIST.map(a => <option key={a.id} value={a.id}>{a.title} — {a.artist}</option>)}
        </select>
      </div>
    </Row>
  );
}

Object.assign(window.OMP_SETTINGS_ROWS = window.OMP_SETTINGS_ROWS || {}, { wallpaperArt: _SRW_Art });
