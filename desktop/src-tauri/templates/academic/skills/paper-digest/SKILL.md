---
name: paper-digest
description: Write a layered digest of a paper - section digests and a whole-paper digest - that keeps conditions, numbers and source pages.
---

# Paper digest

Write to `digests/<stem>.md`. Three layers, never merged:

- **Original** stays in `papers/` and is the only authority.
- **Section digests (L1)**: per section - problem and assumptions, method and where it applies, data (dataset, sample, metric, numbers with units), results including negative ones, limits. Each fact ends with `(p.N)`.
- **Paper digest (L2)**: research question, contribution, method, key results, limitations. Each fact points to the page it comes from, not only to an L1 line.

Finish with `Not covered:` listing sections, tables or appendices you did not digest. When a digest is out of date because the paper changed, mark it `stale` at the top instead of rewriting history.
