//! Multi-expert review, Academic only: four experts, one per model-roster tier and each with its
//! own angle (`prompts::EXPERTS`), review the same material in parallel; an arbiter who does not
//! see which model wrote which review weighs them into one result. For a paper title, an idea, the
//! claimed contributions or a manuscript. It is a simulation, said so wherever it shows.
//!
//! It spends a lot: the material goes to every expert. So it never starts on the model's word: a
//! tab shows a card with the models, the providers the material goes to and the estimated tokens,
//! and only the user's yes starts it (`confirm`); over MCP, where there is no card, the caller must
//! pass `confirmed` after asking the user. Experts run as isolated one-shot workers with no tools
//! (`copilot_worker::run_on`). The result is kept in the project's `research.db` (`kind = review`).

pub mod confirm;
pub mod prompts;

use crate::copilot::{frame, HostTools};
use crate::copilot_worker::Pick;
use serde_json::{json, Value};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Characters of material a review takes; longer work is reviewed a section at a time.
const MAX_MATERIAL_CHARS: usize = 120_000;
/// Characters of material the arbiter sees beside the reviews.
const ARBITER_MATERIAL_CHARS: usize = 12_000;
/// Estimated tokens of one review or verdict, and of a prompt.
const ANSWER_TOKENS: usize = 1_500;
const PROMPT_TOKENS: usize = 600;
/// Experts that must answer for a verdict.
const MIN_EXPERTS: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Title,
    Idea,
    Novelty,
    Paper,
}

impl Kind {
    fn parse(s: &str) -> Result<Self, String> {
        match s {
            "title" => Ok(Self::Title),
            "idea" => Ok(Self::Idea),
            "novelty" => Ok(Self::Novelty),
            "paper" => Ok(Self::Paper),
            _ => Err("`kind` is one of title, idea, novelty, paper".into()),
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Title => "Title",
            Self::Idea => "Idea",
            Self::Novelty => "Contributions",
            Self::Paper => "Manuscript",
        }
    }
}

/// What to review: the kind, a one-line subject, and the material itself.
#[derive(Debug, Clone)]
pub struct Request {
    pub kind: Kind,
    pub subject: String,
    pub material: String,
    pub arbiter: Option<String>,
}

/// Rough tokens of `text`: a CJK character is about one, other text about four characters each.
fn tokens(text: &str) -> usize {
    let cjk = text.chars().filter(|c| crate::librarian::cjk::is_cjk(*c)).count();
    cjk + (text.chars().count() - cjk) / 4
}

/// The request from a tool call: `text`, or a project-relative `file` (Markdown, LaTeX, text or
/// PDF), or both; a title or an idea may be its `subject` alone.
pub fn request(root: &Path, args: &Value) -> Result<Request, String> {
    let get = |k: &str| args.get(k).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty());
    let kind = Kind::parse(get("kind").unwrap_or(""))?;
    let subject = get("subject").ok_or("`subject` is empty: say in one line what is reviewed")?.to_string();
    let mut material = get("text").unwrap_or("").to_string();
    if let Some(rel) = get("file") {
        let path = crate::copilot::resolve_in(root, rel, &[])?;
        let text = if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf")) {
            match crate::pdf_text::extract(&path)? {
                crate::pdf_text::Extracted::Text(t) => t,
                crate::pdf_text::Extracted::Scanned => return Err(format!("{rel} has no text layer: run OCR on it first")),
            }
        } else {
            std::fs::read_to_string(&path).map_err(|e| format!("{rel}: {e}"))?
        };
        if !material.is_empty() {
            material.push_str("\n\n");
        }
        material.push_str(&text);
    }
    if material.is_empty() {
        if matches!(kind, Kind::Paper | Kind::Novelty) {
            return Err("give the manuscript as `text` or a project `file`".into());
        }
        material.clone_from(&subject);
    }
    let count = material.chars().count();
    if count > MAX_MATERIAL_CHARS {
        return Err(format!("the material is {count} characters, over {MAX_MATERIAL_CHARS}: review it a section at a time"));
    }
    Ok(Request { kind, subject, material, arbiter: get("arbiter").map(str::to_string) })
}

/// Who reviews and what it costs, before anything runs.
#[derive(Debug, Clone)]
pub struct Plan {
    /// `(tier, letter, angle, looks at, model)` of each expert with a model.
    pub experts: Vec<(&'static str, &'static str, &'static str, &'static str, Pick)>,
    pub arbiter: Pick,
    /// Tiers with no model, left out.
    pub missing: Vec<&'static str>,
    pub tokens: usize,
}

/// The plan for `req` from the roster (`model_config::roster_get`): an expert per set tier, the
/// arbiter from the request, the roster's arbiter, or the flagship.
pub fn plan(roster: &Value, req: &Request) -> Result<Plan, String> {
    let pick = |role: &str| roster.get(role).and_then(Value::as_str).and_then(Pick::parse);
    let mut experts = Vec::new();
    let mut missing = Vec::new();
    for (tier, letter, angle, looks_at) in prompts::EXPERTS {
        match pick(tier) {
            Some(p) => experts.push((tier, letter, angle, looks_at, p)),
            None => missing.push(tier),
        }
    }
    if experts.len() < MIN_EXPERTS {
        return Err("a review needs at least two tiers of the model roster: set them in Settings → Models → Model roster (or `modelRoles` in npi)".into());
    }
    let arbiter = req
        .arbiter
        .as_deref()
        .and_then(Pick::parse)
        .or_else(|| pick("arbiter"))
        .or_else(|| pick("flagship"))
        .unwrap_or_else(|| experts[0].4.clone()); // there is at least one expert
    let material = tokens(&req.material);
    let per_expert = PROMPT_TOKENS + material + ANSWER_TOKENS;
    let arbiter_cost = PROMPT_TOKENS + material.min(ARBITER_MATERIAL_CHARS) + experts.len() * ANSWER_TOKENS + ANSWER_TOKENS;
    Ok(Plan { tokens: experts.len() * per_expert + arbiter_cost, experts, arbiter, missing })
}

/// What the card says: models, the providers the material goes to, the estimate.
fn card(plan: &Plan, req: &Request) -> Value {
    let mut providers: Vec<String> = plan.experts.iter().map(|e| e.4.provider.clone()).chain([plan.arbiter.provider.clone()]).collect();
    providers.sort();
    providers.dedup();
    json!({
        "kind": req.kind.label(),
        "subject": req.subject,
        "experts": plan.experts.iter().map(|(tier, letter, angle, _, p)| json!({ "tier": tier, "letter": letter, "angle": angle, "model": p.label() })).collect::<Vec<_>>(),
        "arbiter": plan.arbiter.label(),
        "missing": plan.missing,
        "providers": providers,
        "tokens": plan.tokens,
        "characters": req.material.chars().count(),
    })
}

/// The first JSON object in `text` (models sometimes fence it or add a line), else `{ "text": … }`.
fn parse_json(text: &str) -> Value {
    let found = text.find('{').zip(text.rfind('}')).and_then(|(a, b)| serde_json::from_str::<Value>(&text[a..=b]).ok());
    found.filter(Value::is_object).unwrap_or_else(|| json!({ "text": text.trim() }))
}

fn task(req: &Request) -> String {
    format!("Subject: {}\n\nMaterial:\n<<<\n{}\n>>>", req.subject, req.material)
}

/// One review: the expert's answer, or why there is none.
type Review = (&'static str, &'static str, &'static str, String, Result<Value, String>);

/// Runs `plan` and keeps the result in the project. Returns the stored record and its id.
pub fn run(root: &Path, profile: Option<&str>, plan: &Plan, req: &Request) -> Result<(Value, i64), String> {
    let task = task(req);
    let reviews: Vec<Review> = std::thread::scope(|scope| {
        #[expect(clippy::needless_collect, reason = "Start every expert before joining any worker")]
        let handles: Vec<_> = plan
            .experts
            .iter()
            .map(|(tier, letter, angle, looks_at, pick)| {
                let prompt = prompts::expert(req.kind, angle, looks_at);
                let task = &task;
                (tier, letter, angle, pick.label(), scope.spawn(move || crate::copilot_worker::run_on(root, profile, Some(pick), &prompt, None, task)))
            })
            .collect();
        handles
            .into_iter()
            .map(|(tier, letter, angle, model, h)| {
                let answer = h.join().unwrap_or_else(|_| Err("the expert's run crashed".into()));
                (*tier, *letter, *angle, model, answer.map(|t| parse_json(&t)))
            })
            .collect()
    });
    let done: Vec<&Review> = reviews.iter().filter(|r| r.4.is_ok()).collect();
    if done.len() < MIN_EXPERTS {
        let why: Vec<String> = reviews.iter().filter_map(|r| r.4.as_ref().err().map(|e| format!("{}: {e}", r.3))).collect();
        return Err(format!("too few experts answered for a verdict ({})", why.join("; ")));
    }
    let anonymous: Vec<Value> = done.iter().map(|(_, letter, angle, _, answer)| json!({ "reviewer": letter, "angle": angle, "review": answer.as_ref().ok() })).collect();
    let excerpt: String = req.material.chars().take(ARBITER_MATERIAL_CHARS).collect();
    let arbiter_task = format!(
        "Subject: {}\n\nMaterial (the start of it, when long):\n<<<\n{excerpt}\n>>>\n\nReviews:\n{}",
        req.subject,
        serde_json::to_string_pretty(&anonymous).unwrap_or_default()
    );
    let verdict = crate::copilot_worker::run_on(root, profile, Some(&plan.arbiter), &prompts::arbiter(req.kind), None, &arbiter_task).map(|t| parse_json(&t))?;
    let experts: Vec<Value> = reviews
        .iter()
        .map(|(tier, letter, angle, model, answer)| match answer {
            Ok(r) => json!({ "tier": tier, "letter": letter, "angle": angle, "model": model, "review": r }),
            Err(e) => json!({ "tier": tier, "letter": letter, "angle": angle, "model": model, "error": e }),
        })
        .collect();
    let record = json!({ "simulated": true, "kind": req.kind.label(), "subject": req.subject, "arbiter": plan.arbiter.label(), "experts": experts, "verdict": verdict });
    let title = format!("{} review: {}", req.kind.label(), req.subject);
    let id = crate::librarian::Library::open(root)?.save_review(&title, &req.subject, &record["experts"], &record)?;
    Ok((record, id))
}

/// The result as the session's model reads it.
fn render(record: &Value, id: i64) -> String {
    let v = &record["verdict"];
    let list = |key: &str| v[key].as_array().map(|a| a.iter().filter_map(Value::as_str).map(|s| format!("- {s}")).collect::<Vec<_>>().join("\n")).unwrap_or_default();
    let mut out = format!(
        "Simulated multi-expert review #{id} (kept in the project's research.db). Present it as a simulation, not a real peer review.\n\nVerdict: {} ({}/10)\n{}\n",
        v["verdict"].as_str().unwrap_or("?"),
        v["score"].as_u64().map_or_else(|| "?".to_string(), |n| n.to_string()),
        v["summary"].as_str().or_else(|| v["text"].as_str()).unwrap_or("")
    );
    for (key, head) in [("consensus", "Agreed"), ("must_fix", "Must fix"), ("nice_to_have", "Worth doing")] {
        let items = list(key);
        if !items.is_empty() {
            write!(out, "\n{head}:\n{items}\n").unwrap();
        }
    }
    if let Some(ds) = v["disagreements"].as_array().filter(|d| !d.is_empty()) {
        out.push_str("\nDisagreements:\n");
        for d in ds {
            writeln!(out, "- {}: {} → {}", d["point"].as_str().unwrap_or(""), d["views"].as_str().unwrap_or(""), d["resolution"].as_str().unwrap_or("")).unwrap();
        }
    }
    out.push_str("\nReviewers:\n");
    for e in record["experts"].as_array().into_iter().flatten() {
        let line = e.get("error").and_then(Value::as_str).map_or_else(
            || format!("{} ({}/10): {}", e["review"]["verdict"].as_str().unwrap_or("?"), e["review"]["score"].as_u64().unwrap_or(0), e["review"]["summary"].as_str().unwrap_or("")),
            |err| format!("no review ({err})"),
        );
        writeln!(out, "- {} {} [{}]: {line}", e["letter"].as_str().unwrap_or(""), e["angle"].as_str().unwrap_or(""), e["model"].as_str().unwrap_or("")).unwrap();
    }
    out
}

pub fn definition(mcp: bool) -> Value {
    let mut properties = json!({
        "kind": { "type": "string", "enum": ["title", "idea", "novelty", "paper"] },
        "subject": { "type": "string" },
        "text": { "type": "string" },
        "file": { "type": "string" },
        "arbiter": { "type": "string" }
    });
    let ask = if mcp {
        properties["confirmed"] = json!({ "type": "boolean" });
        "It spends a great deal (the material goes to every expert and to several providers): first tell the user the models and the cost and get their explicit yes, then call with `confirmed: true`."
    } else {
        "It spends a great deal (the material goes to every expert and to several providers), so the user is shown the models and the estimated cost and must approve before it starts; if they decline, accept that."
    };
    json!({
        "name": "expert_review",
        "label": "Multi-expert review",
        "description": format!("A simulated review by four expert models (domain expert, methodologist, evidence and statistics, skeptic; one per model-roster tier) and an arbiter who weighs them. Use it when the user asks to have a paper title, an idea or claim, the claimed contributions, or a manuscript or section judged: \"what do you think of this idea\", \"review my paper\". `kind`: title, idea, novelty (the contributions) or paper. `subject`: one line on what is reviewed. The material: `text`, or a project-relative `file` (.md, .tex, .txt, .pdf), up to {MAX_MATERIAL_CHARS} characters; a title or idea may be the subject alone. `arbiter`: a model selector, only when the user names one. Review only the user's own work. {ask}"),
        "parameters": { "type": "object", "properties": properties, "required": ["kind", "subject"], "additionalProperties": false },
        "loadMode": "essential"
    })
}

type ShowConfirmation = Box<dyn Fn(&str, Value) -> bool + Send + Sync>;

/// How a review gets the user's go-ahead.
pub enum Consent {
    /// A tab: the card, shown by `show` (it emits `review://confirm` with the card's id and contents).
    Card(ShowConfirmation),
    /// MCP: the caller's `confirmed`, after asking the user itself.
    Declared,
}

/// Runs one `expert_review` call in `root`.
pub fn call(root: &Path, profile: Option<&str>, consent: &Consent, args: &Value) -> Result<String, String> {
    let req = request(root, args)?;
    let roster = crate::model_config::roster_get(profile)?;
    let plan = plan(&roster, &req)?;
    let summary = card(&plan, &req);
    let go = match consent {
        Consent::Declared => args.get("confirmed") == Some(&json!(true)),
        Consent::Card(show) => {
            let id = format!("review-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis()));
            confirm::ask(&id, || show(&id, summary.clone()), confirm::WAIT) // the card keeps its copy
        }
    };
    if !go {
        return Ok(format!(
            "The user did not approve the review, so nothing was sent. It would have used {} expert models and the arbiter {} (about {} tokens). Do not run it again unless they ask.",
            plan.experts.len(),
            plan.arbiter.label(),
            plan.tokens
        ));
    }
    let (record, id) = run(root, profile, &plan, &req)?;
    Ok(render(&record, id))
}

/// An Academic session's host tools with `expert_review` on top.
pub struct ReviewHost {
    inner: Arc<dyn HostTools>,
    root: PathBuf,
    profile: Option<String>,
    consent: Consent,
}

impl ReviewHost {
    pub fn new(inner: Arc<dyn HostTools>, root: PathBuf, profile: Option<&str>, consent: Consent) -> Self {
        Self { inner, root, profile: profile.map(str::to_string), consent }
    }
}

impl HostTools for ReviewHost {
    fn definitions(&self) -> Value {
        let mut all = self.inner.definitions().as_array().cloned().unwrap_or_default();
        all.push(definition(matches!(self.consent, Consent::Declared)));
        Value::Array(all)
    }

    fn call(&self, id: &str, tool: &str, args: &Value) -> Value {
        if tool != "expert_review" {
            return self.inner.call(id, tool, args);
        }
        frame(id, call(&self.root, self.profile.as_deref(), &self.consent, args))
    }
}

#[cfg(test)]
mod tests;
