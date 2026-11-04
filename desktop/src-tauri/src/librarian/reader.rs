//! What the reader window shows of a project's Research Core index: its papers, one paper's
//! sections, one page of blocks, and where an anchor or a search hit lies. Read only; every
//! query is on this project's own database, and the webview names rows by id, never by path.

use super::store::{sql, Library};
use rusqlite::{params, OptionalExtension};
use serde::Serialize;

/// Most blocks one page (or one text file) returns.
const MAX_BLOCKS: usize = 400;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaperRow {
    pub id: i64,
    pub path: String,
    pub kind: String,
    pub title: String,
    pub authors: String,
    pub year: Option<i64>,
    pub doi: Option<String>,
    /// `ok`, `partial`, `scanned` or `failed` (as the indexer recorded it).
    pub parse_status: String,
    pub parse_note: String,
    pub page_count: i64,
    /// Blocks whose anchors went stale because the file changed after indexing.
    pub stale_anchors: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SectionRow {
    pub id: i64,
    pub heading: String,
    pub level: i64,
    /// The first page with a block of this section.
    pub page: Option<i64>,
    pub blocks: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockRow {
    pub id: i64,
    pub kind: String,
    pub text: String,
    pub section_id: Option<i64>,
    pub anchor_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageView {
    pub paper_id: i64,
    pub page: Option<i64>,
    pub page_count: i64,
    pub blocks: Vec<BlockRow>,
    /// More blocks than [`MAX_BLOCKS`]: the rest are not shown.
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Target {
    pub paper_id: i64,
    pub page: Option<i64>,
    pub block_id: Option<i64>,
    pub stale: bool,
}

impl Library {
    /// Every indexed file of the project, papers first, by path.
    pub fn papers_list(&self) -> Result<Vec<PaperRow>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT p.id, p.path, p.kind, p.title, p.authors, p.year, p.doi, p.parse_status, p.parse_note, p.page_count,
                        (SELECT count(*) FROM anchors a WHERE a.paper_id = p.id AND a.stale = 1)
                 FROM papers p
                 ORDER BY (substr(p.path, 1, 7) = 'papers/') DESC, p.path",
            )
            .map_err(sql)?;
        let rows = stmt
            .query_map([], |r| {
                Ok(PaperRow {
                    id: r.get(0)?,
                    path: r.get(1)?,
                    kind: r.get(2)?,
                    title: r.get(3)?,
                    authors: r.get(4)?,
                    year: r.get(5)?,
                    doi: r.get(6)?,
                    parse_status: r.get(7)?,
                    parse_note: r.get(8)?,
                    page_count: r.get(9)?,
                    stale_anchors: r.get(10)?,
                })
            })
            .map_err(sql)?;
        rows.collect::<Result<_, _>>().map_err(sql)
    }

    /// The sections of one paper in reading order.
    pub fn paper_sections(&self, paper_id: i64) -> Result<Vec<SectionRow>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT s.id, s.heading, s.level,
                        (SELECT min(b.page) FROM blocks b WHERE b.section_id = s.id),
                        (SELECT count(*) FROM blocks b WHERE b.section_id = s.id)
                 FROM sections s WHERE s.paper_id = ?1 ORDER BY s.ordinal",
            )
            .map_err(sql)?;
        let rows = stmt
            .query_map(params![paper_id], |r| {
                Ok(SectionRow { id: r.get(0)?, heading: r.get(1)?, level: r.get(2)?, page: r.get(3)?, blocks: r.get(4)? })
            })
            .map_err(sql)?;
        rows.collect::<Result<_, _>>().map_err(sql)
    }

    /// The blocks of one page of a paper (all of a text file's blocks when `page` is `None`).
    pub fn paper_page(&self, paper_id: i64, page: Option<i64>) -> Result<PageView, String> {
        let page_count: i64 = self
            .conn
            .query_row("SELECT page_count FROM papers WHERE id = ?1", params![paper_id], |r| r.get(0))
            .optional()
            .map_err(sql)?
            .ok_or_else(|| "no such paper".to_string())?;
        let mut stmt = self
            .conn
            .prepare(
                "SELECT b.id, b.kind, b.text, b.section_id,
                        (SELECT a.id FROM anchors a WHERE a.block_id = b.id AND a.stale = 0 ORDER BY a.id LIMIT 1)
                 FROM blocks b
                 WHERE b.paper_id = ?1 AND (?2 IS NULL OR b.page = ?2)
                 ORDER BY b.ordinal LIMIT ?3",
            )
            .map_err(sql)?;
        let limit = i64::try_from(MAX_BLOCKS + 1).unwrap_or(i64::MAX);
        let mut blocks: Vec<BlockRow> = stmt
            .query_map(params![paper_id, page, limit], |r| {
                Ok(BlockRow { id: r.get(0)?, kind: r.get(1)?, text: r.get(2)?, section_id: r.get(3)?, anchor_id: r.get(4)? })
            })
            .map_err(sql)?
            .collect::<Result<_, _>>()
            .map_err(sql)?;
        let truncated = blocks.len() > MAX_BLOCKS;
        blocks.truncate(MAX_BLOCKS);
        Ok(PageView { paper_id, page, page_count, blocks, truncated })
    }

    /// Where an anchor (a citation's evidence) lies: its paper, page and block.
    pub fn anchor_target(&self, anchor_id: i64) -> Result<Option<Target>, String> {
        self.conn
            .query_row("SELECT paper_id, page, block_id, stale FROM anchors WHERE id = ?1", params![anchor_id], |r| {
                Ok(Target { paper_id: r.get(0)?, page: r.get(1)?, block_id: r.get(2)?, stale: r.get::<_, i64>(3)? != 0 })
            })
            .optional()
            .map_err(sql)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdf_text::pages_pdf;
    use crate::test_support::TempDir;

    fn indexed(d: &TempDir) -> (std::path::PathBuf, Library) {
        let root = d.path().join("p");
        for dir in ["papers", "notes", "digests", "drafts"] {
            std::fs::create_dir_all(root.join(dir)).unwrap();
        }
        std::fs::write(root.join("papers/a.pdf"), pages_pdf(&["First page about attention.", "Second page about 文献检索."])).unwrap();
        std::fs::write(root.join("notes/n.md"), b"# Idea\n\nA note about attention.\n").unwrap();
        let lib = Library::open(&root).unwrap();
        lib.index_project(&root, super::super::research::IndexOptions::default()).unwrap();
        (root, lib)
    }

    #[test]
    fn papers_pages_and_anchors_are_listed_from_the_index() {
        let d = TempDir::new("reader", "basic");
        let (_root, lib) = indexed(&d);
        let papers = lib.papers_list().unwrap();
        assert_eq!(papers[0].path, "papers/a.pdf");
        assert!(papers.iter().any(|p| p.path == "notes/n.md"));
        let pdf = &papers[0];
        assert_eq!(pdf.page_count, 2);
        let second = lib.paper_page(pdf.id, Some(2)).unwrap();
        assert!(second.blocks.iter().any(|b| b.text.contains("文献检索")));
        assert!(!second.blocks.iter().any(|b| b.text.contains("First page")));
        let first = lib.paper_page(pdf.id, Some(1)).unwrap();
        let anchored = first.blocks.iter().find_map(|b| b.anchor_id).expect("a block has an anchor");
        let target = lib.anchor_target(anchored).unwrap().unwrap();
        assert_eq!((target.paper_id, target.page, target.stale), (pdf.id, Some(1), false));
        assert!(lib.anchor_target(9999).unwrap().is_none());
        assert!(lib.paper_page(9999, Some(1)).is_err());
    }

    #[test]
    fn a_text_file_is_one_view_without_pages() {
        let d = TempDir::new("reader", "text");
        let (_root, lib) = indexed(&d);
        let note = lib.papers_list().unwrap().into_iter().find(|p| p.path == "notes/n.md").unwrap();
        let view = lib.paper_page(note.id, None).unwrap();
        assert!(view.blocks.iter().any(|b| b.text.contains("note about attention")));
        assert!(!view.truncated);
    }
}
