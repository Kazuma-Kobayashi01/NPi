use super::*;
use crate::pdf_text::{pages_pdf, tiny_pdf};
use crate::test_support::TempDir;
use rusqlite::Connection;
use serde_json::json;

fn project(d: &TempDir, name: &str) -> PathBuf {
    let root = d.path().join(name);
    for dir in ["papers", "notes", "digests", "drafts"] {
        std::fs::create_dir_all(root.join(dir)).unwrap();
    }
    root
}

fn write(root: &Path, rel: &str, body: &[u8]) {
    std::fs::write(root.join(rel), body).unwrap();
}

fn paths(hits: &[Hit]) -> Vec<&str> {
    hits.iter().map(|h| h.path.as_str()).collect()
}

#[test]
fn fts5_is_compiled_in() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("CREATE VIRTUAL TABLE t USING fts5(a); INSERT INTO t VALUES ('hello world');").unwrap();
    let n: i64 = conn.query_row("SELECT count(*) FROM t WHERE t MATCH 'hello'", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 1);
}

#[test]
fn a_v4_library_migrates_to_v5_keeping_its_data_and_a_backup() {
    let d = TempDir::new("research", "migrate");
    let root = project(&d, "p");
    let lib = Library::open(&root).unwrap();
    let entry = lib.add(&json!({"title": "Kept", "type": "book"}), "manual").unwrap();
    lib.set_meta(entry.id, &["t".to_string()], "my note").unwrap();
    drop(lib);
    // Back to what a v4 file looked like.
    let conn = Connection::open(Library::path(&root)).unwrap();
    conn.execute_batch(
        "DROP TABLE search_fts; DROP TABLE search_units; DROP TABLE research_meta; DROP TABLE anchors;
         DROP TABLE blocks; DROP TABLE sections; DROP TABLE papers; PRAGMA user_version = 4;",
    )
    .unwrap();
    drop(conn);
    let lib = Library::open(&root).unwrap();
    let version: i64 = lib.conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
    assert_eq!(version, 5);
    let back = lib.get(entry.id).unwrap().unwrap();
    assert_eq!((back.csl["title"].as_str(), back.notes.as_str(), back.tags.len()), (Some("Kept"), "my note", 1));
    assert!(lib.index_status(&root).unwrap().problems.is_empty());
    assert!(Library::path(&root).with_extension("db.bak-v4").is_file());
}

#[test]
fn chinese_short_words_and_english_are_found() {
    let d = TempDir::new("research", "cjk");
    let root = project(&d, "p");
    write(&root, "notes/zh.md", "# 笔记\n\n本文综述文献检索的方法与评价。\n".as_bytes());
    write(&root, "notes/en.md", b"# Notes\n\nAttention is all you need for transformers.\n");
    write(&root, "notes/abc.md", b"The abc protocol.\n");
    write(&root, "notes/cab.md", b"A cab ride.\n");
    let lib = Library::open(&root).unwrap();
    let report = lib.index_project(&root, IndexOptions::default()).unwrap();
    assert_eq!(report.added, 4);
    for q in ["文献", "检索", "文献检索", "献检", "综述"] {
        assert_eq!(paths(&lib.search_blocks(q, None, 10).unwrap()), ["notes/zh.md"], "{q}");
    }
    assert!(lib.search_blocks("文献检查", None, 10).unwrap().is_empty());
    assert_eq!(paths(&lib.search_blocks("TRANSFORMERS attention", None, 10).unwrap()), ["notes/en.md"]);
    // A Latin word is matched as a prefix, not inside a word: 'ab' finds 'abc' but not 'cab'.
    assert_eq!(paths(&lib.search_blocks("ab", None, 10).unwrap()), ["notes/abc.md"]);
    // FTS syntax typed as a query is only text.
    for q in ["\"", "NEAR(", "a AND", "*", "col:x", "-"] {
        assert!(lib.search_blocks(q, None, 10).is_ok(), "{q}");
    }
    let hit = &lib.search_blocks("文献", None, 10).unwrap()[0];
    assert!(hit.snippet.contains("文献检索"));
    assert_eq!((hit.source_level, hit.page, hit.section.as_str()), ("note", None, "笔记"));
}

#[test]
fn pdf_hits_carry_page_anchor_and_source_level() {
    let d = TempDir::new("research", "pdf");
    let root = project(&d, "p");
    write(&root, "papers/a.pdf", &pages_pdf(&["Alpha first page words", "Beta second page words"]));
    write(&root, "digests/d.md", b"Beta digest line\n");
    let lib = Library::open(&root).unwrap();
    lib.index_project(&root, IndexOptions::default()).unwrap();
    let hits = lib.search_blocks("beta", None, 10).unwrap();
    let pdf = hits.iter().find(|h| h.path == "papers/a.pdf").unwrap();
    assert_eq!((pdf.page, pdf.source_level), (Some(2), "original"));
    assert!(pdf.anchor_id.is_some());
    let only = lib.search_blocks("beta", Some("papers"), 10).unwrap();
    assert_eq!(paths(&only), ["papers/a.pdf"]);
    let (quality, page): (String, Option<u32>) = lib
        .conn
        .query_row("SELECT locator_quality, page FROM anchors WHERE id = ?1", [pdf.anchor_id], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap();
    assert_eq!((quality.as_str(), page), ("page", Some(2)));
    let status = lib.index_status(&root).unwrap();
    assert_eq!((status.papers, status.indexed, status.unindexed), (2, true, 0));
    assert!(status.problems.is_empty(), "{:?}", status.problems);
}

#[test]
fn a_changed_file_is_reindexed_and_its_old_anchors_go_stale() {
    let d = TempDir::new("research", "stale");
    let root = project(&d, "p");
    write(&root, "papers/a.pdf", &tiny_pdf("original wording here"));
    let lib = Library::open(&root).unwrap();
    lib.index_project(&root, IndexOptions::default()).unwrap();
    let old_anchor = lib.search_blocks("original", None, 5).unwrap()[0].anchor_id.unwrap();
    let again = lib.index_project(&root, IndexOptions::default()).unwrap();
    assert_eq!((again.unchanged, again.updated), (1, 0));
    write(&root, "papers/a.pdf", &tiny_pdf("replacement text which is longer"));
    let report = lib.index_project(&root, IndexOptions::default()).unwrap();
    assert_eq!(report.updated, 1);
    assert!(lib.search_blocks("original", None, 5).unwrap().is_empty());
    let new = &lib.search_blocks("replacement", None, 5).unwrap()[0];
    assert_ne!(new.anchor_id, Some(old_anchor));
    let stale: i64 = lib.conn.query_row("SELECT stale FROM anchors WHERE id = ?1", [old_anchor], |r| r.get(0)).unwrap();
    assert_eq!(stale, 1);
    let status = lib.index_status(&root).unwrap();
    assert!(status.stale_anchors >= 1 && status.problems.is_empty());
    // A removed file leaves no blocks and is reported once.
    std::fs::remove_file(root.join("papers/a.pdf")).unwrap();
    assert_eq!(lib.index_project(&root, IndexOptions::default()).unwrap().removed, 1);
    assert_eq!(lib.index_project(&root, IndexOptions::default()).unwrap().removed, 0);
    assert!(lib.search_blocks("replacement", None, 5).unwrap().is_empty());
}

#[test]
fn a_quote_is_checked_on_its_page_and_a_changed_file_is_stale() {
    let d = TempDir::new("research", "quote");
    let root = project(&d, "p");
    write(&root, "papers/a.pdf", &pages_pdf(&["The Quick  Brown fox jumps", "Second page: lazy dogs sleep"]));
    write(&root, "notes/n.md", "A note about 文献\n检索 methods.\n".as_bytes());
    let lib = Library::open(&root).unwrap();
    // Never indexed: the check indexes the one file first.
    let ok = lib.check_quote(&root, "papers/a.pdf", Some(1), "quick brown FOX").unwrap();
    assert!(matches!(ok, QuoteCheck::Matched { anchor_id: Some(_), page: Some(1), .. }), "{ok:?}");
    assert_eq!(lib.check_quote(&root, "papers/a.pdf", Some(2), "quick brown fox").unwrap(), QuoteCheck::NotFound);
    assert_eq!(lib.check_quote(&root, "papers/a.pdf", Some(9), "quick").unwrap(), QuoteCheck::NotFound);
    assert!(lib.check_quote(&root, "papers/a.pdf", None, "quick").is_err());
    assert!(lib.check_quote(&root, "papers/a.pdf", Some(1), "  ").is_err());
    assert!(matches!(lib.check_quote(&root, "notes/n.md", None, "文献检索 methods").unwrap(), QuoteCheck::Matched { .. }));
    write(&root, "papers/a.pdf", &pages_pdf(&["The quick brown fox jumps over", "x"]));
    assert_eq!(lib.check_quote(&root, "papers/a.pdf", Some(1), "quick brown fox").unwrap(), QuoteCheck::Stale);
    lib.index_project(&root, IndexOptions::default()).unwrap();
    assert!(matches!(lib.check_quote(&root, "papers/a.pdf", Some(1), "jumps over").unwrap(), QuoteCheck::Matched { .. }));
    std::fs::remove_file(root.join("papers/a.pdf")).unwrap();
    assert_eq!(lib.check_quote(&root, "papers/a.pdf", Some(1), "jumps").unwrap(), QuoteCheck::Stale);
}

#[test]
fn projects_are_isolated_and_paths_cannot_leave() {
    let d = TempDir::new("research", "iso");
    let (a, b) = (project(&d, "a"), project(&d, "b"));
    write(&a, "notes/x.md", b"secretword in project a\n");
    write(&b, "notes/y.md", b"other thing in project b\n");
    write(&b, "papers/b.pdf", &tiny_pdf("bpaper words"));
    let (la, lb) = (Library::open(&a).unwrap(), Library::open(&b).unwrap());
    la.index_project(&a, IndexOptions::default()).unwrap();
    lb.index_project(&b, IndexOptions::default()).unwrap();
    assert!(lb.search_blocks("secretword", None, 10).unwrap().is_empty());
    assert!(la.search_blocks("bpaper", None, 10).unwrap().is_empty());
    for bad in ["../b/papers/b.pdf", "/etc/passwd", "papers/../../b/papers/b.pdf", "other/b.pdf", ".npi-academic/research.db"] {
        assert!(la.check_quote(&a, bad, Some(1), "bpaper").is_err(), "{bad}");
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(b.join("papers/b.pdf"), a.join("papers/link.pdf")).unwrap();
        assert!(la.check_quote(&a, "papers/link.pdf", Some(1), "bpaper").is_err());
        let report = la.index_project(&a, IndexOptions::default()).unwrap();
        assert_eq!(report.added + report.unchanged, 1, "a link is not followed");
        assert!(la.search_blocks("bpaper", None, 10).unwrap().is_empty());
    }
}

#[test]
fn a_scanned_pdf_is_recorded_not_guessed() {
    let d = TempDir::new("research", "scanned");
    let root = project(&d, "p");
    write(&root, "papers/scan.pdf", &tiny_pdf(""));
    write(&root, "papers/junk.pdf", b"%PDF-1.4\nnot really\n");
    let lib = Library::open(&root).unwrap();
    let report = lib.index_project(&root, IndexOptions::default()).unwrap();
    assert_eq!(report.scanned, ["papers/scan.pdf"]);
    assert_eq!(report.failed.len(), 1);
    let status = lib.index_status(&root).unwrap();
    assert_eq!((status.scanned.as_slice(), status.blocks), (["papers/scan.pdf".to_string()].as_slice(), 0));
    assert_eq!(status.failed, ["papers/junk.pdf"]);
    assert_eq!(
        lib.check_quote(&root, "papers/scan.pdf", Some(1), "anything").unwrap(),
        QuoteCheck::Unreadable { status: "scanned".into() }
    );
}

#[test]
fn indexing_writes_only_inside_the_project_database_and_leaves_files_alone() {
    let d = TempDir::new("research", "writes");
    let root = project(&d, "p");
    write(&root, "papers/a.pdf", &tiny_pdf("some words"));
    write(&root, "notes/n.md", b"note\n");
    std::fs::write(d.path().join("outside.txt"), b"untouched").unwrap();
    let skip = root.join(".npi-academic");
    let snapshot = || {
        let mut out = Vec::new();
        let mut stack = vec![d.path().to_path_buf()];
        while let Some(dir) = stack.pop() {
            for e in std::fs::read_dir(&dir).unwrap().flatten() {
                let p = e.path();
                if p.starts_with(&skip) {
                    continue;
                }
                if p.is_dir() {
                    stack.push(p);
                } else {
                    out.push((p.clone(), std::fs::read(&p).unwrap(), std::fs::metadata(&p).unwrap().modified().unwrap()));
                }
            }
        }
        out.sort();
        out
    };
    let before = snapshot();
    let lib = Library::open(&root).unwrap();
    lib.index_project(&root, IndexOptions::default()).unwrap();
    lib.check_quote(&root, "papers/a.pdf", Some(1), "some words").unwrap();
    lib.search_blocks("words", None, 5).unwrap();
    drop(lib);
    assert_eq!(snapshot(), before);
    assert!(!root.join(".npi-academic/cache").exists());
}

#[test]
fn rebuild_repairs_the_search_index_and_the_check_notices_damage() {
    let d = TempDir::new("research", "rebuild");
    let root = project(&d, "p");
    write(&root, "notes/n.md", "# T\n\n检索 words\n".as_bytes());
    let lib = Library::open(&root).unwrap();
    lib.index_project(&root, IndexOptions::default()).unwrap();
    assert!(lib.check_search().unwrap().is_empty());
    lib.conn.execute("DELETE FROM search_units WHERE kind = 'block'", []).unwrap();
    assert!(!lib.check_search().unwrap().is_empty());
    assert!(lib.search_blocks("检索", None, 5).unwrap().is_empty());
    lib.rebuild_search().unwrap();
    assert!(lib.check_search().unwrap().is_empty());
    assert_eq!(lib.search_blocks("检索", None, 5).unwrap().len(), 1);
    // Another version of the split rebuilds on its own.
    lib.set_meta_value("norm_version", "0").unwrap();
    lib.conn.execute("DELETE FROM search_units WHERE kind = 'block'", []).unwrap();
    assert_eq!(lib.search_blocks("检索", None, 5).unwrap().len(), 1);
}

#[test]
fn the_library_is_searchable_by_fts_once_the_project_is_indexed() {
    let d = TempDir::new("research", "refs");
    let root = project(&d, "p");
    let lib = Library::open(&root).unwrap();
    let zh = lib
        .add(&json!({"title": "文献检索综述", "type": "article-journal", "author": [{"family": "王", "given": "小明"}]}), "manual")
        .unwrap();
    let en = lib
        .add(&json!({"title": "Graph networks", "abstract": "message passing", "type": "article-journal"}), "manual")
        .unwrap();
    let all = lib.list().unwrap();
    assert_eq!(lib.search_library(&all, "文献", 10).unwrap(), None, "no index yet");
    lib.index_project(&root, IndexOptions::default()).unwrap();
    assert_eq!(lib.search_library(&all, "文献", 10).unwrap(), Some(vec![zh.id]));
    assert_eq!(lib.search_library(&all, "passing", 10).unwrap(), Some(vec![en.id]));
    assert_eq!(lib.search_library(&all, "小明", 10).unwrap(), Some(vec![zh.id]));
    lib.set_meta(en.id, &["survey".to_string()], "读书笔记").unwrap();
    lib.remove(zh.id).unwrap();
    let live = lib.list().unwrap();
    assert_eq!(lib.search_library(&live, "笔记", 10).unwrap(), Some(vec![en.id]));
    assert_eq!(lib.search_library(&live, "survey", 10).unwrap(), Some(vec![en.id]));
    assert_eq!(lib.search_library(&live, "文献", 10).unwrap(), Some(vec![]));
    assert!(lib.check_search().unwrap().is_empty());
}

#[test]
fn a_paper_is_linked_to_the_reference_that_attached_the_same_file() {
    let d = TempDir::new("research", "link");
    let root = project(&d, "p");
    let pdf = tiny_pdf("linked paper text");
    write(&root, "papers/a.pdf", &pdf);
    let lib = Library::open(&root).unwrap();
    let entry = lib.add(&json!({"title": "Linked", "type": "book"}), "manual").unwrap();
    lib.conn
        .execute(
            "INSERT INTO attachments (ref_id, path, hash, kind, added_at) VALUES (?1, 'papers/a.pdf', ?2, 'pdf', 0)",
            params![entry.id, hex(&Sha256::digest(&pdf))],
        )
        .unwrap();
    lib.index_project(&root, IndexOptions::default()).unwrap();
    let linked: Option<i64> =
        lib.conn.query_row("SELECT ref_id FROM papers WHERE path = 'papers/a.pdf'", [], |r| r.get(0)).unwrap();
    assert_eq!(linked, Some(entry.id));
}

#[test]
fn blocks_are_typed_and_long_paragraphs_are_cut() {
    let md = "# Intro\n\nPlain paragraph.\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\nTable 1: Results\n\n$$x = y$$\n";
    let mut out = Vec::new();
    blocks_of(md, Some(3), &mut out);
    let kinds: Vec<&str> = out.iter().map(|b| b.kind).collect();
    assert_eq!(kinds, ["heading", "paragraph", "table", "caption", "formula"]);
    assert!(out.iter().all(|b| b.page == Some(3)));
    let long = "word. ".repeat(900);
    let pieces = split_long(&long);
    assert!(pieces.len() > 2 && pieces.iter().all(|p| p.chars().count() <= MAX_BLOCK_CHARS));
}
