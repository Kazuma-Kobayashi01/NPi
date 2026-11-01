/* ui/chart-view.jsx — Declarative SVG chart components for assistant messages.
   Supports percentage-bar, donut, pie (real filled wedges), bar, and line charts.
   Strict schema validation (fail fast without silent coercions or truncation). */

(function () {
  const COLOR_PALETTE = [
    "var(--accent)",
    "var(--cyan)",
    "var(--magenta)",
    "var(--lime)",
    "var(--amber)",
    "var(--rose)",
    "var(--fg-3)"
  ];

  const ALLOWED_NAMED_COLORS = new Set([
    "accent",
    "cyan",
    "magenta",
    "lime",
    "amber",
    "rose",
    "line",
    "fg-3"
  ]);

  function resolveColor(c, idx) {
    if (typeof c === "string") {
      const s = c.trim();
      if (ALLOWED_NAMED_COLORS.has(s)) return `var(--${s})`;
    }
    return COLOR_PALETTE[idx % COLOR_PALETTE.length];
  }

  const numFormatter = new Intl.NumberFormat("en-US", {
    maximumSignificantDigits: 4,
    useGrouping: true
  });

  function formatValue(val) {
    return numFormatter.format(val);
  }

  function formatPercent(pct) {
    if (pct >= 10) return pct.toFixed(1) + "%";
    if (pct >= 1) return pct.toFixed(2) + "%";
    return pct.toPrecision(2) + "%";
  }

  // ── Percentage Bar Chart ───────────────────────────────────────────────────
  function PercentageBarChart({ data, unit }) {
    const total = data.reduce((sum, d) => sum + d.value, 0);

    return (
      <div className="chart-pbar-container">
        <div className="chart-pbar-track">
          {data.map((d, i) => {
            const pct = (d.value / total) * 100;
            if (pct <= 0) return null;
            return (
              <div
                key={i}
                className="chart-pbar-segment"
                style={{ width: `${pct}%`, background: resolveColor(d.color, i) }}
                title={`${d.label}: ${formatValue(d.value)}${unit ? " " + unit : ""} (${formatPercent(pct)})`}
              />
            );
          })}
        </div>
        <div className="chart-legend">
          {data.map((d, i) => {
            const pct = (d.value / total) * 100;
            return (
              <div key={i} className="chart-legend-item" title={d.label}>
                <span className="chart-dot" style={{ background: resolveColor(d.color, i) }} />
                <span className="chart-legend-label">{d.label}</span>
                <span className="chart-legend-val">{formatValue(d.value)}{unit ? " " + unit : ""}</span>
                <span className="chart-legend-pct">({formatPercent(pct)})</span>
              </div>
            );
          })}
        </div>
      </div>
    );
  }

  // ── Donut Chart (Hollow Ring) ──────────────────────────────────────────────
  function DonutChart({ data, unit, title }) {
    const total = data.reduce((sum, d) => sum + d.value, 0);
    const radius = 72;
    const strokeWidth = 26;
    const circ = 2 * Math.PI * radius;

    let accumulatedPct = 0;
    const segments = data.map((d, i) => {
      const pct = d.value / total;
      const strokeDasharray = `${pct * circ} ${circ}`;
      const strokeDashoffset = -(accumulatedPct * circ);
      accumulatedPct += pct;
      return {
        ...d,
        pct: pct * 100,
        color: resolveColor(d.color, i),
        strokeDasharray,
        strokeDashoffset
      };
    });

    return (
      <div className="chart-body-row">
        <div className="chart-svg-wrap">
          <svg viewBox="0 0 200 200" className="chart-svg" style={{ maxWidth: 200, maxHeight: 200 }}>
            <g transform="rotate(-90 100 100)">
              {segments.map((seg, i) => (
                <circle
                  key={i}
                  cx="100"
                  cy="100"
                  r={radius}
                  fill="none"
                  stroke={seg.color}
                  strokeWidth={strokeWidth}
                  strokeDasharray={seg.strokeDasharray}
                  strokeDashoffset={seg.strokeDashoffset}
                />
              ))}
            </g>
            <text x="100" y="96" textAnchor="middle" fill="var(--fg-3)" fontSize="11" fontWeight="500">
              TOTAL
            </text>
            <text x="100" y="115" textAnchor="middle" fill="var(--fg)" fontSize="15" fontWeight="700" fontFamily="var(--font-mono, monospace)">
              {formatValue(total)}
            </text>
            {unit && (
              <text x="100" y="130" textAnchor="middle" fill="var(--fg-3)" fontSize="10">
                {unit}
              </text>
            )}
          </svg>
        </div>
        <div className="chart-legend-side">
          {segments.map((d, i) => (
            <div key={i} className="chart-legend-item" title={d.label}>
              <span className="chart-dot" style={{ background: d.color }} />
              <span className="chart-legend-label">{d.label}</span>
              <span className="chart-legend-val">{formatValue(d.value)}{unit ? " " + unit : ""}</span>
              <span className="chart-legend-pct">({formatPercent(d.pct)})</span>
            </div>
          ))}
        </div>
      </div>
    );
  }

  // ── Pie Chart (Real Filled Slices / Wedges) ────────────────────────────────
  function PieChart({ data, unit }) {
    const total = data.reduce((sum, d) => sum + d.value, 0);
    const cx = 100;
    const cy = 100;
    const r = 82;

    let accumulatedAngle = -Math.PI / 2; // start at 12 o'clock
    const slices = data.map((d, i) => {
      const pct = d.value / total;
      const angle = pct * 2 * Math.PI;
      const startAngle = accumulatedAngle;
      const endAngle = accumulatedAngle + angle;
      accumulatedAngle += angle;

      const x1 = cx + r * Math.cos(startAngle);
      const y1 = cy + r * Math.sin(startAngle);
      const x2 = cx + r * Math.cos(endAngle);
      const y2 = cy + r * Math.sin(endAngle);
      const largeArc = pct > 0.5 ? 1 : 0;
      const isFull = pct >= 0.9999;
      const pathData = isFull
        ? null
        : `M ${cx} ${cy} L ${x1.toFixed(2)} ${y1.toFixed(2)} A ${r} ${r} 0 ${largeArc} 1 ${x2.toFixed(2)} ${y2.toFixed(2)} Z`;

      return {
        ...d,
        pct: pct * 100,
        color: resolveColor(d.color, i),
        isFull,
        pathData
      };
    });

    return (
      <div className="chart-body-row">
        <div className="chart-svg-wrap">
          <svg viewBox="0 0 200 200" className="chart-svg" style={{ maxWidth: 200, maxHeight: 200 }}>
            {slices.map((sl, i) =>
              sl.isFull ? (
                <circle key={i} cx={cx} cy={cy} r={r} fill={sl.color} />
              ) : (
                <path
                  key={i}
                  d={sl.pathData}
                  fill={sl.color}
                  stroke="var(--bg-elevated)"
                  strokeWidth="1.5"
                />
              )
            )}
          </svg>
        </div>
        <div className="chart-legend-side">
          {slices.map((d, i) => (
            <div key={i} className="chart-legend-item" title={d.label}>
              <span className="chart-dot" style={{ background: d.color }} />
              <span className="chart-legend-label">{d.label}</span>
              <span className="chart-legend-val">{formatValue(d.value)}{unit ? " " + unit : ""}</span>
              <span className="chart-legend-pct">({formatPercent(d.pct)})</span>
            </div>
          ))}
        </div>
      </div>
    );
  }

  // ── Bar Chart ──────────────────────────────────────────────────────────────
  function BarChart({ data, unit }) {
    const values = data.map((d) => d.value);
    const minVal = Math.min(0, ...values);
    const maxVal = Math.max(0, ...values);
    const range = maxVal - minVal === 0 ? 1 : maxVal - minVal;

    const w = 400;
    const h = 200;
    const padL = 45;
    const padR = 20;
    const padT = 20;
    const padB = 30;
    const plotW = w - padL - padR;
    const plotH = h - padT - padB;

    const zeroY = padT + ((maxVal - 0) / range) * plotH;
    const slotW = plotW / (data.length || 1);
    const barW = Math.max(8, Math.min(36, slotW * 0.65));

    return (
      <div className="chart-svg-wrap">
        <svg viewBox={`0 0 ${w} ${h}`} className="chart-svg">
          {/* Baseline */}
          <line x1={padL} y1={zeroY} x2={w - padR} y2={zeroY} stroke="var(--line)" strokeWidth="1" />

          {/* Bars */}
          {data.map((d, i) => {
            const val = d.value;
            // Height is strictly 0 when val is 0 — no fake positive bar
            const barH = val === 0 ? 0 : (Math.abs(val) / range) * plotH;
            const x = padL + i * slotW + (slotW - barW) / 2;
            const y = val >= 0 ? zeroY - barH : zeroY;
            const color = resolveColor(d.color, i);

            return (
              <g key={i}>
                {barH > 0 && (
                  <rect
                    x={x}
                    y={y}
                    width={barW}
                    height={barH}
                    rx="3"
                    ry="3"
                    fill={color}
                    title={`${d.label}: ${formatValue(val)}${unit ? " " + unit : ""}`}
                  />
                )}
                <text
                  x={x + barW / 2}
                  y={val >= 0 ? y - 5 : y + barH + 12}
                  textAnchor="middle"
                  fill="var(--fg)"
                  fontSize="10"
                  fontFamily="var(--font-mono, monospace)"
                >
                  {formatValue(val)}
                </text>
                <text
                  x={x + barW / 2}
                  y={h - 10}
                  textAnchor="middle"
                  fill="var(--fg-3)"
                  fontSize="10"
                >
                  {d.label.length > 8 ? d.label.slice(0, 7) + "…" : d.label}
                </text>
              </g>
            );
          })}
        </svg>
      </div>
    );
  }

  // ── Line Chart ─────────────────────────────────────────────────────────────
  function LineChart({ data, unit }) {
    const values = data.map((d) => d.value);
    const minVal = Math.min(...values);
    const maxVal = Math.max(...values);
    const range = maxVal - minVal === 0 ? 1 : maxVal - minVal;

    const w = 400;
    const h = 200;
    const padL = 45;
    const padR = 20;
    const padT = 25;
    const padB = 30;
    const plotW = w - padL - padR;
    const plotH = h - padT - padB;

    const points = data.map((d, i) => {
      const x = padL + (i / Math.max(1, data.length - 1)) * plotW;
      const y = padT + plotH - ((d.value - minVal) / range) * plotH;
      return { x, y, ...d };
    });

    const polyPoints = points.map((p) => `${p.x.toFixed(1)},${p.y.toFixed(1)}`).join(" ");

    return (
      <div className="chart-svg-wrap">
        <svg viewBox={`0 0 ${w} ${h}`} className="chart-svg">
          {/* Grid lines */}
          {[0, 0.5, 1].map((pct, idx) => {
            const gy = padT + plotH * (1 - pct);
            const val = minVal + pct * range;
            return (
              <g key={idx}>
                <line x1={padL} y1={gy} x2={w - padR} y2={gy} stroke="var(--line)" strokeWidth="1" strokeDasharray="3 3" opacity="0.6" />
                <text x={padL - 6} y={gy + 3} textAnchor="end" fill="var(--fg-3)" fontSize="9" fontFamily="var(--font-mono, monospace)">
                  {formatValue(val)}
                </text>
              </g>
            );
          })}

          {/* Polyline */}
          <polyline
            points={polyPoints}
            fill="none"
            stroke="var(--accent)"
            strokeWidth="2.5"
            strokeLinecap="round"
            strokeLinejoin="round"
          />

          {/* Data Points */}
          {points.map((p, i) => (
            <g key={i}>
              <circle
                cx={p.x}
                cy={p.y}
                r="3.5"
                fill="var(--bg-elevated)"
                stroke={resolveColor(p.color, 0)}
                strokeWidth="2"
              />
              <text x={p.x} y={h - 10} textAnchor="middle" fill="var(--fg-3)" fontSize="10">
                {p.label.length > 8 ? p.label.slice(0, 7) + "…" : p.label}
              </text>
            </g>
          ))}
        </svg>
      </div>
    );
  }

  // ── Main ChartView Component ───────────────────────────────────────────────
  const KNOWN_TYPES = new Set(["percentage-bar", "donut", "pie", "bar", "line"]);

  function ChartView({ specText, isClosed }) {
    // React Hook Order: All hooks MUST be declared unconditionally at top of component
    const [copied, setCopied] = React.useState(false);

    const parsed = React.useMemo(() => {
      // If fence is open, mark pending without attempting JSON parse
      if (isClosed === false) {
        return { pending: true, error: null };
      }

      const text = (specText ?? "").trim();
      if (!text) {
        return { error: new Error("Chart spec cannot be empty") };
      }

      let obj;
      try {
        obj = JSON.parse(text);
      } catch (err) {
        return { error: new Error(`Invalid chart JSON: ${err.message}`) };
      }

      if (!obj || typeof obj !== "object" || Array.isArray(obj)) {
        return { error: new Error("Chart spec must be a JSON object") };
      }

      if (typeof obj.type !== "string" || !KNOWN_TYPES.has(obj.type.toLowerCase())) {
        return {
          error: new Error(
            `Unknown chart type "${obj.type}". Expected one of: percentage-bar, donut, pie, bar, line`
          )
        };
      }
      const type = obj.type.toLowerCase();

      if (!Array.isArray(obj.data) || obj.data.length === 0) {
        return { error: new Error("Chart 'data' property must be a non-empty array") };
      }

      if (obj.data.length > 50) {
        return { error: new Error(`Chart data exceeds maximum limit of 50 items (got ${obj.data.length})`) };
      }

      const validatedData = [];
      for (let i = 0; i < obj.data.length; i++) {
        const item = obj.data[i];
        if (!item || typeof item !== "object" || Array.isArray(item)) {
          return { error: new Error(`Chart data[${i}] must be an object`) };
        }
        if (typeof item.label !== "string" || item.label.trim().length === 0) {
          return { error: new Error(`Chart data[${i}] must have a non-empty string 'label'`) };
        }
        if (typeof item.value !== "number" || !Number.isFinite(item.value)) {
          return { error: new Error(`Chart data[${i}] ('${item.label}') must have a finite number 'value'`) };
        }

        // Percentage, donut, and pie charts require non-negative values
        if (["percentage-bar", "donut", "pie"].includes(type) && item.value < 0) {
          return {
            error: new Error(
              `Chart type "${type}" requires non-negative values, got ${item.value} for '${item.label}'`
            )
          };
        }

        validatedData.push({
          label: item.label.trim(),
          value: item.value,
          color: typeof item.color === "string" ? item.color.trim() : undefined
        });
      }

      if (["percentage-bar", "donut", "pie"].includes(type)) {
        const total = validatedData.reduce((sum, d) => sum + d.value, 0);
        if (total <= 0) {
          return { error: new Error(`Total value for chart type "${type}" must be greater than zero`) };
        }
      }

      return {
        type,
        title: typeof obj.title === "string" && obj.title.trim() ? obj.title.trim() : null,
        unit: typeof obj.unit === "string" && obj.unit.trim() ? obj.unit.trim() : null,
        data: validatedData,
        error: null
      };
    }, [specText, isClosed]);

    const handleCopy = React.useCallback(() => {
      if (!specText) return;
      navigator.clipboard?.writeText(specText).then(() => {
        setCopied(true);
        setTimeout(() => setCopied(false), 1600);
      });
    }, [specText]);

    // Renders after all hooks have executed
    if (parsed.pending) {
      return (
        <div className="chart-pending">
          <span className="pulse-dot" /> Streaming chart data…
        </div>
      );
    }

    if (parsed.error) {
      return (
        <div className="chart-error">
          <div className="chart-error-head">
            <span className="chip danger">Chart Error</span>
            <span className="chart-error-msg">{parsed.error.message}</span>
          </div>
          <pre className="code-block copy-host">
            <code className="hljs">{specText}</code>
          </pre>
        </div>
      );
    }

    return (
      <div className="chart-card">
        <div className="chart-header">
          <div className="chart-title">
            {parsed.title && <span>{parsed.title}</span>}
            {parsed.unit && <span className="chart-unit">({parsed.unit})</span>}
          </div>
          <div className="chart-actions">
            <button
              className="btn ghost is-floating"
              type="button"
              onClick={handleCopy}
              title="Copy chart JSON"
              style={{ height: 20, fontSize: 11, padding: "0 6px" }}
            >
              {copied ? "copied!" : "copy JSON"}
            </button>
          </div>
        </div>

        {parsed.type === "percentage-bar" && (
          <PercentageBarChart data={parsed.data} unit={parsed.unit} />
        )}
        {parsed.type === "donut" && (
          <DonutChart data={parsed.data} unit={parsed.unit} title={parsed.title} />
        )}
        {parsed.type === "pie" && (
          <PieChart data={parsed.data} unit={parsed.unit} />
        )}
        {parsed.type === "bar" && (
          <BarChart data={parsed.data} unit={parsed.unit} />
        )}
        {parsed.type === "line" && (
          <LineChart data={parsed.data} unit={parsed.unit} />
        )}
      </div>
    );
  }

  window.ChartView = ChartView;
})();
