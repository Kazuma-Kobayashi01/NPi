// app/librarian.js — pure helpers of the Librarian window (design/librarian/):
// what a reference looks like in the list, search and filter, sorting, and
// the edit form ↔ CSL-JSON mapping. Exposes `window.OMP_LIBRARIAN`.
//
// An entry is what Rust sends (librarian/store.rs): `{ id, citekey, csl,
// source, createdAt, updatedAt, notes, tags, groups }`. The form edits the
// common CSL fields and carries every other field of the record through.
(function () {
  const TYPES = [
    ["article-journal", "Journal article"], ["paper-conference", "Conference paper"], ["book", "Book"],
    ["chapter", "Book chapter"], ["thesis", "Thesis"], ["report", "Report"],
    ["webpage", "Web page"], ["manuscript", "Manuscript"], ["document", "Other"],
  ];

  const text = v => (typeof v === "string" ? v.trim() : "");

  function personText(p) {
    if (!p) return "";
    const family = text(p.family), given = text(p.given);
    if (family && given) return `${family}, ${given}`;
    return family || given || text(p.literal);
  }

  const personName = p => text(p?.family) || text(p?.literal) || text(p?.given);

  /** "Family, Given", "Given Family" or a single name (kept as a literal). */
  function parsePerson(line) {
    const s = line.trim();
    const comma = s.indexOf(",");
    if (comma >= 0) return { family: s.slice(0, comma).trim(), given: s.slice(comma + 1).trim() };
    const space = s.lastIndexOf(" ");
    if (space > 0) return { family: s.slice(space + 1).trim(), given: s.slice(0, space).trim() };
    return { literal: s };
  }

  const yearOf = csl => {
    const y = csl?.issued?.["date-parts"]?.[0]?.[0];
    const n = typeof y === "string" ? parseInt(y, 10) : y;
    return Number.isInteger(n) ? n : null;
  };

  const ENTITIES = { amp: "&", lt: "<", gt: ">", quot: '"', apos: "'", nbsp: " " };

  /** A title as a publisher's metadata or a PDF's Markdown left it (`Λ<sub>σ</sub>`, `<i>…</i>`, `&amp;`): plain text. */
  const plainText = value => String(value ?? "")
    .replace(/<\/?(?:sub|sup|i|b|em|strong|span|scp|u|mml:[a-z]+|jats:[a-z]+)(?:\s[^>]*)?>/gi, "")
    .replace(/&(amp|lt|gt|quot|apos|nbsp);/g, (_, name) => ENTITIES[name])
    .replace(/\s+/g, " ").trim();

  /** The list row of an entry: authors as "Vaswani, Shazeer et al.". */
  function display(entry) {
    const csl = entry.csl ?? {};
    const names = (csl.author ?? []).map(personName).filter(Boolean);
    const authors = names.length > 2 ? `${names[0]} et al.` : names.join(", ");
    return {
      title: plainText(csl.title) || "(untitled)", authors, year: yearOf(csl),
      container: plainText(csl["container-title"]), kind: text(csl.type) || "document",
    };
  }

  const fold = s => String(s ?? "").toLowerCase();

  /** The text a query is matched against: every field the user can see. */
  function haystack(entry) {
    const csl = entry.csl ?? {};
    return fold([
      csl.title, (csl.author ?? []).map(personText).join(" "), yearOf(csl), csl["container-title"],
      csl.DOI, csl.publisher, csl.keyword, entry.citekey, (entry.tags ?? []).join(" "), entry.notes,
    ].filter(v => v !== null && v !== undefined).join(" \n"));
  }

  /** Every whitespace-separated term must occur (case-insensitive substring, so a Chinese word works). */
  function matches(entry, query) {
    const terms = fold(query).split(/\s+/).filter(Boolean);
    if (!terms.length) return true;
    const hay = haystack(entry);
    return terms.every(t => hay.includes(t));
  }

  /** A reference that lacks what a citation needs: a DOI or URL, a venue, or a year. */
  const incomplete = entry => {
    const csl = entry.csl ?? {};
    return !(text(csl.DOI) || text(csl.URL)) || !text(csl["container-title"]) || yearOf(csl) === null;
  };

  /** A title reduced to its words, so punctuation, case and markup do not hide a duplicate. */
  const titleKey = csl => plainText(csl?.title).toLowerCase().replace(/[^\p{L}\p{N}]+/gu, " ").trim();

  /** Ids of the references that share a DOI, or a title and year, with another one. */
  function duplicateIds(entries) {
    const seen = new Map();
    const out = new Set();
    const note = (key, id) => {
      if (seen.has(key)) { out.add(id); out.add(seen.get(key)); } else seen.set(key, id);
    };
    for (const e of entries) {
      const doi = text(e.csl?.DOI).toLowerCase();
      if (doi) note(`doi:${doi}`, e.id);
      const title = titleKey(e.csl);
      if (title) note(`title:${title}|${yearOf(e.csl) ?? ""}`, e.id);
    }
    return out;
  }

  /** The built-in filters of the sidebar: `id → (entry, duplicates) → bool`. */
  const SMART = {
    incomplete: e => incomplete(e),
    nofile: e => (e.attachments ?? []).length === 0,
    duplicates: (e, dups) => dups.has(e.id),
    suggestions: e => (e.suggestions ?? []).length > 0,
  };

  /** How many references each built-in filter holds. */
  function smartCounts(entries) {
    const dups = duplicateIds(entries);
    return Object.fromEntries(Object.entries(SMART).map(([id, test]) => [id, entries.filter(e => test(e, dups)).length]));
  }

  /** `scope`: { group: id | null, tag: string | null, smart: a SMART id | undefined }; `query` as typed. */
  function visible(entries, { group = null, tag = null, smart = null, query = "" } = {}) {
    const dups = smart === "duplicates" ? duplicateIds(entries) : null;
    return entries.filter(e =>
      (group === null || (e.groups ?? []).includes(group)) &&
      (tag === null || (e.tags ?? []).some(t => fold(t) === fold(tag))) &&
      (smart === null || (SMART[smart]?.(e, dups) ?? true)) &&
      matches(e, query));
  }

  /** The selection after a click on `id` in the list `order` (ids as shown): a plain click selects it alone, Cmd/Ctrl
   *  toggles it, Shift takes the range from `anchor`. Returns `{ ids, anchor }`. */
  function clickSelect({ ids, anchor }, id, order, { toggle = false, range = false } = {}) {
    if (range && anchor !== null && order.includes(anchor) && order.includes(id)) {
      const [a, b] = [order.indexOf(anchor), order.indexOf(id)].sort((x, y) => x - y);
      return { ids: order.slice(a, b + 1), anchor };
    }
    if (toggle) {
      const next = ids.includes(id) ? ids.filter(x => x !== id) : [...ids, id];
      return { ids: next, anchor: id };
    }
    return { ids: [id], anchor: id };
  }

  /** The id `delta` rows away from `id` in `order`, kept inside the list; the first row when `id` is not in it. */
  function stepSelect(order, id, delta) {
    if (order.length === 0) return null;
    const at = order.indexOf(id);
    if (at < 0) return order[0];
    return order[Math.max(0, Math.min(order.length - 1, at + delta))];
  }

  const COMPARE = {
    added: (a, b) => a.createdAt - b.createdAt || a.id - b.id,
    year: (a, b) => (yearOf(a.csl) ?? 0) - (yearOf(b.csl) ?? 0),
    author: (a, b) => fold(display(a).authors).localeCompare(fold(display(b).authors)),
    title: (a, b) => fold(display(a).title).localeCompare(fold(display(b).title)),
  };
  const SORT_KEYS = Object.keys(COMPARE);

  /** A sorted copy; `dir` "asc" | "desc" (ties keep the order they were added in). */
  function sorted(entries, key = "added", dir = "desc") {
    const cmp = COMPARE[key] ?? COMPARE.added;
    const out = [...entries].sort((a, b) => cmp(a, b) || COMPARE.added(a, b));
    return dir === "desc" ? out.reverse() : out;
  }

  /** `[{ tag, count }]`, most used first. */
  function tagCounts(entries) {
    const counts = new Map();
    for (const e of entries) for (const t of e.tags ?? []) {
      const k = fold(t);
      counts.set(k, { tag: counts.get(k)?.tag ?? t, count: (counts.get(k)?.count ?? 0) + 1 });
    }
    return [...counts.values()].sort((a, b) => b.count - a.count || a.tag.localeCompare(b.tag));
  }

  const FIELDS = [
    ["container-title", "container"], ["volume", "volume"], ["issue", "issue"], ["page", "page"],
    ["publisher", "publisher"], ["DOI", "doi"], ["URL", "url"], ["abstract", "abstract"],
  ];

  /** The edit form of a record. */
  function toForm(csl = {}) {
    const form = {
      type: text(csl.type) || "article-journal", title: text(csl.title),
      authors: (csl.author ?? []).map(personText).filter(Boolean).join("\n"),
      year: yearOf(csl) ?? "",
    };
    for (const [key, name] of FIELDS) form[name] = text(csl[key]);
    return form;
  }

  /** `csl` with the form's values applied; fields the form lacks are kept, and an emptied one is removed. */
  function fromForm(csl = {}, form) {
    const out = { ...csl, type: form.type || "document" };
    const put = (key, value) => { if (value) out[key] = value; else delete out[key]; };
    put("title", form.title.trim());
    const authors = form.authors.split("\n").map(l => l.trim()).filter(Boolean).map(parsePerson);
    if (authors.length) out.author = authors; else delete out.author;
    const year = parseInt(String(form.year).trim(), 10);
    if (Number.isInteger(year) && year > 0) out.issued = { "date-parts": [[year]] }; else delete out.issued;
    for (const [key, name] of FIELDS) put(key, String(form[name] ?? "").trim());
    return out;
  }

  /** Whether the form differs from the record it was made from. */
  const formChanged = (csl, form) => JSON.stringify(fromForm(csl, form)) !== JSON.stringify(csl ?? {});

  /** A tag line "a, b; c" → ["a","b","c"]. */
  const parseTags = line => line.split(/[,;，；]/).map(t => t.trim()).filter(Boolean);

  /** A plain `Author (Year). Title. Container.` line; the window now formats through Rust (citation styles), this stays as a fallback for tools. */
  function plainCitation(entry) {
    const d = display(entry);
    const parts = [d.authors ? `${d.authors}${d.year ? ` (${d.year})` : ""}.` : d.year ? `(${d.year}).` : "", `${d.title}.`, d.container ? `${d.container}.` : ""];
    return parts.filter(Boolean).join(" ");
  }

  const FORM_LABELS = {
    type: "Type", title: "Title", authors: "Authors", year: "Year", volume: "Volume", issue: "Issue", page: "Pages",
    container: "Journal / book / conference", publisher: "Publisher", doi: "DOI", url: "URL", abstract: "Abstract",
  };
  const norm = v => String(v ?? "").trim();

  /** The form fields where a looked-up record has something the form lacks or says differently:
   *  `[{ key, label, current, found, differs }]`, in form order, only fields the record fills. */
  function lookupDiff(form, found = {}) {
    const other = toForm(found);
    return Object.keys(FORM_LABELS)
      .filter(key => (key === "type" ? text(found.type) !== "" : norm(other[key]) !== ""))
      .map(key => ({ key, label: FORM_LABELS[key], current: norm(form[key]), found: norm(other[key]),
        differs: norm(form[key]) !== norm(other[key]) }));
  }

  /** The form with the `keys` of a lookup diff taken from the record. */
  function applyLookup(form, found = {}, keys = []) {
    const other = toForm(found);
    const out = { ...form };
    for (const key of keys) if (key in FORM_LABELS) out[key] = other[key];
    return out;
  }

  /** A not-yet-saved entry shown in the form for a PDF draft (`draft` from library_pdf_pick). */
  const draftEntry = draft => ({
    id: -1, citekey: "", csl: draft.csl, source: "pdf", createdAt: 0, updatedAt: 0,
    notes: "", tags: [], groups: [], attachments: [], draft: true,
  });

  /** The form names a PDF draft's rules filled (CSL `guessed` names → form names), for the "check" marks. */
  const GUESS_FORM = { title: "title", author: "authors", issued: "year", DOI: "doi", URL: "url" };
  const guessedFields = draft => new Set((draft?.guessed ?? []).map(g => GUESS_FORM[g]).filter(Boolean));

  /** The DOIs in pasted text, or null when it holds anything else (then it is a reference file's text).
   *  Takes doi.org links and `doi:`; an arXiv id or link is given as its DOI (10.48550/arXiv.…). */
  function parseIdentifiers(text) {
    const out = [];
    for (const token of String(text ?? "").split(/[\s,;]+/).filter(Boolean)) {
      const doi = token.match(/^(?:https?:\/\/(?:dx\.)?doi\.org\/|doi:)?(10\.\d{4,9}\/\S+)$/i);
      const arxiv = token.match(/^(?:https?:\/\/arxiv\.org\/(?:abs|pdf)\/|arxiv:)?(\d{4}\.\d{4,5})(?:v\d+)?(?:\.pdf)?$/i);
      if (!doi && !arxiv) return null;
      const id = doi ? doi[1].replace(/[.)]+$/, "") : `10.48550/arXiv.${arxiv[1]}`;
      if (!out.some(x => x.toLowerCase() === id.toLowerCase())) out.push(id);
    }
    return out.length > 0 ? out : null;
  }

  /** The page of a form's DOI or URL field, or null when it is not one a browser should open. */
  function linkUrl(key, value) {
    const v = String(value ?? "").trim();
    if (!v) return null;
    if (key === "doi") return `https://doi.org/${v.replace(/^https?:\/\/(dx\.)?doi\.org\//i, "").replace(/^doi:/i, "")}`;
    return /^https?:\/\//i.test(v) ? v : null;
  }

  /** `[@a; @b]`: how a draft cites these keys (library_cite_draft reads this form). */
  function citeMarker(keys) {
    return `[${keys.map(k => `@${k}`).join("; ")}]`;
  }

  window.OMP_LIBRARIAN = {
    lookupDiff, applyLookup, draftEntry, guessedFields, FORM_LABELS,
    TYPES, SORT_KEYS, display, matches, visible, sorted, tagCounts, plainText, incomplete, duplicateIds, smartCounts, clickSelect, stepSelect, parseIdentifiers, linkUrl, citeMarker,
    toForm, fromForm, formChanged, parseTags, parsePerson, plainCitation, personText,
  };
})();
