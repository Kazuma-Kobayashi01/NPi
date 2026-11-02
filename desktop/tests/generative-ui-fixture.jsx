// Isolated consumer fixture: no bridge, credentials, sessions or model calls.
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
window.GUI_FIXTURE = { calls: [], renders: [], cancels: [], fail: false, resolve: null };
function GenerativeUiFixture() {
  const [disabled, setDisabled] = React.useState(false);
  const [version, setVersion] = React.useState(0);
  const [submitted, setSubmitted] = React.useState(null);
  const [count, setCount] = React.useState(0);
  const [terminal, setTerminal] = React.useState(null);
  const [continuation, setContinuation] = React.useState("admitted");
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
  return <div style={{ maxWidth: 760, margin: "32px auto", padding: 16 }}>
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
    <GenerativeUiCard key={version} msg={msg} onSubmit={onSubmit} onRendered={m => { window.GUI_FIXTURE.renders.push(m.ui_id); }} onCancel={async m => { window.GUI_FIXTURE.cancels.push(m.ui_id); setTerminal("cancelled"); return { ui_id: m.ui_id, status: "cancelled" }; }} disabled={disabled} />
    <GenerativeUiCard msg={{ kind: "ui", ui_id: "invalid-fixture", session_id: "isolated-fixture", schema: { version: 2, components: [] }, status: "open" }} onSubmit={onSubmit} />
    <p id="fixture-unaffected">The next message still renders after a malformed UI.</p>
  </div>;
}
ReactDOM.createRoot(document.getElementById("root")).render(<GenerativeUiFixture />);
