// Isolated consumer fixture: no bridge, credentials, sessions or model calls.
const MarkdownMediaContext = window.MarkdownMediaContext;

const fixtureSchema = {
  version: 1, title: "Agent execution options", layout: "grid", components: [
    { type: "text", id: "intro", text: "Adjust locally. Submit once to send your choices. <script>window.UNSAFE=true</script>" },
    { type: "slider", id: "concurrency", label: "Concurrency", min: 1, max: 32, default: 4 },
    { type: "select", id: "thinking", label: "Thinking", options: ["off", "low", "medium", "high"].map(value => ({ label: value, value })) },
    { type: "input", id: "note", label: "Note", minLength: 3, required: true },
    { type: "radio", id: "mode", label: "Execution", options: [{ value: "normal", label: "Normal" }, { value: "careful", label: "Careful" }] },
    { type: "textarea", id: "description", label: "Description", rows: 3, maxLength: 500 },
    { type: "number", id: "temperature", label: "Temperature", min: 0, max: 2, step: 0.1, default: 0.7 },
    { type: "date", id: "deadline", label: "Deadline", min: "2026-01-01", max: "2026-12-31", default: "2026-11-02" },
    { type: "checkbox", id: "citations", label: "Include citations", default: true },
    { type: "switch", id: "concise", label: "Concise response", default: false, visibleWhen: { id: "citations", equals: true } },
    { type: "multiselect", id: "formats", label: "Formats", options: [{ value: "text", label: "Text" }, { value: "table", label: "Table" }, { value: "chart", label: "Chart" }], minItems: 1, maxItems: 2, default: ["text"] },
    { type: "button", id: "send", label: "Send choices" },
  ],
};

// Display card: real public Yellowstone references. The fixture resolver
// draws labelled previews locally, so the smoke test never needs the network.
const PRISMATIC = "https://d9-wret.s3.us-west-2.amazonaws.com/assets/palladium/production/s3fs-public/media/images/Midway.png";
const PRISMATIC_THUMB = "https://d9-wret.s3.us-west-2.amazonaws.com/assets/palladium/production/s3fs-public/styles/thumbnail/public/media/images/Midway.png?itok=MTMzyGco";
const YELLOWSTONE_MAP = "https://www.nps.gov/yell/planyourvisit/images/2023_YELL-GRTE-Tear-Off-Map_Web-Graphic_2.png?maxwidth=650&autorotate=false";

const mediaSchema = {
  version: 1, mode: "display", title: "Yellowstone field notes", layout: "grid", components: [
    { type: "text", id: "media-intro", text: "Two images share one delayed resolver. The spring asks for a preview first and upgrades to full bytes; the map uses its explicit preview reference; the missing card shows the retry path. Query strings stay out of debug timing." },
    {
      type: "image", id: "prismatic",
      src: PRISMATIC,
      thumbnail: PRISMATIC_THUMB,
      alt: "Grand Prismatic Spring in the Midway Geyser Basin",
      title: "Grand Prismatic Spring",
      sourceUrl: "https://www.usgs.gov/media/images/grand-prismatic-spring-midway-geyser-basin-yellowstone-national-park",
      attribution: "Robert Fournier / USGS public domain",
    },
    {
      type: "image", id: "park-map",
      src: YELLOWSTONE_MAP,
      alt: "Yellowstone National Park tear-off map preview",
      title: "Official park map",
      sourceUrl: "https://www.nps.gov/yell/planyourvisit/maps.htm",
      attribution: "National Park Service · public domain",
    },
    {
      type: "image", id: "missing",
      src: "https://example.invalid/missing-field-photo.jpg",
      alt: "Missing field photo",
      title: "Unavailable field photo",
    },
    {
      type: "image", id: "decode-fail",
      src: "https://decode.invalid/truncated.png",
      alt: "Truncated image with a valid PNG signature",
      title: "Undecodable image",
    },
  ],
};

window.GUI_FIXTURE = { calls: [], renders: [], cancels: [], fail: false, resolve: null };

// ── Controllable delayed image resolver ──────────────────────────────────
window.GUI_FIXTURE.image = {
  calls: [],
  cancels: [],
  pending: new Map(),
  delayMs: 900,
  failThumbnail: false,
  failFull: false,
};

function fixtureBase64(text) {
  const bytes = new TextEncoder().encode(text);
  let binary = "";
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary);
}

function fixtureSvg(source, priority) {
  const isMap = source.includes("nps.gov");
  const isPreview = priority !== "full" || source.includes("/styles/thumbnail/");
  const width = isPreview ? 320 : 960;
  const height = isPreview ? 180 : 540;
  const label = `${isMap ? "Park map" : "Prismatic Spring"} · ${isPreview ? "preview" : "full"}`;
  const color = isMap ? "#2f5d46" : "#b4522e";
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}">` +
    `<rect width="100%" height="100%" fill="${color}"/>` +
    `<circle cx="${width / 2}" cy="${height / 2}" r="${Math.round(height / 4)}" fill="rgba(255,255,255,0.16)"/>` +
    `<text x="50%" y="50%" text-anchor="middle" dominant-baseline="middle" font-family="Helvetica,Arial,sans-serif" font-size="${Math.round(width / 22)}" fill="#ffffff">${label}</text>` +
    `</svg>`;
  return fixtureBase64(svg);
}

function fixtureShouldFail(entry) {
  if (entry.source.includes("decode.invalid")) return false;
  if (entry.source.includes("example.invalid")) return true;
  if (entry.priority === "thumbnail") return window.GUI_FIXTURE.image.failThumbnail;
  return window.GUI_FIXTURE.image.failFull && !entry.source.includes("/styles/thumbnail/");
}

function fixtureSettle(entry) {
  const pending = window.GUI_FIXTURE.image.pending;
  if (!pending.has(entry.requestId)) return;
  pending.delete(entry.requestId);
  clearTimeout(entry.timer);
  window.GUI_FIXTURE.tick?.();
  if (entry.source.includes("decode.invalid")) {
    // Valid PNG magic signature, no IHDR/image data: the host resolves it,
    // but the webview decoder fails. This exercises the decode-error retry.
    entry.resolve({
      data: "iVBORw0KGgoAAAANSUhEUg==",
      mimeType: "image/png",
      requestId: entry.requestId,
      cacheHit: false,
      timing: { totalMs: window.GUI_FIXTURE.image.delayMs },
    });
    return;
  }
  if (fixtureShouldFail(entry)) {
    entry.reject(new Error(entry.source.includes("example.invalid")
      ? "Simulated unavailable image"
      : `Simulated ${entry.priority} failure`));
    return;
  }
  entry.resolve({
    data: fixtureSvg(entry.source, entry.priority),
    mimeType: "image/svg+xml",
    requestId: entry.requestId,
    cacheHit: false,
    timing: { totalMs: window.GUI_FIXTURE.image.delayMs },
  });
}

function fixtureRead(source, { requestId, priority }) {
  window.GUI_FIXTURE.image.calls.push({ requestId, source, priority });
  window.GUI_FIXTURE.tick?.();
  return new Promise((resolve, reject) => {
    const entry = { requestId, source, priority, resolve, reject, timer: null };
    entry.timer = setTimeout(() => fixtureSettle(entry), window.GUI_FIXTURE.image.delayMs);
    window.GUI_FIXTURE.image.pending.set(requestId, entry);
  });
}

function fixtureCancel(requestId) {
  const entry = window.GUI_FIXTURE.image.pending.get(requestId);
  if (!entry) return false;
  window.GUI_FIXTURE.image.pending.delete(requestId);
  clearTimeout(entry.timer);
  window.GUI_FIXTURE.image.cancels.push(requestId);
  window.GUI_FIXTURE.tick?.();
  entry.reject(new Error("image request cancelled"));
  return true;
}

function fixtureResolveNow() {
  for (const entry of [...window.GUI_FIXTURE.image.pending.values()]) fixtureSettle(entry);
}

function GenerativeUiFixture() {
  const [disabled, setDisabled] = React.useState(false);
  const [version, setVersion] = React.useState(0);
  const [submitted, setSubmitted] = React.useState(null);
  const [count, setCount] = React.useState(0);
  const [terminal, setTerminal] = React.useState(null);
  const [continuation, setContinuation] = React.useState("admitted");
  const [delayMs, setDelayMs] = React.useState(900);
  const [failThumbnail, setFailThumbnail] = React.useState(false);
  const [failFull, setFailFull] = React.useState(false);
  const [imageSession, setImageSession] = React.useState(0);
  const [, setMediaTick] = React.useState(0);

  React.useEffect(() => {
    window.GUI_FIXTURE.tick = () => setMediaTick(t => t + 1);
    return () => { delete window.GUI_FIXTURE.tick; };
  }, []);

  React.useEffect(() => {
    window.GUI_FIXTURE.image.delayMs = delayMs;
    window.GUI_FIXTURE.image.failThumbnail = failThumbnail;
    window.GUI_FIXTURE.image.failFull = failFull;
  }, [delayMs, failThumbnail, failFull]);

  const mediaStore = React.useMemo(() => window.OMP_CHAT_IMAGE.createStore({
    key: `fixture-media-${imageSession}`,
    read: fixtureRead,
    cancel: fixtureCancel,
  }), [imageSession]);
  React.useEffect(() => () => mediaStore.dispose(), [mediaStore]);

  const onSubmit = (msg, values) => {
    window.GUI_FIXTURE.calls.push({ ui_id: msg.ui_id, values });
    setCount(window.GUI_FIXTURE.calls.length);
    return new Promise((resolve, reject) => {
      window.GUI_FIXTURE.resolve = () => {
        if (window.GUI_FIXTURE.fail) reject(new Error("Fixture backend rejected submission"));
        else resolve({ ui_id: msg.ui_id, status: "submitted", continuation: "admitted" });
      };
    });
  };
  const msg = { kind: "ui", ui_id: `fixture-${version}`, session_id: "isolated-fixture", task_id: `tool-${version}`, message_id: `assistant-${version}`, schema: fixtureSchema, status: submitted ? "submitted" : terminal ?? "waiting_input", ...(submitted ? { values: submitted, continuation } : {}) };
  const restored = { concurrency: 12, thinking: "high", note: "Restored", mode: "careful", description: "Restored description", temperature: 1.2, deadline: "2026-11-15", citations: true, concise: false, formats: ["text", "table"] };
  const displayMsg = { kind: "ui", ui_id: "yellowstone-media", session_id: "isolated-fixture", schema: mediaSchema, status: "rendered", rendered: true };
  return <MarkdownMediaContext.Provider value={mediaStore}>
    <div style={{ maxWidth: 900, margin: "32px auto", padding: 16 }}>
      <h1>Generative UI consumer fixture</h1>
      <p>No model or backend requests. Calls: <output id="fixture-calls">{count}</output></p>
      <div>
        <button id="fixture-disable" onClick={() => setDisabled(v => !v)}>Toggle disabled</button>
        <button id="fixture-resolve" onClick={() => window.GUI_FIXTURE.resolve()}>Confirm pending</button>
        <button id="fixture-fail" onClick={() => { window.GUI_FIXTURE.fail = !window.GUI_FIXTURE.fail; }}>Toggle backend rejection</button>
        <button id="fixture-reset" onClick={() => { setSubmitted(null); setTerminal(null); setVersion(v => v + 1); }}>New form</button>
        <button id="fixture-history" onClick={() => { setSubmitted(restored); setContinuation("admitted"); }}>Restore submitted history</button>
        <button id="fixture-pending" onClick={() => { setSubmitted(restored); setContinuation("pending"); }}>Restore pending continuation</button>
        <button id="fixture-expired" onClick={() => { setSubmitted(null); setTerminal("expired"); }}>Expire form</button>
      </div>
      <div style={{ margin: "12px 0", padding: 8, border: "1px dashed #888" }}>
        <strong>Image resolver</strong>
        <label> delay <select id="fixture-image-delay" value={delayMs} onChange={e => setDelayMs(Number(e.target.value))}>
          <option value={0}>0 ms</option>
          <option value={900}>900 ms</option>
          <option value={2000}>2000 ms</option>
          <option value={5000}>5000 ms</option>
        </select></label>
        <label><input id="fixture-fail-thumbnail" type="checkbox" checked={failThumbnail} onChange={e => setFailThumbnail(e.target.checked)} /> fail preview</label>
        <label><input id="fixture-fail-full" type="checkbox" checked={failFull} onChange={e => setFailFull(e.target.checked)} /> fail full</label>
        <button id="fixture-resolve-images" onClick={fixtureResolveNow}>Resolve pending images</button>
        <button id="fixture-new-image-session" onClick={() => setImageSession(s => s + 1)}>New image session (cancels)</button>
        <output id="fixture-image-calls">{window.GUI_FIXTURE.image.calls.length}</output>
        <output id="fixture-image-cancels">{window.GUI_FIXTURE.image.cancels.length}</output>
      </div>
      <GenerativeUiCard key={version} msg={msg} onSubmit={onSubmit} onRendered={m => { window.GUI_FIXTURE.renders.push(m.ui_id); }} onCancel={async m => { window.GUI_FIXTURE.cancels.push(m.ui_id); setTerminal("cancelled"); return { ui_id: m.ui_id, status: "cancelled" }; }} disabled={disabled} />
      <GenerativeUiCard msg={displayMsg} />
      <GenerativeUiCard msg={{ kind: "ui", ui_id: "invalid-fixture", session_id: "isolated-fixture", schema: { version: 2, components: [] }, status: "open" }} onSubmit={onSubmit} />
      <p id="fixture-unaffected">The next message still renders after a malformed UI.</p>
    </div>
  </MarkdownMediaContext.Provider>;
}
ReactDOM.createRoot(document.getElementById("root")).render(<GenerativeUiFixture />);
