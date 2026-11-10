// app/office-meter.js — the Office window's fuel-pump meter.
//
// Pure: the numbers the meter shows between two readings (it rolls toward
// the latest reading instead of jumping), the unit price, the JPY amount and
// how each figure is written.
(function () {
  /** How long the meter takes to reach a new reading, ms. Readings arrive about every two seconds while members work. */
  const ROLL_MS = 1800;

  /** A meter at rest on `reading` ({ tokens, cost }). */
  function rest(reading) {
    const r = { tokens: reading?.tokens ?? 0, cost: reading?.cost ?? 0 };
    return { from: r, to: r, at: 0 };
  }

  /** `meter` heading for `reading` from whatever it shows at `now`. A lower reading (a new day, the other range) is shown at once. */
  function aim(meter, reading, now) {
    const to = { tokens: reading?.tokens ?? 0, cost: reading?.cost ?? 0 };
    if (to.tokens === meter.to.tokens && to.cost === meter.to.cost) return meter;
    if (to.tokens < meter.to.tokens || to.cost < meter.to.cost) return { from: to, to, at: now };
    return { from: shown(meter, now), to, at: now };
  }

  /** What the meter shows at `now`: eased from `from` to `to` over ROLL_MS. */
  function shown(meter, now) {
    const t = Math.min(1, Math.max(0, (now - meter.at) / ROLL_MS));
    const k = 1 - (1 - t) * (1 - t);
    return {
      tokens: meter.from.tokens + (meter.to.tokens - meter.from.tokens) * k,
      cost: meter.from.cost + (meter.to.cost - meter.from.cost) * k,
    };
  }

  /** Whether the meter is still moving at `now`. */
  function rolling(meter, now) {
    return (meter.from.tokens !== meter.to.tokens || meter.from.cost !== meter.to.cost) && now - meter.at < ROLL_MS;
  }

  /** USD per million tokens; null when nothing was used. */
  function unitPrice(reading) {
    return reading && reading.tokens > 0 ? (reading.cost / reading.tokens) * 1e6 : null;
  }

  /** `usd` in yen at `rate` (JPY per USD); null without a rate. */
  function toJpy(usd, rate) {
    return typeof rate === "number" && rate > 0 ? usd * rate : null;
  }

  const group = n => String(n).replace(/\B(?=(\d{3})+(?!\d))/g, ",");

  /** "1,832,406" */
  function fmtTokens(n) {
    return group(Math.max(0, Math.floor(n ?? 0)));
  }

  /** Short token count for a card: "412k", "1.8M", "950". */
  function fmtTokensShort(n) {
    const v = Math.max(0, n ?? 0);
    if (v >= 1e6) return `${(v / 1e6).toFixed(v >= 1e7 ? 0 : 1)}M`;
    if (v >= 1e3) return `${(v / 1e3).toFixed(v >= 1e5 ? 0 : 1)}k`;
    return String(Math.floor(v));
  }

  /** "$12.47"; four decimals under a cent so a small run is not "$0.00". */
  function fmtUsd(usd) {
    const v = Math.max(0, usd ?? 0);
    if (v > 0 && v < 0.01) return `$${v.toFixed(4)}`;
    const [whole, cents] = v.toFixed(2).split(".");
    return `$${group(whole)}.${cents}`;
  }

  /** "¥1,873"; one decimal under a yen; "¥—" without a rate. */
  function fmtJpy(yen) {
    if (yen == null) return "¥—";
    const v = Math.max(0, yen);
    if (v > 0 && v < 1) return `¥${v.toFixed(1)}`;
    return `¥${group(Math.round(v))}`;
  }

  /** Milliseconds from `now` (a Date) to the next local midnight: when "today" starts over. */
  function msToMidnight(now) {
    const next = new Date(now.getFullYear(), now.getMonth(), now.getDate() + 1);
    return next.getTime() - now.getTime();
  }

  window.OMP_OFFICE_METER = { ROLL_MS, rest, aim, shown, rolling, unitPrice, toJpy, fmtTokens, fmtTokensShort, fmtUsd, fmtJpy, msToMidnight };
})();
