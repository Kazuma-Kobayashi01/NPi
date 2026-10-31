/* design/settings/rows-chat.jsx — Settings → Chat's custom rows (window.OMP_SETTINGS_ROWS):
   `chatInstructions` and `chatMemory`, the two texts the Chat prompt carries
   (chat_space.rs). Both are saved together with the button; they are read from
   Rust, not kept in the tweaks. */

function _SRC_useChatPrefs(ctx) {
  const [saved, setSaved] = React.useState(null);   // { instructions, memory } as on disk
  const [draft, setDraft] = React.useState(null);
  const [state, setState] = React.useState("");     // "" | "saving" | "saved" | an error
  const load = React.useCallback(() => {
    Promise.resolve(ctx.bridge?.chatPrefs?.()).then(p => { if (p) { setSaved(p); setDraft(p); } });
  }, [ctx.bridge]);
  React.useEffect(load, [load]);
  const save = async () => {
    setState("saving");
    const r = await ctx.bridge?.setChatPrefs?.(draft.instructions, draft.memory);
    if (r?.ok) { setSaved(draft); setState("saved"); } else setState(String(r?.error ?? "could not save"));
  };
  return { saved, draft, setDraft, save, state, dirty: !!draft && !!saved && (draft.instructions !== saved.instructions || draft.memory !== saved.memory) };
}

function _SRC_Text({ row, ctx, query, field, placeholder }) {
  const { SettingsRow: Row, SettingsButton: Button } = window;
  const p = _SRC_useChatPrefs(ctx);
  return (
    <Row stack label={row.label} desc={row.desc} query={query}>
      <div className="st-chattext">
        <textarea className="st-textarea selectable" rows={field === "memory" ? 6 : 5} value={p.draft?.[field] ?? ""}
          placeholder={placeholder} disabled={!p.draft} aria-label={row.label}
          onChange={e => p.setDraft(d => ({ ...d, [field]: e.target.value }))} />
        <div className="st-chattext-foot">
          <Button accent onClick={p.save} disabled={!p.dirty || p.state === "saving"}>Save</Button>
          <span className="st-fav-empty">{p.state === "saved" && !p.dirty ? "Saved" : p.state && p.state !== "saving" && p.state !== "saved" ? p.state : ""}</span>
        </div>
      </div>
    </Row>
  );
}

const _SRC_Instructions = (props) => <_SRC_Text {...props} field="instructions" placeholder="For example: answer in Chinese, warm and concise, no bullet lists." />;
const _SRC_Memory = (props) => <_SRC_Text {...props} field="memory" placeholder="One fact per line, for example: I live in Shanghai." />;

Object.assign(window.OMP_SETTINGS_ROWS = window.OMP_SETTINGS_ROWS || {}, { chatInstructions: _SRC_Instructions, chatMemory: _SRC_Memory });
