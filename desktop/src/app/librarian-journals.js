// app/librarian-journals.js — which citation style a journal or a regulation asks for, for the
// Librarian's "Copy as…" menu. `window.OMP_JOURNALS = { presets, find }`.
//
// A preset is `{ label, style, exact, match, note? }`. `style` is the id of a style in the bundled CSL
// archive (librarian/format.rs; a Rust test checks every id), or null when no bundled style fits (the
// user adds the journal's .csl). `exact` is true when the style is the one the publisher or standard
// itself names, false when it is only the closest bundled one. A journal's own guide for authors always
// decides: the list says what the style is named after, not what a given editor will accept.
//
// Matching is by lower-case words (and Chinese phrases) in `match`, so "phys rev lett" and "PRL" find
// the APS style.
(function () {
  const p = (label, style, match, opts = {}) => ({ label, style, exact: opts.exact !== false, match, note: opts.note ?? "" });
  const GUIDE = "Check the journal's guide for authors.";
  const presets = [
    // ── Publisher and journal families ────────────────────────────────────
    p("Nature and the Nature journals", "nature", ["nature", "nature communications", "nat commun", "nature methods", "nature physics", "nature materials", "nature biotechnology", "nature medicine", "nature genetics", "nature chemistry", "nature energy", "nature photonics", "nature neuroscience", "nature reviews", "scientific reports", "sci rep", "communications biology", "communications chemistry", "communications physics"]),
    p("Cell Press (Cell, Neuron, Immunity, Current Biology…)", "cell", ["cell", "cell reports", "molecular cell", "neuron", "immunity", "cancer cell", "cell stem cell", "current biology", "developmental cell", "cell metabolism", "cell host microbe", "joule", "chem", "matter", "iscience", "structure"]),
    p("Trends in … (Cell Press reviews)", "trends", ["trends", "trends in cell biology", "trends in ecology", "trends in neurosciences"]),
    p("Current Opinion in …", "current-opinion", ["current opinion"]),
    p("The Lancet family", "the-lancet", ["lancet", "the lancet", "lancet oncology", "lancet neurology"]),
    p("BMJ", "bmj", ["bmj", "british medical journal"]),
    p("JAMA and the AMA", "american-medical-association", ["jama", "ama", "american medical association", "jama network"]),
    p("New England Journal of Medicine", "nlm-citation-sequence-superscript", ["nejm", "new england journal of medicine"], { exact: false, note: "Closest bundled style (numbered, superscript). " + GUIDE }),
    p("PLOS journals", "plos", ["plos", "plos one", "plos biology", "plos computational biology", "plos medicine", "plos genetics"]),
    p("BMC / BioMed Central journals", "biomed-central", ["bmc", "biomed central", "biomedcentral", "genome biology", "bmc bioinformatics"]),
    p("Frontiers in …", "frontiers", ["frontiers", "frontiers in neuroscience", "frontiers in immunology", "frontiers in psychology"]),
    p("MDPI journals (Sensors, Remote Sensing, Applied Sciences…)", "multidisciplinary-digital-publishing-institute", ["mdpi", "sensors", "remote sensing", "applied sciences", "energies", "sustainability", "materials", "molecules", "ijms", "electronics", "micromachines"]),
    p("American Chemical Society (JACS, ACS Nano, Nano Letters…)", "american-chemical-society", ["acs", "jacs", "j am chem soc", "acs nano", "nano letters", "acs catalysis", "analytical chemistry", "j phys chem", "american chemical society"]),
    p("Angewandte Chemie", "angewandte-chemie", ["angewandte", "angew chem"]),
    p("Royal Society of Chemistry", "royal-society-of-chemistry", ["rsc", "royal society of chemistry", "chem commun", "chemical science", "j mater chem"]),
    p("IEEE (Transactions, Letters, conferences)", "ieee", ["ieee", "ieee transactions", "ieee trans", "ieee access", "cvpr", "iccv", "icra", "globecom", "icc"]),
    p("ACM", "association-for-computing-machinery", ["acm", "communications of the acm", "cacm", "chi", "siggraph", "sigmod", "kdd", "tochi", "tog"]),
    p("Springer – Lecture Notes in Computer Science", "springer-lecture-notes-in-computer-science", ["lncs", "lecture notes in computer science", "springer lncs", "eccv", "miccai"]),
    p("Springer – numbered, in brackets", "springer-basic", ["springer", "springer brackets", "springer numeric"], { note: "Most Springer journals; some use author-date. " + GUIDE }),
    p("Springer – author-date", "springer-basic-author-date", ["springer author-date", "springer author date"]),
    p("Springer – Mathematics and Physics", "springer-mathphys", ["springer mathphys", "springer mathematics", "springer physics"]),
    p("Elsevier – numbered, with titles", "elsevier-with-titles", ["elsevier", "elsevier with titles", "elsevier numeric"], { note: "Elsevier journals use one of three styles; " + GUIDE }),
    p("Elsevier – Harvard (author-date)", "elsevier-harvard", ["elsevier harvard", "elsevier author-date"]),
    p("Elsevier – Vancouver", "elsevier-vancouver", ["elsevier vancouver"]),
    p("American Physical Society (Physical Review, PRL)", "american-physics-society", ["aps", "phys rev", "physical review", "prl", "phys rev lett", "reviews of modern physics", "rmp", "prx", "american physical society"]),
    p("American Institute of Physics (APL, JAP, JCP…)", "american-institute-of-physics", ["aip", "applied physics letters", "apl", "journal of applied physics", "jap", "journal of chemical physics", "jcp", "aip advances", "physics of fluids", "american institute of physics"]),
    p("Institute of Physics (IOP)", "institute-of-physics-numeric", ["iop", "iopscience", "j phys d", "new journal of physics", "nanotechnology", "classical and quantum gravity", "institute of physics"]),
    p("AIAA", "american-institute-of-aeronautics-and-astronautics", ["aiaa", "aiaa journal", "american institute of aeronautics"]),
    p("ASCE (civil engineers)", "american-society-of-civil-engineers", ["asce", "american society of civil engineers"]),
    p("ASME (mechanical engineers)", "american-society-of-mechanical-engineers", ["asme", "american society of mechanical engineers"]),
    p("American Meteorological Society", "american-meteorological-society", ["ams", "american meteorological society", "journal of climate", "monthly weather review"]),
    p("American Geophysical Union (JGR, GRL)", "american-geophysical-union", ["agu", "jgr", "grl", "geophysical research letters", "journal of geophysical research", "american geophysical union"]),
    p("SPIE journals", "spie", ["spie", "optical engineering", "journal of biomedical optics"]),
    p("IET (Institution of Engineering and Technology)", "the-institution-of-engineering-and-technology", ["iet", "institution of engineering and technology"]),
    p("Copernicus / EGU (ACP, HESS, ESSD…)", "copernicus", ["copernicus", "egu", "acp", "atmospheric chemistry and physics", "hess", "essd", "the cryosphere"]),
    p("Annual Reviews (numbered)", "annual-reviews", ["annual reviews", "annual review"]),
    p("Annual Reviews (author-date)", "annual-reviews-author-date", ["annual reviews author-date"]),
    p("Karger journals", "karger", ["karger"]),
    p("Thieme (German)", "thieme", ["thieme"]),
    p("Pensoft journals", "pensoft", ["pensoft", "zookeys", "phytokeys"]),
    p("SAGE – Vancouver", "sage-vancouver", ["sage", "sage vancouver"]),
    p("Taylor & Francis – Chicago author-date", "taylor-and-francis-chicago-author-date", ["taylor francis", "taylor and francis", "t&f", "tandf"]),
    p("Taylor & Francis – NLM (numbered)", "taylor-and-francis-national-library-of-medicine", ["taylor francis nlm", "tandf nlm"]),
    p("Mary Ann Liebert – Vancouver", "mary-ann-liebert-vancouver", ["liebert", "mary ann liebert"]),
    p("Future Medicine", "future-medicine", ["future medicine"]),
    p("Future Science Group", "future-science", ["future science"]),
    p("American Physiological Society", "american-physiological-society", ["american physiological society", "j appl physiol", "am j physiol"]),
    p("American Society for Microbiology", "american-society-for-microbiology", ["asm", "american society for microbiology", "mbio", "j bacteriol"]),
    p("American Political Science Association", "american-political-science-association", ["apsa", "american political science review", "apsr"]),
    p("American Sociological Association", "american-sociological-association", ["asa", "american sociological review"]),
    p("American Anthropological Association", "american-anthropological-association", ["aaa", "american anthropologist"]),
    p("APA (American Psychological Association)", "apa", ["apa", "apa 7", "psychology", "american psychological association"]),
    p("Deutsche Gesellschaft für Psychologie", "deutsche-gesellschaft-für-psychologie", ["dgps", "deutsche gesellschaft fur psychologie"]),
    p("Bristol University Press", "bristol-university-press", ["bristol university press", "policy press"]),
    // ── Not bundled: the user adds the journal's own .csl ─────────────────
    p("Science, Science Advances, Science Translational Medicine", null, ["science", "science advances", "sci adv", "science translational medicine"], { exact: false, note: "No bundled style. Download the journal's .csl (citationstyles.org) and add it with “Add a .csl style…”." }),
    p("PNAS", null, ["pnas", "proceedings of the national academy of sciences"], { exact: false, note: "No bundled style. Download the .csl (citationstyles.org) and add it with “Add a .csl style…”." }),
    p("Optica (OSA) journals", null, ["optica", "osa", "optics express", "optics letters", "applied optics"], { exact: false, note: "No bundled style. Download the .csl (citationstyles.org) and add it with “Add a .csl style…”." }),
    p("Wiley journals", null, ["wiley", "advanced materials", "adv mater", "chemistry a european journal"], { exact: false, note: "Wiley journals differ; download the journal's .csl (citationstyles.org) and add it with “Add a .csl style…”." }),
    p("Oxford University Press journals", null, ["oup", "oxford university press", "bioinformatics", "nucleic acids research", "nar"], { exact: false, note: "OUP journals differ; download the journal's .csl (citationstyles.org) and add it with “Add a .csl style…”." }),
    // ── Standards and regulations ─────────────────────────────────────────
    p("GB/T 7714-2015 顺序编码制 (numbered)", "gb-7714-2015-numeric", ["gb/t 7714", "gbt7714", "gb7714", "国标", "顺序编码制", "中国科学", "物理学报", "计算机学报", "自动化学报", "电子学报", "光学学报", "软件学报", "学位论文", "中文期刊"]),
    p("GB/T 7714-2015 著者-出版年制 (author-date)", "gb-7714-2015-author-date", ["gb/t 7714 author-date", "著者-出版年制", "著者出版年制", "国标 著者"]),
    p("GB/T 7714-2015 脚注 (notes)", "gb-7714-2015-note", ["gb/t 7714 note", "脚注", "国标 脚注"]),
    p("GB/T 7714-2005 顺序编码制 (older edition)", "gb-7714-2005-numeric", ["gb/t 7714-2005", "gb7714 2005", "2005版", "旧国标"]),
    p("ISO 690 (numbered)", "iso-690-numeric", ["iso 690", "iso690", "iso 690 numeric"]),
    p("ISO 690 (author-date)", "iso-690-author-date", ["iso 690 author-date", "iso690 author date"]),
    p("Vancouver / ICMJE (numbered)", "vancouver", ["vancouver", "icmje", "nlm", "pubmed", "medical journals"]),
    p("Vancouver (superscript numbers)", "vancouver-superscript", ["vancouver superscript"]),
    p("NLM citation sequence", "nlm-citation-sequence", ["nlm citation sequence", "national library of medicine"]),
    p("CSE (name-year)", "cse-name-year", ["cse", "council of science editors", "cse name year"]),
    p("CSE (citation-sequence)", "cse-citation-sequence-brackets-8th-edition", ["cse citation sequence", "cse numeric"]),
    p("Chicago (author-date)", "chicago-author-date", ["chicago", "chicago author date", "chicago 17"]),
    p("Chicago (notes and bibliography)", "chicago-notes", ["chicago notes", "chicago footnotes", "history"]),
    p("Turabian (author-date)", "turabian-author-date", ["turabian"]),
    p("MLA", "modern-language-association", ["mla", "mla 9", "literature", "humanities"]),
    p("MHRA (notes)", "modern-humanities-research-association-notes", ["mhra"]),
    p("Harvard (Cite Them Right)", "harvard-cite-them-right", ["harvard", "cite them right"]),
    p("ABNT (Brazil)", "associacao-brasileira-de-normas-tecnicas", ["abnt", "brazil", "nbr 6023"]),
    p("GOST R 7.0.5-2008 (Russia)", "gost-r-705-2008-numeric", ["gost", "russia"]),
    p("SIST 02 (Japan, JST)", "sist02", ["sist02", "sist 02", "jst", "japan"]),
    p("Alphanumeric labels [Smi20]", "alphanumeric", ["alphanumeric", "label", "alpha"]),
  ];

  const fold = (s) => String(s ?? "").toLowerCase().replace(/[._\-]+/g, " ").replace(/\s+/g, " ").trim();

  /** Presets for what the user typed (a journal, an abbreviation, "gb/t 7714"…), best first, at most `limit`. */
  function find(query, limit = 10) {
    const q = fold(query);
    if (q.length < 2) return [];
    const scored = [];
    for (const preset of presets) {
      let best = 0;
      for (const m of preset.match) {
        const f = fold(m);
        if (f === q) best = Math.max(best, 100);
        else if (f.startsWith(q) || q.startsWith(f)) best = Math.max(best, f.length >= 3 || q.length >= 3 ? 80 : 40);
        else if (f.includes(q) || (f.length >= 4 && q.includes(f))) best = Math.max(best, 60);
      }
      if (fold(preset.label).includes(q)) best = Math.max(best, 70);
      if (best > 0) scored.push([best, preset]);
    }
    scored.sort((a, b) => b[0] - a[0]);
    return scored.slice(0, limit).map(([, preset]) => preset);
  }

  window.OMP_JOURNALS = { presets, find };
})();
