// Wires marked 18 (token-object renderer API) for everything the app renders
// through `dangerouslySetInnerHTML` (ui/markdown.jsx, ui/plan-annotations.jsx):
// highlight.js for code blocks, and the two gaps that would otherwise let
// agent output run script in the app's own privileged origin, which exposes
// `window.__TAURI__`:
//
// - Raw HTML. marked passes it through verbatim, and `<img onerror>`,
//   `<svg onload>`, `<details ontoggle>` or a `srcdoc` iframe run their
//   handlers the moment the markup is inserted. It is rendered as text
//   instead — every tag, block or inline, with no allowlist: a hand-written
//   sanitizer is exactly the thing that gets bypassed.
// - `javascript:` (or `data:`/`vbscript:`) link hrefs. They never fire the
//   webview's navigation-policy hook (`on_navigation`,
//   src-tauri/src/navigation_guard.rs): WebKit executes them inline while
//   resolving the click, before a navigation is even attempted (chromium and
//   WebView2 do the same). marked's own `cleanUrl` only `encodeURI`s the
//   href; it never checks the scheme.
//
// A file, not an inline <script>, so the release CSP can drop
// 'unsafe-inline' from script-src (scripts/build-frontend.mjs fails the build
// on any inline script). Loaded right after marked.min.js/highlight.min.js;
// both are parser-blocking, so they are already defined here.
(function () {
  // Fence `text` as a code block that its own content cannot close: a fixed
  // ``` fence lets an embedded ``` line end the block early, and everything
  // after it would be parsed as markdown. A backtick run longer than any
  // run inside is always safe. For callers that build markdown source
  // (message-queue.js's refusal notes); defined before the guard below so it
  // needs neither library.
  function fenceCode(text, lang = '') {
    const longest = (text.match(/`+/g) ?? []).reduce((n, r) => Math.max(n, r.length), 0);
    const fence = '`'.repeat(Math.max(3, longest + 1));
    return fence + lang + '\n' + text + '\n' + fence;
  }
  // A file path `new URL` reads as a scheme: a Windows drive path (`C:/x.rs`,
  // a slash after the letter) or a bare file with a line (`main.rs:12`,
  // `main.rs:12-20`, `main.rs:12:5`). Neither shape can name a script
  // scheme. The `link` renderer below keeps such hrefs as anchors for the
  // file preview, and app/file-preview.js's `linkTarget` parses them;
  // defined before the guard below so that file needs neither library.
  const DRIVE_PATH = /^[a-z]:[/\\]/i;
  const BARE_FILE_LINE = /^\w[\w.-]*\.[a-z0-9]+:\d+(?:[-:]\d+)?$/i;
  function isSchemeLikeFilePath(href) {
    const s = String(href ?? '');
    return DRIVE_PATH.test(s) || BARE_FILE_LINE.test(s);
  }
  // LaTeX delimiter grammar (ported from omp/packages/utils/src/math-delimiters.ts)
  const MATH_BLOCK_DOLLAR = / {0,3}\$\$[ \t]*\r?\n([\s\S]+?)\r?\n {0,3}\$\$[ \t]*(?:\r?\n|$)/y;
  const MATH_BLOCK_BRACKET = / {0,3}\\\[[ \t]*\r?\n([\s\S]+?)\r?\n {0,3}\\\][ \t]*(?:\r?\n|$)/y;

  function mathOpenerAt(source, at) {
    const first = source.charCodeAt(at);
    if (first === 0x24 /* $ */) return source.charCodeAt(at + 1) === 0x24 ? "$$" : "$";
    if (first !== 0x5c /* \ */) return undefined;
    const second = source.charCodeAt(at + 1);
    if (second === 0x28 /* ( */) return "\\(";
    if (second === 0x5b /* [ */) return "\\[";
    return undefined;
  }

  function mathStartIndex(source, from = 0) {
    for (let at = from; at < source.length; at++) {
      if (mathOpenerAt(source, at) !== undefined) return at;
    }
    return undefined;
  }

  function escapedAt(source, index, from = 0) {
    let backslashes = 0;
    for (let at = index - 1; at >= from && source.charCodeAt(at) === 0x5c; at--) backslashes++;
    return backslashes % 2 === 1;
  }

  function dollarOpens(source, open) {
    const after = source[open + 1];
    return !(after === undefined || after === " " || after === "\t" || after === "\n" || after === "$");
  }

  function scanDollar(source, open) {
    if (!dollarOpens(source, open)) return { close: -1, stop: open + 1 };
    for (let at = open + 1; at < source.length; at++) {
      const char = source[at];
      if (char === "\\") { at++; continue; }
      if (char === "\n") return { close: -1, stop: at };
      if (char !== "$") continue;
      const before = source[at - 1];
      if (before === " " || before === "\t") return { close: -1, stop: at };
      const next = source[at + 1];
      if (next !== undefined && next >= "0" && next <= "9") continue;
      return { close: source.slice(open + 1, at).trim().length > 0 ? at : -1, stop: at };
    }
    return { close: -1, stop: source.length };
  }

  function closerIndex(source, opener, from) {
    const closer = opener === "\\(" ? "\\)" : opener === "\\[" ? "\\]" : opener;
    for (let at = source.indexOf(closer, from); at !== -1; at = source.indexOf(closer, at + 1)) {
      if (!escapedAt(source, at, from)) return at;
    }
    return -1;
  }

  function spanOf(source, opener, bodyStart, closeAt) {
    if (closeAt === -1) return undefined;
    const body = source.slice(bodyStart, closeAt);
    if (opener === "$$" && body.trim() === "") return undefined;
    return { opener, display: opener === "$$" || opener === "\\[", end: closeAt + opener.length, body };
  }

  function mathSpanAt(source, at, from = 0) {
    const opener = mathOpenerAt(source, at);
    if (opener === undefined || escapedAt(source, at, from)) return undefined;
    const bodyStart = at + opener.length;
    return spanOf(
      source,
      opener,
      bodyStart,
      opener === "$" ? scanDollar(source, at).close : closerIndex(source, opener, bodyStart)
    );
  }

  function mathBlockAt(source) {
    MATH_BLOCK_DOLLAR.lastIndex = 0;
    MATH_BLOCK_BRACKET.lastIndex = 0;
    const m = MATH_BLOCK_DOLLAR.exec(source) || MATH_BLOCK_BRACKET.exec(source);
    return (m && m[1].trim() !== "") ? { raw: m[0], body: m[1] } : undefined;
  }

  class MathSpans {
    #source;
    #dollarOpen = -1;
    #dollarStop = -1;
    #dollarClose = -1;
    #dollarSolid = -1;
    #searchedFrom = { $$: Infinity, "\\(": Infinity, "\\[": Infinity };
    #closer = { $$: -1, "\\(": -1, "\\[": -1 };

    constructor(source) {
      this.#source = source;
    }

    spanAt(at, end = this.#source.length) {
      const source = this.#source;
      const opener = mathOpenerAt(source, at);
      if (opener === undefined) return undefined;
      const bodyStart = at + opener.length;
      const close = opener === "$" ? this.#dollarCloser(at) : this.#closerFrom(opener, bodyStart);
      if (close === -1 || close + opener.length > end) return undefined;
      return spanOf(source, opener, bodyStart, close);
    }

    #dollarCloser(open) {
      const source = this.#source;
      if (this.#dollarOpen < open && open < this.#dollarStop) {
        if (this.#dollarClose === -1 || !dollarOpens(source, open)) return -1;
        return this.#dollarSolid > open ? this.#dollarClose : -1;
      }
      const { close, stop } = scanDollar(source, open);
      this.#dollarOpen = open;
      this.#dollarStop = stop;
      this.#dollarClose = close;
      if (close !== -1) {
        let solid = close - 1;
        while (/\s/.test(source[solid])) solid--;
        this.#dollarSolid = solid;
      }
      return close;
    }

    #closerFrom(opener, bodyStart) {
      const found = this.#closer[opener];
      if (this.#searchedFrom[opener] <= bodyStart && (found === -1 || bodyStart <= found)) return found;
      const close = closerIndex(this.#source, opener, bodyStart);
      this.#searchedFrom[opener] = bodyStart;
      this.#closer[opener] = close;
      return close;
    }
  }

  class MathBlockScan {
    #source;
    #unclosedDollarFrom = Infinity;
    #unclosedBracketFrom = Infinity;

    constructor(source) {
      this.#source = source;
    }

    at(from) {
      const opener = mathOpenerAt(this.#source, from);
      if (opener !== "$$" && opener !== "\\[") return undefined;
      const unclosedFrom = opener === "$$" ? this.#unclosedDollarFrom : this.#unclosedBracketFrom;
      if (unclosedFrom <= from) return undefined;
      const grammar = opener === "$$" ? MATH_BLOCK_DOLLAR : MATH_BLOCK_BRACKET;
      grammar.lastIndex = from;
      const match = grammar.exec(this.#source);
      if (!match) {
        if (opener === "$$") this.#unclosedDollarFrom = from;
        else this.#unclosedBracketFrom = from;
        return undefined;
      }
      return (match[1].trim() !== "") ? { raw: match[0], body: match[1] } : undefined;
    }
  }

  const spansByContext = new WeakMap();
  function mathSpanInContext(context, src) {
    const source = context.source ?? src;
    const end = context.end ?? source.length;
    let spans = spansByContext.get(context);
    if (!spans) spansByContext.set(context, (spans = new MathSpans(source)));
    const at = end - src.length;
    const span = spans.spanAt(at, end);
    return span && { ...span, end: span.end - at };
  }

  const blockScansByContext = new WeakMap();
  function mathBlockInContext(context, src) {
    const source = context.source;
    if (source === undefined) return mathBlockAt(src);
    let scan = blockScansByContext.get(context);
    if (!scan) blockScansByContext.set(context, (scan = new MathBlockScan(source)));
    return scan.at((context.end ?? source.length) - src.length);
  }

  window.OMP_MARKDOWN = { fenceCode, isSchemeLikeFilePath };
  window.OMP_MATH = {
    mathStartIndex,
    mathOpenerAt,
    mathSpanAt,
    mathBlockAt,
    MathSpans,
    MathBlockScan,
    mathSpanInContext,
    mathBlockInContext
  };

  if (!window.marked || !window.hljs) return;
  // Since marked 15, renderers get raw token fields: `href`, `title`,
  // `lang`, raw HTML and an autolink's `text` arrive unescaped and must be
  // escaped here (and the `autolink` flag itself needs marked 18).
  const escapeHtml = (s) => String(s)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
  // marked's own text escape (`escape(text, false)`): `<>"'` always, but a
  // well-formed `&…;` reference stays a reference — output is text-node
  // content, where a reference can only ever decode to a character.
  const escapeKeepingRefs = (s) => String(s)
    .replace(/[<>"']|&(?!(?:#\d{1,7}|#[xX][\da-fA-F]{1,6}|\w+);)/g, escapeHtml);
  // CommonMark decodes character references in a link destination or
  // title (`&amp;`, `&#106;`). Decode once, then check *and* emit that
  // same string: checking the raw text would let `&#106;avascript:`
  // pass as a relative URL and then decode to `javascript:` in the
  // attribute. Only `;`-terminated references are decoded, as CommonMark
  // requires — HTML text parsing would also expand legacy forms without
  // `;`, turning `?a=1&currency=USD` into `?a=1¤cy=USD`. Each one goes
  // through a <textarea> (RCDATA: decodes references, never creates
  // elements); a result longer than 2 UTF-16 units means the name was
  // invalid and only a prefix decoded, so the reference stays literal.
  const decoder = document.createElement('textarea');
  const decodeRefs = (s) => s.includes('&')
    ? s.replace(/&(?:#\d{1,7}|#[xX][\da-fA-F]{1,6}|[A-Za-z][A-Za-z\d]{1,31});/g, (ref) => {
        decoder.innerHTML = ref;
        const v = decoder.value;
        return v.length <= 2 ? v : ref;
      })
    : s;

  const SAFE_TAGS = new Set([
    "kbd", "sub", "sup", "mark", "ruby", "rt", "rp", "abbr", "s", "del", "ins",
    "small", "q", "cite", "span", "b", "i", "strong", "em", "u",
    "details", "summary", "dl", "dt", "dd", "br", "hr",
    "table", "thead", "tbody", "tr", "th", "td",
    "audio", "video", "source"
  ]);

  const SAFE_ATTRS = new Set([
    "class", "title", "open", "align", "span", "colspan", "rowspan",
    "width", "height", "controls", "preload", "loop", "muted", "poster", "src", "type"
  ]);

  function isSafeMediaUrl(url) {
    if (!url) return false;
    const trimmed = url.trim();
    if (/^\s*(javascript|vbscript|data:text|data:application)/i.test(trimmed)) return false;
    if (/^https?:\/\//i.test(trimmed)) return true;
    if (/^data:(image|audio|video)\//i.test(trimmed)) return true;
    if (/^blob:/i.test(trimmed)) return true;
    return isSchemeLikeFilePath(trimmed);
  }

  function sanitizeSafeHtml(html) {
    if (!html.includes('<')) return escapeHtml(html);
    return html.replace(/<\/?([a-zA-Z][\w-]*)([^>]*)>/g, (match, tagName, rawAttrs) => {
      const lowerTag = tagName.toLowerCase();
      if (!SAFE_TAGS.has(lowerTag)) {
        return escapeHtml(match);
      }
      if (match.startsWith('</')) {
        return `</${lowerTag}>`;
      }
      let cleanAttrs = '';
      const attrRegex = /([a-zA-Z0-9_-]+)(?:\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+)))?/g;
      let m;
      while ((m = attrRegex.exec(rawAttrs)) !== null) {
        const attrName = m[1].toLowerCase();
        if (attrName.startsWith('on')) continue;
        const val = m[2] ?? m[3] ?? m[4] ?? '';
        if (attrName === 'src' || attrName === 'poster') {
          if (!isSafeMediaUrl(val)) continue;
          cleanAttrs += ` ${attrName}="${escapeHtml(val)}"`;
        } else if (attrName === 'style') {
          const safeStyle = val.split(';').filter(rule => {
            const [k, v] = rule.split(':').map(s => s?.trim().toLowerCase());
            if (!k || !v) return false;
            if (v.includes('url(') || v.includes('expression') || v.includes('behavior')) return false;
            return ['color', 'background-color', 'font-size', 'font-weight', 'text-align', 'text-decoration', 'border-radius', 'padding', 'margin'].includes(k);
          }).join(';');
          if (safeStyle) {
            cleanAttrs += ` style="${escapeHtml(safeStyle)}"`;
          }
        } else if (SAFE_ATTRS.has(attrName)) {
          cleanAttrs += val ? ` ${attrName}="${escapeHtml(val)}"` : ` ${attrName}`;
        }
      }
      if ((lowerTag === 'audio' || lowerTag === 'video') && !cleanAttrs.includes('controls')) {
        cleanAttrs += ' controls';
      }
      const selfClosing = match.endsWith('/>') ? ' />' : '>';
      return `<${lowerTag}${cleanAttrs}${selfClosing}`;
    });
  }

  function renderMath(token) {
    const text = token.text ?? '';
    const display = Boolean(token.display);
    if (!window.katex) {
      const raw = token.raw ?? (display ? `$$\n${text}\n$$` : `$${text}$`);
      return display
        ? `<div class="katex-display-container"><span class="katex-raw">${escapeHtml(raw)}</span></div>\n`
        : `<span class="katex-raw">${escapeHtml(raw)}</span>`;
    }
    try {
      const rendered = window.katex.renderToString(text, {
        displayMode: display,
        throwOnError: false,
        trust: false,
        strict: false
      });
      return display
        ? `<div class="katex-display-container"><span class="tr-math">${rendered}</span></div>\n`
        : `<span class="tr-math-inline">${rendered}</span>`;
    } catch (_) {
      const raw = token.raw ?? (display ? `$$\n${text}\n$$` : `$${text}$`);
      return display
        ? `<div class="katex-display-container"><span class="katex-raw">${escapeHtml(raw)}</span></div>\n`
        : `<span class="katex-raw">${escapeHtml(raw)}</span>`;
    }
  }

  window.marked.use({
    extensions: [
      {
        name: "mathBlock",
        level: "block",
        tokenizer(src) {
          const block = mathBlockInContext(this, src);
          if (!block) return undefined;
          return { type: "mathBlock", raw: block.raw, text: block.body, display: true };
        },
        renderer(token) {
          return renderMath(token);
        }
      },
      {
        name: "mathInline",
        level: "inline",
        start(src) {
          return mathStartIndex(src);
        },
        tokenizer(src) {
          const span = mathSpanInContext(this, src);
          if (!span || span.body.includes("`")) return undefined;
          return { type: "mathInline", raw: src.slice(0, span.end), text: span.body, display: span.display };
        },
        renderer(token) {
          return renderMath(token);
        }
      }
    ],
    renderer: {
      // One renderer for both token kinds: a block (`<div>…`, a comment, a
      // `<script>` element on its own lines) and an inline tag inside a
      // paragraph, list item, heading, table cell or link text. A block
      // keeps paragraph spacing; its line breaks collapse like any text.
      html({ text, block }) {
        const cleaned = sanitizeSafeHtml(text);
        return block ? (cleaned.startsWith('<p>') || cleaned.startsWith('<div') || cleaned.startsWith('<table') || cleaned.startsWith('<details') || cleaned.startsWith('<audio') || cleaned.startsWith('<video') ? cleaned : '<p>' + cleaned.trimEnd() + '</p>\n') : cleaned;
      },
      blockquote(token) {
        const raw = token.text ?? '';
        const match = /^\s*\[!(NOTE|TIP|IMPORTANT|WARNING|CAUTION)\](?:\s*\n)?/i.exec(raw);
        if (match) {
          const kind = match[1].toLowerCase();
          const body = raw.slice(match[0].length);
          const bodyHtml = window.marked.parse(body);
          return `<div class="md-callout md-callout-${kind}"><div class="md-callout-title"><span class="chip md-callout-badge">${kind.toUpperCase()}</span></div><div class="md-callout-body">${bodyHtml}</div></div>\n`;
        }
        return false;
      },
      // The `html` escape above does not stop the lexer from *acting* on a
      // tag: an inline `<pre>`, `<code>`, `<kbd>` or `<script>` switches it
      // into raw-block mode until the matching close tag (across paragraphs,
      // since one lexer queues the whole document), and every text token
      // lexed meanwhile is the raw source flagged `escaped: true`, which the
      // default renderer emits verbatim — so `wrap it in <code>` followed by
      // `<img/src=x onerror=…>` would render live. `escaped` is set nowhere
      // else; every other text token falls through (`false`) to marked's
      // default, which escapes it.
      text(token) {
        return token.escaped && !token.tokens ? escapeKeepingRefs(token.text) : false;
      },
      code(token) {
        const code     = token.text ?? '';
        const lang     = (token.lang ?? '').trim().split(/\s/)[0].toLowerCase();
        const raw      = token.raw ?? '';
        const isClosed = /(?:^|\n)(?:`{3,}|~{3,})[ \t]*$/.test(raw);
        if (lang === 'chart') {
          return '<div class="md-slot md-slot-chart" data-slot-type="chart" data-spec="' + escapeHtml(code) + '" data-closed="' + (isClosed ? '1' : '0') + '"></div>';
        }
        if (lang === 'mermaid') {
          return '<div class="md-slot md-slot-mermaid" data-slot-type="mermaid" data-code="' + escapeHtml(code) + '" data-closed="' + (isClosed ? '1' : '0') + '"></div>';
        }
        let highlighted;
        try {
          highlighted = lang && hljs.getLanguage(lang)
            ? hljs.highlight(code, { language: lang }).value
            : hljs.highlightAuto(code).value;
        } catch (_) {
          highlighted = escapeHtml(code);
        }
        const cls = lang ? ' language-' + escapeHtml(lang) : '';
        // Copy button (issue #33): plain markup, no handler — the release
        // CSP forbids inline ones. The markdown container's delegated
        // onClick (copyCodeBlockClick, ui/copy-button.jsx) copies the
        // sibling <code>'s textContent; its icon and label are CSS, so the
        // button adds no text to a manual selection.
        return '<pre class="code-block copy-host"><code class="hljs' + cls + '">' +
               highlighted + '</code>' +
               '<button class="copy-btn is-floating" type="button" aria-label="Copy code" title="Copy code"></button>' +
               '</pre>';
      },
      link({ href, title, tokens, text, autolink }) {
        // Only these schemes can ever be a legitimate chat/diff/plan
        // link; anything else (a script scheme, or an href `new URL`
        // can't even parse) renders as plain text - `inner` is the
        // rendered-safe inner HTML, so no anchor, no navigation
        // attempt, no policy hook to bypass.
        const inner = autolink ? escapeHtml(text) : this.parser.parseInline(tokens);
        // An autolink `<…>` resolves no references, so it stays literal.
        const url = autolink ? href : decodeRefs(href);
        let scheme = null;
        try { scheme = new URL(url, 'https://omp-desktop.invalid/').protocol; }
        catch (_) { /* leave scheme null - falls through to plain text */ }
        // A file path that only looks like a scheme stays a link for the
        // file preview (app-live.jsx), which claims the click; unclaimed,
        // navigation_guard.rs blocks it like any non-web scheme.
        if (scheme !== 'http:' && scheme !== 'https:' && scheme !== 'mailto:' && !isSchemeLikeFilePath(url)) {
          return inner;
        }
        let safeHref;
        try { safeHref = escapeHtml(encodeURI(url).replace(/%25/g, '%')); }
        catch (_) { return inner; }
        let out = '<a href="' + safeHref + '"';
        if (title) out += ' title="' + escapeHtml(decodeRefs(title)) + '"';
        return out + '>' + inner + '</a>';
      },
      image(token) {
        const href      = token.href || '';
        const text      = token.text || '';
        const title     = token.title || '';
        const url       = decodeRefs(href);
        let scheme      = null;
        try { scheme = new URL(url, 'https://omp-desktop.invalid/').protocol; }
        catch (_) { /* leave scheme null */ }
        if (scheme && scheme !== 'http:' && scheme !== 'https:' && scheme !== 'data:' && !isSchemeLikeFilePath(url)) {
          return escapeHtml(text);
        }
        if (scheme === 'data:' && !url.startsWith('data:image/')) {
          return escapeHtml(text);
        }
        const safeSrc   = escapeHtml(encodeURI(url).replace(/%25/g, '%'));
        const safeAlt   = escapeHtml(text);
        const titleAttr = title ? ' data-title="' + escapeHtml(decodeRefs(title)) + '"' : '';
        return '<span class="md-slot md-slot-image" data-slot-type="image" data-src="' + safeSrc + '" data-alt="' + safeAlt + '"' + titleAttr + '></span>';
      }
    }
  });
})();
