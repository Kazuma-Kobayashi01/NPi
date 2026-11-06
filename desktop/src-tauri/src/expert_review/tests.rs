use super::*;
use crate::test_support::TempDir;

fn roster(pairs: &[(&str, &str)]) -> Value {
    let mut m = serde_json::Map::new();
    for role in ["flagship", "high", "standard", "economy", "arbiter"] {
        m.insert(role.into(), Value::Null);
    }
    for (k, v) in pairs {
        m.insert((*k).into(), json!(v));
    }
    Value::Object(m)
}

fn req(kind: Kind, material: &str) -> Request {
    Request { kind, subject: "S".into(), material: material.into(), arbiter: None }
}

#[test]
fn a_request_takes_text_or_a_project_file_and_refuses_what_it_cannot_review() {
    let d = TempDir::new("expert-review", "request");
    std::fs::create_dir_all(d.path().join("write")).unwrap();
    std::fs::write(d.path().join("write/intro.md"), "We propose X.").unwrap();
    let root = d.path().canonicalize().unwrap();
    let r = request(&root, &json!({ "kind": "paper", "subject": "Intro", "file": "write/intro.md" })).unwrap();
    assert_eq!(r.material, "We propose X.");
    let title = request(&root, &json!({ "kind": "title", "subject": "Deep EIT for lungs" })).unwrap();
    assert_eq!(title.material, "Deep EIT for lungs", "a title is its own material");
    assert!(request(&root, &json!({ "kind": "paper", "subject": "x" })).unwrap_err().contains("manuscript"));
    assert!(request(&root, &json!({ "kind": "poem", "subject": "x" })).unwrap_err().contains("kind"));
    assert!(request(&root, &json!({ "kind": "idea", "subject": " " })).unwrap_err().contains("subject"));
    assert!(request(&root, &json!({ "kind": "paper", "subject": "x", "file": "../etc/passwd" })).is_err());
    let long = "x".repeat(MAX_MATERIAL_CHARS + 1);
    assert!(request(&root, &json!({ "kind": "paper", "subject": "x", "text": long })).unwrap_err().contains("section"));
}

#[test]
fn the_plan_takes_one_expert_per_set_tier_and_an_arbiter_by_precedence() {
    let r = roster(&[("flagship", "anthropic/opus:high"), ("high", "openai-codex/gpt"), ("economy", "google/flash")]);
    let p = plan(&r, &req(Kind::Idea, "idea")).unwrap();
    assert_eq!(p.experts.iter().map(|e| e.0).collect::<Vec<_>>(), ["flagship", "high", "economy"]);
    assert_eq!(p.missing, ["standard"]);
    assert_eq!(p.arbiter.label(), "anthropic/opus", "no arbiter set: the flagship");
    assert_eq!(p.experts[0].4.thinking.as_deref(), Some("high"));
    let with_arbiter = roster(&[("flagship", "a/x"), ("high", "b/y"), ("arbiter", "c/z")]);
    assert_eq!(plan(&with_arbiter, &req(Kind::Idea, "i")).unwrap().arbiter.label(), "c/z");
    let mut asked = req(Kind::Idea, "i");
    asked.arbiter = Some("d/w:low".into());
    assert_eq!(plan(&with_arbiter, &asked).unwrap().arbiter.label(), "d/w", "the request's arbiter wins");
    assert!(plan(&roster(&[("flagship", "a/x")]), &req(Kind::Idea, "i")).unwrap_err().contains("two tiers"));
}

#[test]
fn the_estimate_grows_with_the_material_and_counts_chinese_by_character() {
    let r = roster(&[("flagship", "a/x"), ("high", "b/y"), ("standard", "c/z"), ("economy", "d/w")]);
    let small = plan(&r, &req(Kind::Paper, "short")).unwrap().tokens;
    let english = plan(&r, &req(Kind::Paper, &"word ".repeat(8000))).unwrap().tokens;
    let chinese = plan(&r, &req(Kind::Paper, &"论文".repeat(10000))).unwrap().tokens;
    assert!(english > small + 4 * 9000, "every expert reads the material");
    assert!(chinese > english, "20 000 CJK characters are about 20 000 tokens");
    assert_eq!(tokens("论文ab cd"), 2 + 5 / 4);
}

#[test]
fn a_card_lists_models_and_providers_once() {
    let r = roster(&[("flagship", "anthropic/opus"), ("high", "anthropic/sonnet"), ("economy", "google/flash")]);
    let rq = req(Kind::Title, "T");
    let c = card(&plan(&r, &rq).unwrap(), &rq);
    assert_eq!(c["providers"], json!(["anthropic", "google"]));
    assert_eq!(c["experts"][2]["angle"], "Skeptical reviewer");
    assert_eq!(c["missing"], json!(["standard"]));
}

#[test]
fn answers_are_read_as_json_even_when_fenced_and_kept_as_text_otherwise() {
    assert_eq!(parse_json("```json\n{\"verdict\": \"weak\", \"score\": 3}\n```")["score"], 3);
    assert_eq!(parse_json("Here: {\"a\": {\"b\": 1}} done")["a"]["b"], 1);
    assert_eq!(parse_json("no json at all")["text"], "no json at all");
    assert_eq!(parse_json("[1, 2]")["text"], "[1, 2]");
}

#[test]
fn the_rendered_result_says_simulated_and_lists_every_reviewer() {
    let record = json!({
        "experts": [
            { "letter": "A", "angle": "Domain expert", "model": "a/x", "review": { "verdict": "promising", "score": 7, "summary": "Good question." } },
            { "letter": "D", "angle": "Skeptical reviewer", "model": "d/w", "error": "429" }
        ],
        "verdict": { "verdict": "needs-work", "score": 6, "summary": "Sharpen the claim.", "must_fix": ["State the baseline."], "disagreements": [{ "point": "novelty", "views": "A yes, C no", "resolution": "check prior work" }] }
    });
    let text = render(&record, 9);
    assert!(text.contains("Simulated multi-expert review #9") && text.contains("needs-work (6/10)"));
    assert!(text.contains("Must fix:\n- State the baseline.") && text.contains("novelty: A yes, C no → check prior work"));
    assert!(text.contains("A Domain expert [a/x]: promising (7/10)") && text.contains("D Skeptical reviewer [d/w]: no review (429)"));
}

#[test]
fn the_tool_says_how_consent_is_given() {
    assert!(definition(false)["description"].as_str().unwrap().contains("must approve"));
    let mcp = definition(true);
    assert!(mcp["description"].as_str().unwrap().contains("confirmed: true"));
    assert_eq!(mcp["parameters"]["properties"]["confirmed"]["type"], "boolean");
}

#[test]
fn selectors_split_a_thinking_level_but_keep_colons_of_a_model_id() {
    let p = Pick::parse("openrouter/qwen/qwen3-coder:exacto").unwrap();
    assert_eq!((p.provider.as_str(), p.id.as_str(), p.thinking), ("openrouter", "qwen/qwen3-coder:exacto", None));
    let q = Pick::parse("anthropic/opus:high").unwrap();
    assert_eq!((q.id.as_str(), q.thinking.as_deref()), ("opus", Some("high")));
    assert!(Pick::parse("noslash").is_none() && Pick::parse("/x").is_none());
}
