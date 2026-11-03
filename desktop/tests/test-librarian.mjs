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

console.log(`test-librarian: ${n} checks passed`);
