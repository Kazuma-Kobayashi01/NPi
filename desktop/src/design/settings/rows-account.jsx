/* design/settings/rows-account.jsx — Settings → Account's custom row
   `account` (window.OMP_SETTINGS_ROWS): photo, name, what to call you, work,
   interests and a free note. The photo and full name stay in the tweaks (the
   sidebar shows them); the rest is the Chat assistant's profile, which Rust
   keeps (chat_space.rs) and adds to every chat's prompt. */

const _SRA_WORK = ["", "Engineering", "Design", "Product", "Research", "Writing", "Education", "Business", "Student", "Other"];

/** A picked image as a 128px square JPEG data URL (cropped to the middle). */
function _SRA_square(file) {
  return new Promise((resolve, reject) => {
    const url = URL.createObjectURL(file);
    const img = new Image();
    img.onload = () => {
      const side = Math.min(img.width, img.height);
      const c = document.createElement("canvas");
      c.width = c.height = 128;
      c.getContext("2d").drawImage(img, (img.width - side) / 2, (img.height - side) / 2, side, side, 0, 0, 128, 128);
      URL.revokeObjectURL(url);
      resolve(c.toDataURL("image/jpeg", 0.88));
    };
    img.onerror = () => { URL.revokeObjectURL(url); reject(new Error("not an image")); };
    img.src = url;
  });
}

function _SRA_Avatar({ src, name, size = 56 }) {
  const initial = String(name || "?").trim().charAt(0).toUpperCase() || "?";
  return src
    ? <img className="acct-avatar" src={src} alt="" width={size} height={size} />
    : <span className="acct-avatar acct-initial" style={{ width: size, height: size, fontSize: size * 0.42 }}>{initial}</span>;
}

function _SRA_Account({ row, ctx, query }) {
  const { SettingsRow: Row, SettingsButton: Button } = window;
  const t = ctx.t;
  const [saved, setSaved] = React.useState(null);
  const [draft, setDraft] = React.useState(null);
  const [state, setState] = React.useState("");
  const fileRef = React.useRef(null);
  const syncTimer = React.useRef(0);
  React.useEffect(() => {
    Promise.resolve(ctx.bridge?.chatProfile?.()).then(p => {
      const v = p ?? { call: "", work: "", interests: "", about: "" };
      setSaved(v); setDraft(v);
      // A name saved before the field lived in the tweaks comes along.
      if (v.call && !t.accountCall) ctx.setTweak("accountCall", v.call);
    });
    return () => clearTimeout(syncTimer.current);
  }, [ctx.bridge]);
  // The name to call you is a tweak (the greeting reads it at once); the assistant's copy follows a moment later.
  const setCall = (value) => {
    ctx.setTweak("accountCall", value);
    clearTimeout(syncTimer.current);
    syncTimer.current = setTimeout(() => {
      const base = saved ?? { call: "", work: "", interests: "", about: "" };
      ctx.bridge?.setChatProfile?.({ ...base, ...(draft ?? {}), call: value.trim() });
    }, 500);
  };
  // `call` lives apart from the Save button below.
  const dirty = !!draft && !!saved && ["work", "interests", "about"].some(k => draft[k] !== saved[k]);
  const set = (k) => (e) => setDraft(d => ({ ...d, [k]: e.target.value }));
  const save = async () => {
    setState("saving");
    const full = { ...draft, call: String(t.accountCall ?? "").trim() };
    const r = await ctx.bridge?.setChatProfile?.(full);
    if (r?.ok) { setSaved(full); setDraft(full); setState("saved"); }
    else setState(String(r?.error ?? "could not save"));
  };
  const pick = async (e) => {
    const f = e.target.files?.[0]; e.target.value = "";
    if (!f) return;
    if (f.size > 10 * 1024 * 1024) { setState("The photo is over 10 MB."); return; }
    try { ctx.setTweak("accountAvatar", await _SRA_square(f)); setState(""); } catch { setState("That file is not an image."); }
  };
  return (
    <>
      <Row label="Profile photo" desc="PNG, JPEG or WebP, up to 10 MB." query={query}>
        <div className="acct-photo">
          <_SRA_Avatar src={t.accountAvatar} name={t.accountCall || t.accountName} />
          <input ref={fileRef} type="file" accept="image/png,image/jpeg,image/webp" style={{ display: "none" }} onChange={pick} />
          <Button onClick={() => fileRef.current?.click()}>Change</Button>
          {t.accountAvatar && <Button onClick={() => ctx.setTweak("accountAvatar", "")}>Remove</Button>}
        </div>
      </Row>
      <Row label="Full name" query={query}>
        <input className="st-input" value={t.accountName ?? ""} aria-label="Full name" onChange={e => ctx.setTweak("accountName", e.target.value)} />
      </Row>
      <Row label="What should the assistant call you?" query={query}>
        <input className="st-input" value={t.accountCall ?? ""} aria-label="Name to call you" placeholder="Used in greetings and chats" onChange={e => setCall(e.target.value)} />
      </Row>
      <Row label="What best describes your work?" query={query}>
        <div className="st-modelsel">
          <select value={draft?.work ?? ""} disabled={!draft} aria-label="Work" onChange={set("work")}>
            {_SRA_WORK.map(w => <option key={w} value={w}>{w || "Not set"}</option>)}
            {draft?.work && !_SRA_WORK.includes(draft.work) && <option value={draft.work}>{draft.work}</option>}
          </select>
        </div>
      </Row>
      <Row label="Interests and hobbies" query={query}>
        <input className="st-input" value={draft?.interests ?? ""} disabled={!draft} aria-label="Interests" placeholder="Tea, film photography, hiking" onChange={set("interests")} />
      </Row>
      <Row stack label="Anything else about you" desc="The assistant keeps this in mind in every chat in Chat mode. It is stored on this computer only." query={query}>
        <div className="st-chattext">
          <textarea className="st-textarea selectable" rows={4} value={draft?.about ?? ""} disabled={!draft} aria-label="About you"
            placeholder="Where you live, how you like to be answered, what you are working toward." onChange={set("about")} />
          <div className="st-chattext-foot">
            <Button accent onClick={save} disabled={!dirty || state === "saving"}>Save</Button>
            <span className="st-fav-empty">{state === "saved" && !dirty ? "Saved" : state && state !== "saving" && state !== "saved" ? state : ""}</span>
          </div>
        </div>
      </Row>
    </>
  );
}

Object.assign(window, { AccountAvatar: _SRA_Avatar });
Object.assign(window.OMP_SETTINGS_ROWS = window.OMP_SETTINGS_ROWS || {}, { account: _SRA_Account });
