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

  /** The list row of an entry: authors as "Vaswani, Shazeer et al.". */
  function display(entry) {
    const csl = entry.csl ?? {};
    const names = (csl.author ?? []).map(personName).filter(Boolean);
    const authors = names.length > 2 ? `${names[0]} et al.` : names.join(", ");
    return {
      title: text(csl.title) || "(untitled)", authors, year: yearOf(csl),
      container: text(csl["container-title"]), kind: text(csl.type) || "document",
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

  /** `scope`: { group: id | null, tag: string | null }; `query` as typed. */
  function visible(entries, { group = null, tag = null, query = "" } = {}) {
    return entries.filter(e =>
      (group === null || (e.groups ?? []).includes(group)) &&
      (tag === null || (e.tags ?? []).some(t => fold(t) === fold(tag))) &&
      matches(e, query));
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

  /** The plain-text citation used for "copy": `Author (Year). Title. Container.` — a stand-in until citation styles (L2). */
  function plainCitation(entry) {
    const d = display(entry);
    const parts = [d.authors ? `${d.authors}${d.year ? ` (${d.year})` : ""}.` : d.year ? `(${d.year}).` : "", `${d.title}.`, d.container ? `${d.container}.` : ""];
    return parts.filter(Boolean).join(" ");
  }

  window.OMP_LIBRARIAN = {
    TYPES, SORT_KEYS, display, matches, visible, sorted, tagCounts,
    toForm, fromForm, formChanged, parseTags, parsePerson, plainCitation, personText,
  };
})();
