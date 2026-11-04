// app/librarian.js: list rows, search, filters, sorting and the edit form ↔ CSL-JSON mapping.
import assert from "node:assert/strict";
import fs from "node:fs";
import { fileURLToPath } from "node:url";

const win = {};
new Function("window", fs.readFileSync(fileURLToPath(new URL("../src/app/librarian.js", import.meta.url)), "utf8"))(win);
const L = win.OMP_LIBRARIAN;
let n = 0;
const check = (name, fn) => { try { fn(); n++; } catch (e) { console.error(`FAILED: ${name}`); throw e; } };

let id = 0;
const entry = (csl, extra = {}) => ({ id: ++id, citekey: `k${id}`, createdAt: id, updatedAt: id, notes: "", tags: [], groups: [], csl, ...extra });
const paper = (title, year, authors = [["Smith", "Jo"]], more = {}) => ({
  type: "article-journal", title, issued: { "date-parts": [[year]] }, author: authors.map(([family, given]) => ({ family, given })), ...more,
});
const A = entry(paper("Attention Is All You Need", 2017, [["Vaswani", "Ashish"], ["Shazeer", "Noam"], ["Parmar", "Niki"]], { "container-title": "NeurIPS" }), { tags: ["Transformers", "nlp"], groups: [1] });
const B = entry(paper("深度学习综述", 2020, [["王", "小明"]]), { notes: "读过，第三章重要", tags: ["survey"] });
const C = entry(paper("Graph Networks", 2019, [["Lee", "Al"]], { DOI: "10.1/gn" }), { groups: [1, 2] });
const ALL = [A, B, C];

check("a row shows the title, 'first et al.' for three authors, year and journal", () => {
  assert.deepEqual(L.display(A), { title: "Attention Is All You Need", authors: "Vaswani et al.", year: 2017, container: "NeurIPS", kind: "article-journal" });
  assert.equal(L.display(entry({ author: [{ family: "Lee" }, { literal: "WHO" }] })).authors, "Lee, WHO");
  assert.equal(L.display(entry({})).title, "(untitled)");
});

check("search needs every term, ignores case, and finds Chinese words, notes, tags, DOI and citekeys", () => {
  const ids = q => L.visible(ALL, { query: q }).map(e => e.id);
  assert.deepEqual(ids("attention vaswani"), [A.id]);
  assert.deepEqual(ids("ATTENTION lee"), []);
  assert.deepEqual(ids("综述"), [B.id]);
  assert.deepEqual(ids("第三章"), [B.id]);
  assert.deepEqual(ids("transformers"), [A.id]);
  assert.deepEqual(ids("10.1/gn"), [C.id]);
  assert.deepEqual(ids(A.citekey), [A.id]);
  assert.deepEqual(ids("  "), ALL.map(e => e.id));
});

check("group and tag scopes combine with the query", () => {
  assert.deepEqual(L.visible(ALL, { group: 1 }).map(e => e.id), [A.id, C.id]);
  assert.deepEqual(L.visible(ALL, { group: 1, query: "graph" }).map(e => e.id), [C.id]);
  assert.deepEqual(L.visible(ALL, { tag: "NLP" }).map(e => e.id), [A.id]);
  assert.deepEqual(L.visible(ALL, { group: 2, tag: "nlp" }), []);
});

check("sorting by year, author or title, either way, keeps ties in added order", () => {
  assert.deepEqual(L.sorted(ALL, "year", "asc").map(e => e.id), [A.id, C.id, B.id]);
  assert.deepEqual(L.sorted(ALL, "year", "desc").map(e => e.id), [B.id, C.id, A.id]);
  assert.deepEqual(L.sorted(ALL, "title", "asc").map(e => display(e).title), ["Attention Is All You Need", "Graph Networks", "深度学习综述"].map(String));
  assert.deepEqual(L.sorted(ALL, "added", "desc").map(e => e.id), [C.id, B.id, A.id]);
  assert.deepEqual(ALL.map(e => e.id), [A.id, B.id, C.id]);
  const same = [entry(paper("x", 2000)), entry(paper("y", 2000))];
  assert.deepEqual(L.sorted(same, "year", "asc").map(e => e.id), same.map(e => e.id));
});
function display(e) { return L.display(e); }

check("tags are counted without regard to case, most used first", () => {
  const tagged = [entry({}, { tags: ["NLP", "x"] }), entry({}, { tags: ["nlp"] }), entry({}, { tags: ["y"] })];
  assert.deepEqual(L.tagCounts(tagged), [{ tag: "NLP", count: 2 }, { tag: "x", count: 1 }, { tag: "y", count: 1 }]);
});

check("the form shows the record and writes back only what changed, keeping unknown fields", () => {
  const csl = paper("Old", 2018, [["Lee", "Al"], ["Kim", "Bo"]], { "container-title": "J.", ISSN: "1234-5678", custom: { keep: true } });
  const form = L.toForm(csl);
  assert.deepEqual([form.title, form.authors, form.year, form.container], ["Old", "Lee, Al\nKim, Bo", 2018, "J."]);
  assert.equal(L.formChanged(csl, form), false);
  const out = L.fromForm(csl, { ...form, title: "New", year: "2021", container: "", authors: "Wu Chen\nUNESCO\n" });
  assert.deepEqual(out.author, [{ family: "Chen", given: "Wu" }, { literal: "UNESCO" }]);
  assert.deepEqual([out.title, out.issued, out["container-title"], out.ISSN, out.custom], ["New", { "date-parts": [[2021]] }, undefined, "1234-5678", { keep: true }]);
  assert.equal(L.formChanged(csl, { ...form, title: "New" }), true);
  assert.equal(L.fromForm({}, { ...L.toForm({}), year: "n.d.", title: "T" }).issued, undefined);
});

check("tag lines split on commas and semicolons of either width", () => {
  assert.deepEqual(L.parseTags("a, b；c ,, "), ["a", "b", "c"]);
});

check("the plain citation reads Author (Year). Title. Journal.", () => {
  assert.equal(L.plainCitation(A), "Vaswani et al. (2017). Attention Is All You Need. NeurIPS.");
  assert.equal(L.plainCitation(entry({ title: "Bare" })), "Bare.");
});

check("a lookup offers the fields the record fills and marks the ones that differ", () => {
  const form = L.toForm({ title: "Draft title", author: [{ family: "Smith", given: "J." }], DOI: "10.1/x" });
  const found = { title: "Draft title", author: [{ family: "Smith", given: "John" }], issued: { "date-parts": [[2020]] }, "container-title": "J. Things", DOI: "10.1/x" };
  const rows = L.lookupDiff(form, found);
  assert.deepEqual(rows.map(r => [r.key, r.differs]), [["title", false], ["authors", true], ["year", true], ["container", true], ["doi", false]]);
  assert.deepEqual(rows.find(r => r.key === "authors"), { key: "authors", label: "Authors", current: "Smith, J.", found: "Smith, John", differs: true });
  const applied = L.applyLookup(form, found, ["year", "container", "nope"]);
  assert.deepEqual([applied.year, applied.container, applied.title, applied.authors], [2020, "J. Things", "Draft title", "Smith, J."]);
  assert.equal(L.lookupDiff(form, {}).length, 0);
});

check("a PDF draft is an unsaved entry and names the form fields its rules guessed", () => {
  const draft = { csl: { title: "T", author: [{ literal: "A" }], issued: { "date-parts": [[2020]] }, DOI: "10.1/x" }, guessed: ["title", "author", "issued", "DOI", "bogus"], hash: "h" };
  const e = L.draftEntry(draft);
  assert.deepEqual([e.id, e.draft, e.attachments, e.tags], [-1, true, [], []]);
  assert.deepEqual([...L.guessedFields(draft)], ["title", "authors", "year", "doi"]);
  assert.equal(L.guessedFields(undefined).size, 0);
});


check("pasted identifiers, links and citation markers", () => {
  assert.deepEqual(L.parseIdentifiers("10.1038/nature14539\nhttps://doi.org/10.1234/ab.c.\ndoi:10.1038/NATURE14539"), ["10.1038/nature14539", "10.1234/ab.c"]);
  assert.deepEqual(L.parseIdentifiers("arXiv:1706.03762v5, https://arxiv.org/abs/2101.00001"), ["10.48550/arXiv.1706.03762", "10.48550/arXiv.2101.00001"]);
  assert.equal(L.parseIdentifiers("@article{a, title={x}}"), null);
  assert.equal(L.parseIdentifiers("10.1/x and more words"), null);
  assert.equal(L.parseIdentifiers("  "), null);
  assert.equal(L.linkUrl("doi", "https://dx.doi.org/10.1/x"), "https://doi.org/10.1/x");
  assert.equal(L.linkUrl("doi", "doi:10.1/x"), "https://doi.org/10.1/x");
  assert.equal(L.linkUrl("url", "javascript:alert(1)"), null);
  assert.equal(L.linkUrl("url", "https://a.test/x"), "https://a.test/x");
  assert.equal(L.linkUrl("doi", ""), null);
  assert.equal(L.citeMarker(["a"]), "[@a]");
  assert.equal(L.citeMarker(["a", "b"]), "[@a; @b]");
});
check("markup, built-in filters and the list selection", () => {
  assert.equal(L.plainText("Λ<sub>σ</sub>(φ) = Bu|<i>∂Ω</i> &amp; more"), "Λσ(φ) = Bu|∂Ω & more");
  assert.equal(L.display({ csl: { title: "A <jats:italic>b</jats:italic>" } }).title, "A b");
  const e = (id, csl, extra = {}) => ({ id, csl, createdAt: id, attachments: [], suggestions: [], tags: [], groups: [], ...extra });
  const full = { title: "One", DOI: "10.1/a", "container-title": "J", issued: { "date-parts": [[2020]] } };
  const list = [
    e(1, full, { attachments: [{ id: 1 }] }),
    e(2, { ...full, title: "Other", DOI: "10.1/A" }),
    e(3, { title: "Same  Title!", issued: { "date-parts": [[2019]] } }),
    e(4, { title: "same title", issued: { "date-parts": [[2019]] } }, { suggestions: [{ id: 1 }] }),
    e(5, { title: "same title", issued: { "date-parts": [[2021]] } }),
  ];
  assert.equal(L.incomplete(list[0]), false);
  assert.equal(L.incomplete(list[2]), true);
  assert.deepEqual([...L.duplicateIds(list)].sort(), [1, 2, 3, 4], "a DOI in another case, a title in other punctuation; another year is not a duplicate");
  assert.deepEqual(L.smartCounts(list), { incomplete: 3, nofile: 4, duplicates: 4, suggestions: 1 });
  assert.deepEqual(L.visible(list, { smart: "nofile" }).map(x => x.id), [2, 3, 4, 5]);
  assert.deepEqual(L.visible(list, { smart: "duplicates", query: "other" }).map(x => x.id), [2]);

  const order = [10, 20, 30, 40];
  let s = L.clickSelect({ ids: [], anchor: null }, 20, order);
  assert.deepEqual(s, { ids: [20], anchor: 20 });
  s = L.clickSelect(s, 40, order, { range: true });
  assert.deepEqual(s.ids, [20, 30, 40], "shift takes the range");
  s = L.clickSelect(s, 30, order, { toggle: true });
  assert.deepEqual(s.ids, [20, 40], "cmd toggles one");
  assert.deepEqual(L.clickSelect({ ids: [1], anchor: 99 }, 20, order, { range: true }).ids, [20], "an anchor no longer shown starts over");
  assert.equal(L.stepSelect(order, 20, 1), 30);
  assert.equal(L.stepSelect(order, 40, 1), 40);
  assert.equal(L.stepSelect(order, 10, -1), 10);
  assert.equal(L.stepSelect(order, 99, 1), 10);
  assert.equal(L.stepSelect([], 1, 1), null);
});

console.log(`test-librarian: ${n} checks passed`);
