// Bounded, data-only Generative UI consumer. Kernel validation is authoritative.
(function () {
  const blocked = new Set(["__proto__", "constructor", "prototype"]);
  /** Mirrors the kernel: model image references stay compact and public. */
  const MAX_IMAGE_REF = 2048;

  function localHostname(hostname) {
    const host = String(hostname).toLowerCase().replace(/\.$/, "");
    if (host === "localhost" || host.endsWith(".localhost") || host.endsWith(".local")) return true;
    const address = host.startsWith("[") ? host.slice(1, -1) : host;
    if (address.startsWith("::ffff:")) return true;
    if (address === "::1" || address === "::") return true;
    if (/^f[cd][0-9a-f]{2}:/.test(address) || /^fe[89ab][0-9a-f]:/.test(address)) return true;
    const v4 = /^(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})$/.exec(address);
    if (!v4) return false;
    const [a, b] = [Number(v4[1]), Number(v4[2])];
    return a === 0 || a === 127 || a === 10 || (a === 169 && b === 254) ||
      (a === 172 && b >= 16 && b <= 31) || (a === 192 && (b === 168 || b === 0)) ||
      (a === 100 && b >= 64 && b <= 127) || (a === 198 && (b === 18 || b === 19)) || a >= 224;
  }

  const keys = {
    // Display components
    text: ["type", "id", "text", "label", "visibleWhen"],
    card: ["type", "id", "title", "subtitle", "content", "tag", "visibleWhen"],
    stat: ["type", "id", "label", "value", "diff", "trend", "unit", "visibleWhen"],
    badge: ["type", "id", "text", "variant", "visibleWhen"],
    alert: ["type", "id", "message", "title", "level", "visibleWhen"],
    progress: ["type", "id", "label", "value", "max", "unit", "visibleWhen"],
    table: ["type", "id", "title", "columns", "rows", "visibleWhen"],
    "key-value": ["type", "id", "title", "items", "visibleWhen"],
    collapsible: ["type", "id", "title", "content", "defaultExpanded", "visibleWhen"],
    tabs: ["type", "id", "items", "defaultTab", "visibleWhen"],
    timeline: ["type", "id", "title", "items", "visibleWhen"],
    "bar-chart": ["type", "id", "title", "unit", "data", "visibleWhen"],
    "line-chart": ["type", "id", "title", "unit", "data", "visibleWhen"],
    "pie-chart": ["type", "id", "title", "unit", "data", "visibleWhen"],
    divider: ["type", "id", "label", "visibleWhen"],
    image: ["type", "id", "src", "thumbnail", "alt", "title", "sourceUrl", "attribution", "description", "visibleWhen"],
    // Interactive components
    button: ["type", "id", "label"],
    input: ["type", "id", "label", "default", "required", "minLength", "maxLength", "placeholder", "visibleWhen"],
    textarea: ["type", "id", "label", "default", "required", "minLength", "maxLength", "rows", "placeholder", "visibleWhen"],
    date: ["type", "id", "label", "default", "min", "max", "required", "visibleWhen"],
    "date-picker": ["type", "id", "label", "default", "min", "max", "required", "visibleWhen"],
    "time-picker": ["type", "id", "label", "default", "required", "visibleWhen"],
    number: ["type", "id", "label", "min", "max", "step", "default", "unit", "visibleWhen"],
    "number-input": ["type", "id", "label", "min", "max", "step", "default", "unit", "visibleWhen"],
    slider: ["type", "id", "label", "min", "max", "step", "default", "unit", "visibleWhen"],
    checkbox: ["type", "id", "label", "default", "visibleWhen"],
    switch: ["type", "id", "label", "default", "visibleWhen"],
    multiselect: ["type", "id", "label", "options", "default", "minItems", "maxItems", "visibleWhen"],
    "checkbox-group": ["type", "id", "label", "options", "default", "minItems", "maxItems", "visibleWhen"],
    select: ["type", "id", "label", "options", "default", "required", "visibleWhen"],
    radio: ["type", "id", "label", "options", "default", "required", "visibleWhen"],
    "segmented-control": ["type", "id", "label", "options", "default", "required", "visibleWhen"],
    "searchable-select": ["type", "id", "label", "options", "default", "required", "visibleWhen"],
    "option-cards": ["type", "id", "label", "cards", "default", "required", "visibleWhen"],
  };

  const interactiveTypes = new Set([
    "button", "input", "textarea", "date", "date-picker", "time-picker",
    "number", "number-input", "slider", "checkbox", "switch",
    "multiselect", "checkbox-group", "select", "radio",
    "segmented-control", "searchable-select", "option-cards",
  ]);

  function isInteractive(c) {
    return interactiveTypes.has(c.type);
  }

  function isoDate(value, name) {
    const s = string(value, 10, name);
    const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(s);
    if (!m) throw new Error(`Invalid ${name}`);
    const y = Number(m[1]), mo = Number(m[2]), d = Number(m[3]);
    const dt = new Date(Date.UTC(y, mo - 1, d));
    if (dt.getUTCFullYear() !== y || dt.getUTCMonth() !== mo - 1 || dt.getUTCDate() !== d) throw new Error(`Invalid ${name}`);
    return s;
  }

  function timeString(value, name) {
    const s = string(value, 8, name);
    const m = /^([01]\d|2[0-3]):([0-5]\d)(?::([0-5]\d))?$/.exec(s);
    if (!m) throw new Error(`Invalid ${name} (HH:MM or HH:MM:SS)`);
    return s;
  }

  function parseVisibleWhen(value) {
    if (value === undefined) return undefined;
    object(value, ["id", "equals"]);
    if (typeof value.id !== "string" || !/^[A-Za-z0-9_-]{1,64}$/.test(value.id) || blocked.has(value.id)) throw new Error("Invalid visibleWhen.id");
    const eq = value.equals;
    if (typeof eq === "string") return { id: value.id, equals: string(eq, 128, "visibleWhen.equals") };
    if (typeof eq === "number") return { id: value.id, equals: number(eq) };
    if (typeof eq === "boolean") return { id: value.id, equals: eq };
    throw new Error("Invalid visibleWhen.equals");
  }

  function isComponentVisible(c, values) {
    if (c.type === "button" || !c.visibleWhen) return true;
    const target = values?.[c.visibleWhen.id];
    return Array.isArray(target) ? target.includes(c.visibleWhen.equals) : target === c.visibleWhen.equals;
  }

  function object(value, allowed) {
    if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error("Expected an object");
    for (const key of Object.keys(value)) if (!allowed.includes(key)) throw new Error(`Unknown UI field: ${key}`);
  }

  function string(value, max, name) {
    if (typeof value !== "string" || value.length > max) throw new Error(`Invalid ${name}`);
    return value;
  }

  function number(value) {
    if (!Number.isFinite(value) || Math.abs(value) > 1e6) throw new Error("Invalid UI number");
    return value;
  }

  function onStep(value, c) {
    const n = (value - c.min) / c.step;
    return Number.isFinite(n) && Math.abs(n - Math.round(n)) <= 1e-8;
  }

  function normalizeSchema(schema) {
    object(schema, ["version", "mode", "title", "layout", "components"]);
    if (schema.version !== 1) throw new Error("Unsupported UI version");
    const layout = schema.layout === undefined ? "stack" : schema.layout;
    if (layout !== "stack" && layout !== "row" && layout !== "grid" && layout !== "column") throw new Error("Unsupported UI layout");
    if (!Array.isArray(schema.components) || schema.components.length < 1 || schema.components.length > 24) throw new Error("UI requires 1–24 components");
    let total = 0, buttons = 0;
    const ids = new Set();
    const text = (s, max, name) => { const out = string(s, max, name); total += out.length; return out; };
    const title = schema.title === undefined ? undefined : text(schema.title, 160, "title");

    const components = schema.components.map(raw => {
      if (!raw || !Object.hasOwn(keys, raw.type)) throw new Error("Unsupported UI component");
      object(raw, keys[raw.type]);
      const c = { ...raw };
      if (typeof c.id !== "string" || !/^[A-Za-z0-9_-]{1,64}$/.test(c.id) || blocked.has(c.id) || ids.has(c.id)) throw new Error("Invalid or duplicate UI id");
      ids.add(c.id);
      if (c.type !== "button" && c.visibleWhen !== undefined) c.visibleWhen = parseVisibleWhen(c.visibleWhen);

      // Display components
      if (c.type === "text") {
        if (c.label !== undefined) c.label = text(c.label, 120, "label");
        c.text = text(c.text, 4096, "text");
      } else if (c.type === "card") {
        if (c.title !== undefined) c.title = text(c.title, 160, "card title");
        if (c.subtitle !== undefined) c.subtitle = text(c.subtitle, 200, "card subtitle");
        if (c.content !== undefined) c.content = text(c.content, 4096, "card content");
        if (c.tag !== undefined) c.tag = text(c.tag, 60, "card tag");
      } else if (c.type === "stat") {
        c.label = text(c.label, 120, "stat label");
        c.value = text(c.value, 60, "stat value");
        if (c.diff !== undefined) c.diff = text(c.diff, 60, "stat diff");
        if (c.trend !== undefined && c.trend !== "up" && c.trend !== "down" && c.trend !== "neutral") throw new Error("Invalid stat trend");
        if (c.unit !== undefined) c.unit = text(c.unit, 30, "stat unit");
      } else if (c.type === "badge") {
        c.text = text(c.text, 60, "badge text");
        if (c.variant !== undefined && !["default", "success", "warning", "danger", "info"].includes(c.variant)) throw new Error("Invalid badge variant");
      } else if (c.type === "alert") {
        c.message = text(c.message, 2048, "alert message");
        if (c.title !== undefined) c.title = text(c.title, 160, "alert title");
        if (c.level !== undefined && !["info", "success", "warning", "danger"].includes(c.level)) throw new Error("Invalid alert level");
      } else if (c.type === "progress") {
        if (c.label !== undefined) c.label = text(c.label, 120, "progress label");
        c.max = c.max === undefined ? 100 : number(c.max);
        c.value = number(c.value);
        if (c.max <= 0 || c.value < 0 || c.value > c.max) throw new Error("Invalid progress bounds");
        if (c.unit !== undefined) c.unit = text(c.unit, 30, "progress unit");
      } else if (c.type === "table") {
        if (c.title !== undefined) c.title = text(c.title, 160, "table title");
        if (!Array.isArray(c.columns) || c.columns.length < 1 || c.columns.length > 12) throw new Error("Invalid table columns");
        if (!Array.isArray(c.rows) || c.rows.length > 50) throw new Error("Invalid table rows");
        c.columns = c.columns.map(col => {
          object(col, ["key", "label", "align"]);
          if (col.align !== undefined && !["left", "center", "right"].includes(col.align)) throw new Error("Invalid column align");
          return { key: text(col.key, 64, "col key"), label: text(col.label, 120, "col label"), align: col.align };
        });
        c.rows = c.rows.map(r => {
          object(r, c.columns.map(col => col.key));
          const rowOut = {};
          for (const col of c.columns) {
            const v = r[col.key];
            if (typeof v === "number") rowOut[col.key] = number(v);
            else if (typeof v === "string") rowOut[col.key] = text(v, 256, "cell text");
            else if (v === undefined || v === null) rowOut[col.key] = "";
            else throw new Error("Table cell must be string or number");
          }
          return rowOut;
        });
      } else if (c.type === "key-value") {
        if (c.title !== undefined) c.title = text(c.title, 160, "key-value title");
        if (!Array.isArray(c.items) || c.items.length < 1 || c.items.length > 30) throw new Error("Invalid key-value items");
        c.items = c.items.map(item => {
          object(item, ["key", "label", "value"]);
          return { key: text(item.key, 64, "item key"), label: text(item.label, 120, "item label"), value: text(item.value, 1024, "item value") };
        });
      } else if (c.type === "collapsible") {
        c.title = text(c.title, 160, "collapsible title");
        c.content = text(c.content, 4096, "collapsible content");
        if (c.defaultExpanded !== undefined && typeof c.defaultExpanded !== "boolean") throw new Error("Invalid defaultExpanded");
      } else if (c.type === "tabs") {
        if (!Array.isArray(c.items) || c.items.length < 1 || c.items.length > 8) throw new Error("Invalid tabs items");
        const tabIds = new Set();
        c.items = c.items.map(tab => {
          object(tab, ["id", "label", "content"]);
          const tid = text(tab.id, 64, "tab id");
          if (tabIds.has(tid)) throw new Error("Duplicate tab id");
          tabIds.add(tid);
          return { id: tid, label: text(tab.label, 80, "tab label"), content: text(tab.content, 4096, "tab content") };
        });
        c.defaultTab = c.defaultTab === undefined ? c.items[0].id : text(c.defaultTab, 64, "defaultTab");
        if (!tabIds.has(c.defaultTab)) throw new Error("Invalid defaultTab");
      } else if (c.type === "timeline") {
        if (c.title !== undefined) c.title = text(c.title, 160, "timeline title");
        if (!Array.isArray(c.items) || c.items.length < 1 || c.items.length > 20) throw new Error("Invalid timeline items");
        c.items = c.items.map(item => {
          object(item, ["time", "title", "description", "status"]);
          if (item.status !== undefined && !["pending", "current", "done", "failed"].includes(item.status)) throw new Error("Invalid timeline status");
          return {
            time: text(item.time, 60, "timeline time"),
            title: text(item.title, 120, "timeline title"),
            description: item.description === undefined ? undefined : text(item.description, 1024, "timeline description"),
            status: item.status,
          };
        });
      } else if (["bar-chart", "line-chart", "pie-chart"].includes(c.type)) {
        if (c.title !== undefined) c.title = text(c.title, 160, "chart title");
        if (c.unit !== undefined) c.unit = text(c.unit, 30, "chart unit");
        if (!Array.isArray(c.data) || c.data.length < 1 || c.data.length > 50) throw new Error("Invalid chart data");
        c.data = c.data.map(pt => {
          object(pt, ["label", "value", "color"]);
          const val = number(pt.value);
          if (c.type === "pie-chart" && val < 0) throw new Error("Pie chart values must be non-negative");
          return { label: text(pt.label, 80, "chart label"), value: val, color: pt.color === undefined ? undefined : text(pt.color, 40, "chart color") };
        });
      } else if (c.type === "divider") {
        if (c.label !== undefined) c.label = text(c.label, 120, "divider label");
      } else if (c.type === "image") {
        const imageRef = (value, name, normalize) => {
          const out = normalize(string(value, MAX_IMAGE_REF, name), name);
          total += out.length;
          return out;
        };
        const normalizeHttps = (s, name) => {
          if (/\s/.test(s)) throw new Error(`Invalid ${name} (whitespace is not allowed)`);
          let url;
          try {
            url = new URL(s);
          } catch {
            throw new Error(`Invalid ${name} (absolute https URL required)`);
          }
          if (url.protocol !== "https:") throw new Error(`${name} must use https`);
          if (url.username !== "" || url.password !== "") throw new Error(`${name} must not embed credentials`);
          if (localHostname(url.hostname)) throw new Error(`${name} must not target a local host`);
          // Keep the canonical spelling the host's loader recognises, and
          // enforce the bound after percent-encoding can expand it.
          const canonical = url.toString();
          if (canonical.length > MAX_IMAGE_REF) throw new Error(`Invalid ${name} (canonical URL exceeds ${MAX_IMAGE_REF} characters)`);
          return canonical;
        };
        const normalizeProjectPath = (s, name) => {
          if (s.startsWith("/") || s.startsWith("\\") || s.startsWith("~")) {
            throw new Error(`${name} must be project-relative`);
          }
          const segments = s.split(/[\\/]/);
          if (segments.some(segment => segment === "" || segment === "." || segment === "..")) {
            throw new Error(`${name} must be a normalized project-relative path`);
          }
          return segments.join("/");
        };
        c.src = imageRef(c.src, "image.src", (s, name) =>
          /^[A-Za-z][A-Za-z0-9+.-]*:/.test(s) ? normalizeHttps(s, name) : normalizeProjectPath(s, name));
        if (c.thumbnail !== undefined) c.thumbnail = imageRef(c.thumbnail, "image.thumbnail", normalizeHttps);
        if (c.alt !== undefined) c.alt = text(c.alt, 300, "image.alt");
        if (c.title !== undefined) c.title = text(c.title, 160, "image.title");
        if (c.sourceUrl !== undefined) c.sourceUrl = imageRef(c.sourceUrl, "image.sourceUrl", normalizeHttps);
        if (c.attribution !== undefined) c.attribution = text(c.attribution, 200, "image.attribution");
        if (c.description !== undefined) c.description = text(c.description, 1200, "image.description");
      } else if (c.type === "button") {
        buttons++;
        c.label = text(c.label, 120, "button label");
      } else {
        // Interactive inputs
        c.label = text(c.label, 120, "label");
        if (c.required !== undefined && typeof c.required !== "boolean") throw new Error("Invalid required flag");
        if (["input", "textarea", "date", "date-picker", "time-picker", "select", "radio", "segmented-control", "searchable-select", "option-cards"].includes(c.type)) {
          c.required ??= false;
        }
        if (c.type === "checkbox" || c.type === "switch") {
          if (c.default === undefined) c.default = false;
          if (typeof c.default !== "boolean") throw new Error("Invalid UI boolean");
        }
        if (c.type === "date" || c.type === "date-picker") {
          if (c.min !== undefined) c.min = isoDate(c.min, "date min");
          if (c.max !== undefined) c.max = isoDate(c.max, "date max");
          if (c.min !== undefined && c.max !== undefined && c.min > c.max) throw new Error("Invalid date range");
          if (c.default === undefined || c.default === "") c.default = "";
          else {
            c.default = isoDate(c.default, "date default");
            if ((c.min !== undefined && c.default < c.min) || (c.max !== undefined && c.default > c.max)) throw new Error("Invalid date default");
          }
        }
        if (c.type === "time-picker") {
          if (c.default === undefined || c.default === "") c.default = "";
          else c.default = timeString(c.default, "time default");
        }
        if (c.type === "input" || c.type === "textarea") {
          if (c.minLength === undefined) c.minLength = 0;
          if (c.type === "textarea") {
            if (c.rows === undefined) c.rows = 4;
            if (!Number.isInteger(c.rows) || c.rows < 1 || c.rows > 20) throw new Error("Invalid textarea rows");
          }
          if (c.maxLength === undefined) c.maxLength = 1024;
          if (c.default === undefined) c.default = "";
          if (!Number.isInteger(c.maxLength) || c.maxLength < 0 || c.maxLength > 4096 || (c.minLength !== undefined && (!Number.isInteger(c.minLength) || c.minLength < 0 || c.minLength > c.maxLength))) throw new Error("Invalid input length");
          text(c.default, c.maxLength, "input default");
          if (c.minLength !== undefined && c.default !== "" && c.default.length < c.minLength) throw new Error("Invalid input default");
          if (c.placeholder !== undefined) c.placeholder = text(c.placeholder, 120, "placeholder");
        }
        if (c.type === "slider" || c.type === "number" || c.type === "number-input") {
          if (c.step === undefined) c.step = 1;
          [c.min, c.max, c.step, c.default].forEach(number);
          if (c.min >= c.max || c.step <= 0 || !Number.isFinite((c.max - c.min) / c.step) || c.default < c.min || c.default > c.max || !onStep(c.default, c)) throw new Error("Invalid numeric range or default");
          if (c.unit !== undefined) c.unit = text(c.unit, 30, "unit");
        }
        if (c.type === "select" || c.type === "radio" || c.type === "segmented-control" || c.type === "searchable-select" || c.type === "multiselect" || c.type === "checkbox-group") {
          if (!Array.isArray(c.options) || c.options.length < 1 || c.options.length > 32) throw new Error("Invalid UI options");
          const seen = new Set();
          c.options = c.options.map(o => {
            object(o, ["label", "value", "description"]);
            const label = text(o.label, 120, "option label"), value = text(o.value, 128, "option value");
            if (!value || seen.has(value)) throw new Error("Empty or duplicate option value");
            seen.add(value);
            return { label, value, ...(o.description ? { description: text(o.description, 200, "description") } : {}) };
          });
          if (c.type === "multiselect" || c.type === "checkbox-group") {
            if (c.minItems === undefined) c.minItems = 0;
            if (c.maxItems === undefined) c.maxItems = c.options.length;
            if (!Number.isInteger(c.minItems) || !Number.isInteger(c.maxItems) || c.minItems < 0 || c.maxItems > c.options.length || c.minItems > c.maxItems) throw new Error("Invalid multiselect limits");
            if (c.default === undefined) c.default = [];
            if (!validSelections(c.default, c, 0)) throw new Error("Invalid multiselect default");
            c.default = [...c.default];
          } else {
            if (c.default === undefined) c.default = c.options[0].value;
            text(c.default, 128, "option default");
            if (!seen.has(c.default) && !(c.default === "" && c.required === false)) throw new Error("Invalid option default");
          }
        }
        if (c.type === "option-cards") {
          if (!Array.isArray(c.cards) || c.cards.length < 1 || c.cards.length > 12) throw new Error("Invalid option cards");
          const cardIds = new Set();
          c.cards = c.cards.map(card => {
            object(card, ["id", "title", "description", "tag", "recommended"]);
            const cid = text(card.id, 64, "card id");
            if (cardIds.has(cid)) throw new Error("Duplicate option card id");
            cardIds.add(cid);
            return {
              id: cid,
              title: text(card.title, 120, "card title"),
              ...(card.description ? { description: text(card.description, 512, "card description") } : {}),
              ...(card.tag ? { tag: text(card.tag, 60, "card tag") } : {}),
              ...(card.recommended !== undefined ? { recommended: typeof card.recommended === "boolean" ? card.recommended : false } : {}),
            };
          });
          if (c.default === undefined) c.default = c.cards[0].id;
          text(c.default, 64, "option-cards default");
          if (!cardIds.has(c.default) && !(c.default === "" && c.required === false)) throw new Error("Invalid option-cards default");
        }
      }
      return c;
    });

    let mode;
    const hasInteractive = components.some(isInteractive);
    if (schema.mode !== undefined) {
      if (schema.mode !== "display" && schema.mode !== "interactive") throw new Error("Invalid UI mode");
      mode = schema.mode;
    } else {
      mode = buttons > 0 || hasInteractive ? "interactive" : "display";
    }

    if (mode === "display") {
      if (buttons > 0) throw new Error("Display UI must not contain submit buttons");
      const interactiveControls = components.filter(c => isInteractive(c) && c.type !== "button");
      if (interactiveControls.length > 0) throw new Error("Display UI must not contain interactive inputs");
    } else {
      if (buttons !== 1) throw new Error("Interactive UI requires exactly one submit button");
    }

    const controlIds = new Set(components.filter(c => isInteractive(c) && c.type !== "button").map(c => c.id));
    for (const c of components) {
      if (c.visibleWhen && (c.visibleWhen.id === c.id || !controlIds.has(c.visibleWhen.id))) throw new Error("Invalid visibleWhen target");
    }
    if (total > 32768) throw new Error("UI text exceeds limit");
    return { version: 1, mode, ...(title === undefined ? {} : { title }), layout, components };
  }

  function validSelections(value, c, min = c.minItems) {
    return Array.isArray(value) && value.length >= min && value.length <= c.maxItems && new Set(value).size === value.length && value.every(v => typeof v === "string" && c.options.some(o => o.value === v));
  }

  function cloneValues(values) {
    return Object.fromEntries(Object.entries(values).map(([id, value]) => [id, Array.isArray(value) ? [...value] : value]));
  }

  function initialValues(schema) {
    return cloneValues(Object.fromEntries(schema.components.filter(c => isInteractive(c) && c.type !== "button").map(c => [c.id, c.default])));
  }

  function validateValues(schema, values) {
    const errors = {};
    const controls = schema.components.filter(c => isInteractive(c) && c.type !== "button");
    object(values, controls.map(c => c.id));
    for (const c of controls) {
      if (!isComponentVisible(c, values)) continue;
      const v = values[c.id];
      if (c.type === "slider" || c.type === "number" || c.type === "number-input") {
        if (!Number.isFinite(v) || Math.abs(v) > 1e6 || v < c.min || v > c.max || !onStep(v, c)) errors[c.id] = "Choose a value within the range and step.";
      } else if (c.type === "checkbox" || c.type === "switch") {
        if (typeof v !== "boolean") errors[c.id] = "Choose on or off.";
      } else if (c.type === "multiselect" || c.type === "checkbox-group") {
        if (!validSelections(v, c)) errors[c.id] = `Choose ${c.minItems}–${c.maxItems} distinct options.`;
      } else if (c.type === "date" || c.type === "date-picker") {
        if (typeof v !== "string" || (c.required && v === "")) errors[c.id] = "Choose a valid date.";
        else if (v !== "") {
          try {
            const d = isoDate(v, c.id);
            if ((c.min !== undefined && d < c.min) || (c.max !== undefined && d > c.max)) errors[c.id] = "Choose a date within the allowed range.";
          } catch { errors[c.id] = "Choose a valid YYYY-MM-DD date."; }
        }
      } else if (c.type === "time-picker") {
        if (typeof v !== "string" || (c.required && v === "")) errors[c.id] = "Choose a valid time.";
        else if (v !== "") {
          try { timeString(v, c.id); } catch { errors[c.id] = "Choose a valid HH:MM time."; }
        }
      } else if (typeof v !== "string") errors[c.id] = "Enter a value.";
      else if (c.type === "input" || c.type === "textarea") {
        if (c.required && v.length === 0) errors[c.id] = "This field is required.";
        else if ((v !== "" && v.length < (c.minLength ?? 0)) || v.length > c.maxLength) errors[c.id] = `Use ${c.minLength ?? 0}–${c.maxLength} characters.`;
      } else if (c.type === "option-cards") {
        if (!c.cards.some(card => card.id === v) && !(v === "" && c.required === false)) errors[c.id] = "Choose one of the cards.";
      } else if (["select", "radio", "segmented-control", "searchable-select"].includes(c.type)) {
        if (!c.options.some(o => o.value === v) && !(v === "" && c.required === false)) errors[c.id] = "Choose one of the available options.";
      }
    }
    return errors;
  }

  const isTerminal = status => ["submitted", "cancelled", "expired", "rendered"].includes(status);

  function messageToUi(message) {
    if (message?.role !== "custom" || !["ui.present", "ui.submit", "ui.state"].includes(message.customType)) return null;
    const d = message.details;
    if (!d || typeof d.ui_id !== "string" || !d.ui_id || typeof d.session_id !== "string" || !d.session_id) {
      return { kind: "ui", ui_id: `malformed:${message.customType}:${Number.isFinite(message.timestamp) ? message.timestamp : "unknown"}`, session_id: "", status: "expired", error: "Invalid UI message identity" };
    }
    const identity = { ui_id: d.ui_id, session_id: d.session_id, task_id: d.task_id, message_id: d.message_id };
    if (message.customType === "ui.submit") return { kind: "ui.submit", ...identity, status: "submitted", rendered: d.rendered === true, values: d.values, continuation: d.continuation };
    if (message.customType === "ui.state") {
      if (d.status !== "waiting_input" && !isTerminal(d.status)) return { kind: "ui", ...identity, ui_id: `malformed:ui.state:${d.ui_id}`, status: "expired", error: "Invalid UI lifecycle status" };
      return { kind: "ui.state", ...identity, status: d.status, rendered: d.rendered === true, reason: d.reason, continuation: d.continuation };
    }
    let schema, error;
    try { schema = normalizeSchema(d.schema); } catch (e) { error = e.message; }
    const status = schema?.mode === "display"
      ? "rendered"
      : (isTerminal(d.status) ? d.status : (d.task_id && d.message_id ? "waiting_input" : "expired"));
    return {
      kind: "ui", ...identity, schema, status,
      rendered: schema?.mode === "display" || d.rendered === true,
      projected_expired: status === "expired" && !isTerminal(d.status),
      continuation: d.continuation,
      ...(error ? { error } : {}),
    };
  }

  function upsertMessage(messages, message) {
    const ui = ["ui", "ui.submit", "ui.state"].includes(message?.kind) ? message : messageToUi(message);
    if (!ui) return messages;
    const index = messages.findIndex(m => m.kind === "ui" && m.ui_id === ui.ui_id);
    if (index === -1) return ui.kind === "ui" ? [...messages, ui] : messages;
    const prior = messages[index];
    if (prior.session_id !== ui.session_id || (prior.task_id && ui.task_id && prior.task_id !== ui.task_id) || (prior.message_id && ui.message_id && prior.message_id !== ui.message_id)) {
      return upsertMessage(messages, { kind: "ui", ui_id: `malformed:${ui.kind}:${ui.ui_id}:${ui.session_id}`, session_id: "", status: "expired", error: "UI lifecycle session mismatch" });
    }
    if ((prior.status === "expired" && !prior.projected_expired) || prior.status === "cancelled") return messages;
    if (ui.kind === "ui.submit") {
      try {
        if (!prior.schema || Object.keys(validateValues(prior.schema, ui.values)).length) throw new Error("Invalid UI submission values");
      } catch (error) {
        return upsertMessage(messages, { kind: "ui", ui_id: `malformed:ui.submit:${ui.ui_id}:values`, session_id: "", status: "expired", error: error.message });
      }
    }
    const incomingStatus = ui.kind === "ui.submit" ? "submitted" : ui.status;
    const endsPending = prior.status === "submitted" && prior.continuation === "pending" && !ui.projected_expired && (incomingStatus === "expired" || incomingStatus === "cancelled");
    const status = endsPending ? incomingStatus : prior.status === "submitted" ? "submitted" : prior.projected_expired && incomingStatus !== "submitted" ? prior.status : incomingStatus;
    const next = messages.slice();
    next[index] = {
      ...prior,
      ...(ui.kind === "ui" ? ui : {}),
      kind: "ui",
      ...(prior._id === undefined ? {} : { _id: prior._id }),
      task_id: prior.task_id ?? ui.task_id,
      message_id: prior.message_id ?? ui.message_id,
      status,
      projected_expired: status === "expired" && (incomingStatus === "expired" ? ui.projected_expired === true : prior.projected_expired === true),
      rendered: prior.rendered || ui.rendered,
      reason: ui.reason ?? prior.reason,
      continuation: prior.continuation === "admitted" ? "admitted" : ui.continuation ?? prior.continuation,
      ...(prior.values !== undefined ? { values: prior.values } : ui.values !== undefined ? { values: cloneValues(ui.values) } : {}),
    };
    return next;
  }

  window.OMP_GENERATIVE_UI = Object.freeze({
    normalizeSchema,
    initialValues,
    cloneValues,
    validateValues,
    isComponentVisible,
    isInteractive,
    isTerminal,
    messageToUi,
    upsertMessage,
  });
})();
