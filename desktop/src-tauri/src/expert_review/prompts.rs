//! What the experts and the arbiter of a review are told. One expert per roster tier, each with a
//! fixed angle; every review is one JSON object, so the arbiter and the page can read it.

use super::Kind;

/// `(tier, letter, angle name, what that angle looks at)`, in roster order.
pub const EXPERTS: [(&str, &str, &str, &str); 4] = [
    ("flagship", "A", "Domain expert", "where the work stands in its field: whether the question matters, what is genuinely new against what the field likely knows, and how far the contribution reaches"),
    ("high", "B", "Methodologist", "the design: whether the method can answer the question, the assumptions it rests on, threats to validity, and what a sound alternative design would be"),
    ("standard", "C", "Evidence and statistics", "claims against evidence: whether each claim is supported by what is shown, the numbers and the analysis behind them, and what is missing to believe it"),
    ("economy", "D", "Skeptical reviewer", "the strongest objections a hostile reviewer would raise first, the weakest link, and what would make the work fail"),
];

const fn kind_focus(kind: Kind) -> &'static str {
    match kind {
        Kind::Title => "a paper title. Judge whether it says what the work does and finds, its scope and accuracy, how it reads to someone searching for it, and its length. Give three better alternatives among your suggestions.",
        Kind::Idea => "a research idea or claim. Judge its novelty against what is likely known, the assumptions it needs, whether it can be tested or refuted, feasibility, and the risks. Name prior work worth checking as search terms, never as invented citations.",
        Kind::Novelty => "the claimed contributions (innovation points) of a work. For each claim, judge whether it is new, whether it is a real contribution or a rewording of known work, and whether the work shows it. Name prior work worth checking as search terms, never as invented citations.",
        Kind::Paper => "a manuscript or a part of one. Judge the contribution, the soundness of the method, whether the evidence supports the claims, clarity, coverage of related work, and reproducibility.",
    }
}

const REVIEW_JSON: &str = r#"{"verdict": "strong" | "promising" | "needs-work" | "weak", "score": 1-10, "scores": {"novelty": 1-10, "rigor": 1-10, "clarity": 1-10, "significance": 1-10, "feasibility": 1-10}, "summary": "two sentences", "strengths": ["..."], "weaknesses": [{"point": "...", "where": "the passage or section it concerns", "why": "..."}], "questions": ["..."], "suggestions": ["..."]}"#;

/// The system prompt of the expert of `tier`.
pub fn expert(kind: Kind, angle: &str, looks_at: &str) -> String {
    format!(
        "You are one of four independent reviewers, each with a different angle, in a simulated review of {focus}\n\
         Your angle: {angle}. You look at {looks_at}. Other reviewers cover the other angles; stay on yours.\n\
         Judge only what is in front of you and quote it when you criticise. Never invent references, data, results or facts about the field: when a point needs a source you do not have, say so. Be specific and fair, and say what works as plainly as what does not. Write in the language of the material.\n\
         Reply with one JSON object and nothing else, of this shape:\n{REVIEW_JSON}",
        focus = kind_focus(kind)
    )
}

const ARBITER_JSON: &str = r#"{"verdict": "strong" | "promising" | "needs-work" | "weak", "score": 1-10, "summary": "three sentences for the author", "consensus": ["points the reviewers agree on"], "disagreements": [{"point": "...", "views": "who says what", "resolution": "which view holds and why, or what would settle it"}], "must_fix": ["..."], "nice_to_have": ["..."]}"#;

/// The system prompt of the arbiter.
pub fn arbiter(kind: Kind) -> String {
    format!(
        "You chair a simulated review of {focus}\n\
         Four reviewers, A to D, each with a stated angle, reviewed the same material independently. You do not know which model wrote which review: weigh arguments, not tone or confidence. A point raised from several angles weighs more; a point only one reviewer makes stays when its argument holds against the material. Settle each disagreement, or say what would settle it. Add no criticism the reviews and the material do not support, and invent no references.\n\
         Write in the language of the material. Reply with one JSON object and nothing else, of this shape:\n{ARBITER_JSON}",
        focus = kind_focus(kind)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_tier_has_one_angle_and_prompts_ask_for_json_without_invented_sources() {
        let tiers: Vec<&str> = EXPERTS.iter().map(|e| e.0).collect();
        assert_eq!(tiers, ["flagship", "high", "standard", "economy"]);
        let p = expert(Kind::Title, EXPERTS[3].2, EXPERTS[3].3);
        assert!(p.contains("Skeptical reviewer") && p.contains("three better alternatives") && p.contains("\"verdict\""));
        assert!(p.contains("Never invent references"));
        let a = arbiter(Kind::Paper);
        assert!(a.contains("do not know which model") && a.contains("\"must_fix\"") && a.contains("invent no references"));
    }
}
