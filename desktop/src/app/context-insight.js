// What the current context is made of, beyond the rail's Context card, pure.
// Exposes `window.OMP_CONTEXT_INSIGHT`; an IIFE (AGENTS.md "IIFE rule").
// Reads the same model context the card does (get_messages without
// fullHistory, so only what follows the active compaction summary) and the
// card's estimator `window.OMP_USAGE.estimateTokens`, looked up late.
// Ideas after dsh-context (Apache-2.0, github.com/bowenliang123/dsh-context);
// no code is shared with it.
//
//  - analyze(contextMessages, messagesTokens): `{ parts, tools, files,
//    commands, trend, requests, cacheHit, turns, images }`. `parts` splits
//    the card's "Messages" category (`messagesTokens`): the text estimates
//    scaled together to that total. `trend` is one point per model request
//    (the last 60 of `requests`), from its usage; `images` counts image
//    blocks, which have no text estimate.
(function () {
  const PART_LABELS = {
    user: "Your messages",
    answers: "Answers",
    reasoning: "Reasoning",
    toolCalls: "Tool calls",
    toolResults: "Tool results",
    other: "Other",
    overhead: "Unattributed",
  };
  const READ_TOOLS = new Set(["read"]);
  const WRITE_TOOLS = new Set(["write", "edit", "ast_edit"]);
  const TREND_LIMIT = 60;
  const LIST_LIMIT = 12;

  const est = (text) => window.OMP_USAGE.estimateTokens(text);

  function textOf(content) {
    if (typeof content === "string") return content;
    let out = "";
    for (const block of content ?? []) if (block?.type === "text") out += `${block.text ?? ""}\n`;
    return out;
  }

  function imagesIn(content) {
    return Array.isArray(content) ? content.filter((b) => b?.type === "image").length : 0;
  }

  /** A read path without its `:<line range>` selector. */
  function cleanPath(p) {
    return String(p).replace(/:\d[\d,+\-]*$/, "");
  }

  // Words that only wrap the program a segment runs.
  const PREFIX_WORDS = new Set(["sudo", "env", "time", "nohup", "command", "exec", "builtin", "nice"]);
  const PROGRAM_NAME = /^[\w.+-]+$/;

  /**
   * Shell segments as word lists, split at `&&`, `||`, `;`, `|`, `&`, `(`, `)`
   * and newlines outside quotes. Quotes and backslash escapes stay inside a
   * word (`"a b"`, `NPi\ Desktop`); a heredoc's body is skipped.
   */
  function segmentsOf(command) {
    const text = String(command ?? "");
    const segments = [];
    let words = [], word = "", inWord = false, quote = "", heredoc = null;
    const endWord = () => { if (inWord) words.push(word); word = ""; inWord = false; };
    const endSegment = () => { endWord(); if (words.length) segments.push(words); words = []; };
    for (let i = 0; i < text.length; i++) {
      const ch = text[i];
      if (quote) {
        if (ch === quote) quote = "";
        else if (ch === "\\" && quote === '"' && i + 1 < text.length) word += text[++i];
        else word += ch;
        continue;
      }
      if (ch === "\n") {
        endSegment();
        if (heredoc) {
          // Skip the body up to the terminator line.
          const stop = text.indexOf(`\n${heredoc}\n`, i - 1);
          const end = stop === -1 ? (text.endsWith(`\n${heredoc}`) ? text.length : -1) : stop + heredoc.length + 1;
          i = end === -1 ? text.length : end;
          heredoc = null;
        }
        continue;
      }
      if (ch === "<" && text[i + 1] === "<" && text[i + 2] !== "<") {
        const m = /^<<-?\s*(['"]?)([\w.-]+)\1/.exec(text.slice(i));
        if (m) { heredoc = m[2]; i += m[0].length - 1; continue; }
      }
      if (ch === "'" || ch === '"') { quote = ch; inWord = true; continue; }
      if (ch === "\\" && i + 1 < text.length) { word += text[++i]; inWord = true; continue; }
      if (/\s/.test(ch)) { endWord(); continue; }
      // `&` in a redirection (`2>&1`, `&>`, `>&`) stays in the word.
      if (ch === "&" && (text[i - 1] === ">" || text[i + 1] === ">" || text[i - 1] === "<")) { word += ch; inWord = true; continue; }
      if (ch === ";" || ch === "|" || ch === "&" || ch === "(" || ch === ")") { endSegment(); continue; }
      word += ch;
      inWord = true;
    }
    endSegment();
    return segments;
  }

  /** Program names a shell command runs, one per segment; `export`/assignment-only segments run none. */
  function programsOf(command) {
    const out = [];
    for (const words of segmentsOf(command)) {
      // Skip assignments, wrapper words and the wrappers' own options (`sudo -E`).
      let word = null, wrapped = false;
      for (const w of words) {
        if (/^[A-Za-z_][A-Za-z0-9_]*=/.test(w)) continue;
        if (PREFIX_WORDS.has(w)) { wrapped = true; continue; }
        if (wrapped && w.startsWith("-")) continue;
        word = w;
        break;
      }
      if (!word || word === "export" || word.startsWith("-")) continue;
      const name = word.split("/").pop();
      if (PROGRAM_NAME.test(name)) out.push(name);
    }
    return out;
  }

  function bump(map, key, init) {
    let row = map.get(key);
    if (!row) map.set(key, (row = init()));
    return row;
  }

  function analyze(contextMessages, messagesTokens) {
    const raw = { user: 0, answers: 0, reasoning: 0, toolCalls: 0, toolResults: 0, other: 0 };
    const tools = new Map();
    const files = new Map();
    const commands = new Map();
    const trend = [];
    let turns = 0, prompt = 0, cached = 0, images = 0;
    const countCommand = (command) => {
      for (const program of programsOf(command)) bump(commands, program, () => ({ program, count: 0 })).count++;
    };

    for (const m of contextMessages ?? []) {
      if (!m || m.excludeFromContext || m.role === "compactionSummary") continue;
      if (m.role === "user") {
        turns++;
        images += imagesIn(m.content);
        raw.user += est(textOf(m.content));
      } else if (m.role === "fileMention") {
        for (const f of m.files ?? []) raw.user += est(f.content);
      } else if (m.role === "assistant") {
        for (const b of m.content ?? []) {
          if (b?.type === "text") raw.answers += est(b.text);
          else if (b?.type === "thinking") raw.reasoning += est(b.thinking);
          else if (b?.type === "toolCall") {
            const t = est(b.name) + est(JSON.stringify(b.arguments ?? {}));
            raw.toolCalls += t;
            const row = bump(tools, b.name, () => ({ name: b.name, calls: 0, errors: 0, tokens: 0 }));
            row.calls++;
            row.tokens += t;
            const args = b.arguments ?? {};
            if (typeof args.path === "string" && (READ_TOOLS.has(b.name) || WRITE_TOOLS.has(b.name))) {
              const path = READ_TOOLS.has(b.name) ? cleanPath(args.path) : args.path;
              const f = bump(files, path, () => ({ path, reads: 0, writes: 0 }));
              if (READ_TOOLS.has(b.name)) f.reads++;
              else f.writes++;
            }
            if (b.name === "bash" && typeof args.command === "string") countCommand(args.command);
          }
        }
        const u = m.usage;
        if (u && (u.input || u.cacheRead || u.cacheWrite || u.output)) {
          const context = (u.input ?? 0) + (u.cacheRead ?? 0) + (u.cacheWrite ?? 0);
          prompt += context;
          cached += u.cacheRead ?? 0;
          trend.push({ ts: m.timestamp ?? null, turn: turns, context, cacheRead: u.cacheRead ?? 0, output: u.output ?? 0, model: m.model ?? "" });
        }
      } else if (m.role === "toolResult") {
        const t = est(textOf(m.content));
        images += imagesIn(m.content);
        raw.toolResults += t;
        const row = bump(tools, m.toolName, () => ({ name: m.toolName, calls: 0, errors: 0, tokens: 0 }));
        row.tokens += t;
        if (m.isError) row.errors++;
      } else if (m.role === "bashExecution") {
        raw.toolResults += est(m.command) + est(m.output);
        countCommand(m.command);
      } else if (m.role === "pythonExecution") {
        raw.toolResults += est(m.code) + est(m.output);
      } else {
        raw.other += est(textOf(m.content)) + est(m.summary);
      }
    }

    // Text estimates (about 4 characters a token) run low on code and JSON
    // and high on prose: scale them together to the reported Messages total.
    // Only with no text to scale does the whole share stay unattributed.
    const budget = Math.max(0, messagesTokens ?? 0);
    const estimated = Object.values(raw).reduce((n, v) => n + v, 0);
    const parts = [];
    if (estimated > 0) {
      let given = 0;
      for (const key of Object.keys(raw)) {
        const tokens = Math.floor((raw[key] * budget) / estimated);
        given += tokens;
        if (tokens > 0) parts.push({ key, label: PART_LABELS[key], tokens });
      }
      const largest = parts.reduce((a, b) => (b.tokens > (a?.tokens ?? -1) ? b : a), null);
      if (largest) largest.tokens += budget - given;
    } else if (budget > 0) {
      parts.push({ key: "overhead", label: PART_LABELS.overhead, tokens: budget });
    }

    const byCount = (a, b) => b - a;
    return {
      parts,
      tools: [...tools.values()].sort((a, b) => byCount(a.calls, b.calls) || byCount(a.tokens, b.tokens)),
      files: [...files.values()].sort((a, b) => byCount(a.reads + a.writes, b.reads + b.writes) || a.path.localeCompare(b.path)).slice(0, LIST_LIMIT),
      commands: [...commands.values()].sort((a, b) => byCount(a.count, b.count) || a.program.localeCompare(b.program)).slice(0, LIST_LIMIT),
      trend: trend.slice(-TREND_LIMIT),
      requests: trend.length,
      images,
      cacheHit: prompt > 0 ? cached / prompt : null,
      turns,
    };
  }

  window.OMP_CONTEXT_INSIGHT = { analyze, programsOf, segmentsOf, cleanPath };
})();
