//! `intent trace`: audit a `.intent` against the `facts.md` it was translated
//! from (RFC: workflow-hardening D6/D7/D8).
//!
//! The failure this exists to catch: an extraction produces 47 confirmed
//! facts, the translation drops six of them, and nothing notices — the
//! `.intent` verifies green because the missing requirements simply aren't
//! there to contradict anything.
//!
//! This is an **audit only**. It never writes or generates `.intent` content.
//! Generating a skeleton from facts would turn translation into fill-in-the-
//! blanks, and translation is exactly where judgement is required: contradictory
//! source facts must be carried over *as contradictions* so the verifier can
//! report them, not quietly reconciled.
//!
//! # Why the parse count is reported
//!
//! A lenient parser fails the same way the bug does: a fact it didn't
//! understand and a fact the translation dropped both look like "no such
//! fact". So the report always states how many facts it read, broken down by
//! kind. A human who knows the document has 47 entries can see "parsed 41" and
//! catch the parser rather than trusting a clean-looking audit.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Review state of a fact. `extract-facts` emits everything as `draft`; a human
/// promotes each entry at the confirmation gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FactStatus {
    /// Not yet reviewed.
    Draft,
    /// Accepted as requirement truth — must appear in the `.intent`.
    Confirmed,
    /// Judged a bug; deliberately not promoted to a requirement.
    Rejected,
    /// Reviewed and deliberately left undecided (e.g. behaviour that cannot be
    /// determined). Distinct from `draft`, which means "nobody looked yet".
    Deferred,
    /// Present but not one of the values above — reported, never guessed at.
    Unknown,
}

impl FactStatus {
    fn parse(raw: &str) -> Self {
        match raw.trim() {
            "draft" => Self::Draft,
            "confirmed" => Self::Confirmed,
            "rejected" => Self::Rejected,
            "deferred" => Self::Deferred,
            _ => Self::Unknown,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Confirmed => "confirmed",
            Self::Rejected => "rejected",
            Self::Deferred => "deferred",
            Self::Unknown => "unknown",
        }
    }
}

/// The three mutually exclusive zones of a facts document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FactKind {
    /// `F-<domain>-BEH-NNN` — neutral record of code behaviour.
    Beh,
    /// `F-<domain>-SUS-NNN` — anchored but suspicious.
    Sus,
    /// `F-<domain>-UNK-NNN` — no anchor, or an unknown sentinel.
    Unk,
}

impl FactKind {
    fn from_id(id: &str) -> Option<Self> {
        let mut parts = id.split('-');
        if parts.next()? != "F" {
            return None;
        }
        let _domain = parts.next()?;
        let kind = match parts.next()? {
            "BEH" => Self::Beh,
            "SUS" => Self::Sus,
            "UNK" => Self::Unk,
            _ => return None,
        };
        let seq = parts.next()?;
        if seq.is_empty() || !seq.chars().all(|c| c.is_ascii_digit()) || parts.next().is_some() {
            return None;
        }
        Some(kind)
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Beh => "BEH",
            Self::Sus => "SUS",
            Self::Unk => "UNK",
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Fact {
    pub id: String,
    pub kind: FactKind,
    pub status: FactStatus,
    /// Raw `status:` text, kept so an unrecognized value can be shown verbatim.
    pub status_raw: String,
    pub statement: String,
    /// 1-based line of the `fact_id:` field, for pointing a human at it.
    pub line: usize,
}

/// Lines that look like a field but whose shape the parser could not use.
/// Surfaced so a malformed entry is visible instead of silently absent.
#[derive(Debug, Clone, serde::Serialize)]
pub struct FactsParseWarning {
    pub line: usize,
    pub text: String,
}

#[derive(Debug, Default)]
pub struct FactsDoc {
    pub facts: Vec<Fact>,
    pub warnings: Vec<FactsParseWarning>,
}

/// Field names the rigid subset recognizes inside a fact entry.
const FACT_FIELDS: [&str; 7] = [
    "fact_id",
    "statement",
    "modality",
    "status",
    "source",
    "evidence",
    "relations",
];

/// Parse the machine-readable subset of a `facts.md`: entries opened by a
/// `fact_id:` field, followed by one field per line. Prose around them is
/// ignored; a `fact_id` whose shape is unrecognized becomes a warning rather
/// than a silently skipped line.
pub fn parse_facts(source: &str) -> FactsDoc {
    let mut doc = FactsDoc::default();
    let mut current: Option<Fact> = None;

    for (idx, raw_line) in source.lines().enumerate() {
        let line_no = idx + 1;
        let trimmed = raw_line.trim_start().trim_start_matches('-').trim_start();
        let Some((key, value)) = trimmed.split_once(':') else {
            continue;
        };
        let key = key.trim();
        if !FACT_FIELDS.contains(&key) {
            continue;
        }
        let value = value.trim();

        if key == "fact_id" {
            if let Some(f) = current.take() {
                doc.facts.push(f);
            }
            match FactKind::from_id(value) {
                Some(kind) => {
                    current = Some(Fact {
                        id: value.to_string(),
                        kind,
                        status: FactStatus::Unknown,
                        status_raw: String::new(),
                        statement: String::new(),
                        line: line_no,
                    });
                }
                None => {
                    doc.warnings.push(FactsParseWarning {
                        line: line_no,
                        text: format!(
                            "`fact_id: {value}` does not match F-<domain>-<BEH|SUS|UNK>-<NNN>"
                        ),
                    });
                    current = None;
                }
            }
            continue;
        }

        let Some(fact) = current.as_mut() else {
            continue; // field outside any entry (e.g. Meta block) — not ours
        };
        match key {
            "status" => {
                fact.status = FactStatus::parse(value);
                fact.status_raw = value.to_string();
            }
            "statement" => fact.statement = value.to_string(),
            _ => {}
        }
    }
    if let Some(f) = current.take() {
        doc.facts.push(f);
    }

    for f in &doc.facts {
        if f.status == FactStatus::Unknown {
            doc.warnings.push(FactsParseWarning {
                line: f.line,
                text: format!(
                    "{} has status `{}` — expected draft / confirmed / rejected / deferred",
                    f.id,
                    if f.status_raw.is_empty() {
                        "(missing)"
                    } else {
                        &f.status_raw
                    }
                ),
            });
        }
    }

    doc
}

/// Every `F-<domain>-<KIND>-<NNN>` token appearing in the `.intent` source
/// (clause comments), mapped to the lines it appears on.
pub fn referenced_fact_ids(source: &str) -> BTreeMap<String, Vec<usize>> {
    let mut out: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (idx, line) in source.lines().enumerate() {
        let bytes = line.as_bytes();
        let mut i = 0;
        while i + 1 < bytes.len() {
            let at_boundary = i == 0 || !is_id_char(bytes[i - 1] as char);
            if at_boundary && bytes[i] == b'F' && bytes[i + 1] == b'-' {
                let start = i;
                let mut end = i;
                while end < bytes.len() && is_id_char(bytes[end] as char) {
                    end += 1;
                }
                let candidate = &line[start..end];
                if FactKind::from_id(candidate).is_some() {
                    out.entry(candidate.to_string()).or_default().push(idx + 1);
                }
                i = end;
            } else {
                i += 1;
            }
        }
    }
    out
}

fn is_id_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-' || c == '_'
}

// ── Audit ────────────────────────────────────────────────────

#[derive(Debug, serde::Serialize)]
pub struct TraceReport {
    pub intent_file: String,
    pub facts_file: String,
    /// How many facts were read, per kind — the parser's self-witness.
    pub parsed: BTreeMap<String, usize>,
    pub parsed_total: usize,
    /// How many facts sit in each review state.
    pub statuses: BTreeMap<String, usize>,
    /// Confirmed facts with no `fact_id` reference in the `.intent`.
    pub confirmed_without_clause: Vec<Fact>,
    /// SUS/UNK facts still `draft` — the confirmation gate was skipped.
    pub undecided_suspicions: Vec<Fact>,
    /// `fact_id`s referenced by the `.intent` that the facts document
    /// does not contain.
    pub dangling_references: Vec<String>,
    /// Referenced facts whose status is not `confirmed`.
    pub references_not_confirmed: Vec<Fact>,
    pub parse_warnings: Vec<FactsParseWarning>,
}

impl TraceReport {
    pub fn ok(&self) -> bool {
        self.confirmed_without_clause.is_empty()
            && self.undecided_suspicions.is_empty()
            && self.dangling_references.is_empty()
            && self.references_not_confirmed.is_empty()
    }
}

pub fn audit(
    intent_file: &str,
    facts_file: &str,
    intent_source: &str,
    facts_source: &str,
) -> TraceReport {
    let doc = parse_facts(facts_source);
    let referenced = referenced_fact_ids(intent_source);

    let mut parsed: BTreeMap<String, usize> = BTreeMap::new();
    let mut statuses: BTreeMap<String, usize> = BTreeMap::new();
    for f in &doc.facts {
        *parsed.entry(f.kind.as_str().to_string()).or_insert(0) += 1;
        *statuses.entry(f.status.as_str().to_string()).or_insert(0) += 1;
    }

    let by_id: BTreeMap<&str, &Fact> = doc.facts.iter().map(|f| (f.id.as_str(), f)).collect();

    let confirmed_without_clause = doc
        .facts
        .iter()
        .filter(|f| f.status == FactStatus::Confirmed && !referenced.contains_key(&f.id))
        .cloned()
        .collect();

    // Only SUS/UNK block the gate: an un-reviewed BEH is visible as a
    // "confirmed but no clause" absence, whereas an un-reviewed suspicion or
    // unknown is precisely the thing a human must rule on before it can be
    // legitimately left out of the requirements.
    let undecided_suspicions = doc
        .facts
        .iter()
        .filter(|f| {
            matches!(f.kind, FactKind::Sus | FactKind::Unk) && f.status == FactStatus::Draft
        })
        .cloned()
        .collect();

    let mut dangling_references = Vec::new();
    let mut references_not_confirmed = Vec::new();
    for id in referenced.keys() {
        match by_id.get(id.as_str()) {
            None => dangling_references.push(id.clone()),
            Some(f) if f.status != FactStatus::Confirmed => {
                references_not_confirmed.push((*f).clone())
            }
            Some(_) => {}
        }
    }

    TraceReport {
        intent_file: intent_file.to_string(),
        facts_file: facts_file.to_string(),
        parsed_total: doc.facts.len(),
        parsed,
        statuses,
        confirmed_without_clause,
        undecided_suspicions,
        dangling_references,
        references_not_confirmed,
        parse_warnings: doc.warnings,
    }
}

/// Where a facts document lives by convention: alongside the `.intent`, named
/// after the same domain (`sweeper-register.intent` →
/// `sweeper-register.facts.md`). Making the default path the convention beats
/// documenting it — the convention that only exists in prose gets guessed at.
pub fn conventional_facts_path(intent_path: &Path) -> PathBuf {
    let stem = intent_path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    intent_path.with_file_name(format!("{stem}.facts.md"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FACTS: &str = r#"
# 订单退款流程 功能点事实

## Meta
- domain: 订单退款流程
- domain_abbrev: RF
- pinned: myproject@a1b2c3d

## 操作

#### 前置检查
- fact_id: F-RF-BEH-001
  statement: 退款金额大于订单实付金额时拒绝退款
  modality: must
  status: confirmed
  source: @a1b2c3d:src/refund/service.py#L47-L49

- fact_id: F-RF-BEH-002
  statement: 退款成功后订单状态从 Paid 变为 Refunded
  modality: must
  status: confirmed
  source: @a1b2c3d:src/refund/service.py#L83

- fact_id: F-RF-BEH-003
  statement: 这条被判定为 bug
  status: rejected
  source: @a1b2c3d:src/refund/service.py#L90

## 疑似问题区
- fact_id: F-RF-SUS-001
  statement: Kafka 消息 country 字段写入 siteCode
  status: draft
  source: @a1b2c3d:src/refund/kafka.py#L12

## 存疑区
- fact_id: F-RF-UNK-001
  statement: (unknown — needs human input) 并发重复退款行为未确定
  status: deferred
  source: —
"#;

    #[test]
    fn parses_the_rigid_subset_and_counts_by_kind() {
        let doc = parse_facts(FACTS);
        assert_eq!(doc.facts.len(), 5);
        assert_eq!(
            doc.facts.iter().filter(|f| f.kind == FactKind::Beh).count(),
            3
        );
        assert!(doc.warnings.is_empty(), "{:?}", doc.warnings);
    }

    #[test]
    fn meta_fields_are_not_mistaken_for_fact_fields() {
        // `- domain: ...` sits outside any entry and must not leak into one.
        let doc = parse_facts(FACTS);
        assert_eq!(doc.facts[0].id, "F-RF-BEH-001");
        assert_eq!(doc.facts[0].status, FactStatus::Confirmed);
    }

    #[test]
    fn malformed_fact_id_and_status_become_warnings_not_silence() {
        let src = "- fact_id: F-RF-BOGUS-1\n  status: confirmed\n\
                   - fact_id: F-RF-BEH-009\n  status: approved\n";
        let doc = parse_facts(src);
        assert_eq!(doc.facts.len(), 1);
        assert_eq!(doc.warnings.len(), 2);
    }

    #[test]
    fn finds_fact_ids_in_clause_comments() {
        let src = "intent Refund(o: Order) {\n  \
                   require r: o.amount > 0 else reject  // F-RF-BEH-001\n  \
                   ensure e: o.status' == Refunded // F-RF-BEH-002\n}";
        let refs = referenced_fact_ids(src);
        assert_eq!(refs.len(), 2);
        assert!(refs.contains_key("F-RF-BEH-001"));
    }

    #[test]
    fn does_not_match_identifiers_that_merely_start_with_f_dash() {
        let refs = referenced_fact_ids("// FF-RF-BEH-001 and F-RF-BEH-x and F-RF-BEH-1-2");
        assert!(refs.is_empty(), "{refs:?}");
    }

    #[test]
    fn confirmed_fact_without_clause_is_reported() {
        let intent = "// F-RF-BEH-001\nintent Refund() {}";
        let report = audit("r.intent", "r.facts.md", intent, FACTS);
        let missing: Vec<&str> = report
            .confirmed_without_clause
            .iter()
            .map(|f| f.id.as_str())
            .collect();
        assert_eq!(missing, vec!["F-RF-BEH-002"]);
        assert!(!report.ok());
    }

    #[test]
    fn undecided_suspicion_blocks_the_gate() {
        let intent = "// F-RF-BEH-001 F-RF-BEH-002";
        let report = audit("r.intent", "r.facts.md", intent, FACTS);
        let undecided: Vec<&str> = report
            .undecided_suspicions
            .iter()
            .map(|f| f.id.as_str())
            .collect();
        assert_eq!(undecided, vec!["F-RF-SUS-001"]);
    }

    #[test]
    fn deferred_does_not_block_the_gate() {
        // A fact a human reviewed and deliberately left undecided must not
        // wedge the pipeline shut forever.
        let intent = "// F-RF-BEH-001 F-RF-BEH-002";
        let report = audit("r.intent", "r.facts.md", intent, FACTS);
        assert!(report
            .undecided_suspicions
            .iter()
            .all(|f| f.id != "F-RF-UNK-001"));
    }

    #[test]
    fn dangling_and_unconfirmed_references_are_reported() {
        let intent = "// F-RF-BEH-001 F-RF-BEH-002 F-RF-BEH-003 F-RF-BEH-404";
        let report = audit("r.intent", "r.facts.md", intent, FACTS);
        assert_eq!(report.dangling_references, vec!["F-RF-BEH-404".to_string()]);
        assert_eq!(
            report
                .references_not_confirmed
                .iter()
                .map(|f| f.id.as_str())
                .collect::<Vec<_>>(),
            vec!["F-RF-BEH-003"]
        );
    }

    #[test]
    fn clean_translation_passes() {
        let src = "// F-RF-BEH-001\n// F-RF-BEH-002\n";
        let facts = FACTS.replace(
            "F-RF-SUS-001\n  statement: Kafka 消息 country 字段写入 siteCode\n  status: draft",
            "F-RF-SUS-001\n  statement: Kafka 消息 country 字段写入 siteCode\n  status: rejected",
        );
        let report = audit("r.intent", "r.facts.md", src, &facts);
        assert!(report.ok(), "{report:?}");
        assert_eq!(report.parsed_total, 5);
    }

    #[test]
    fn conventional_path_sits_next_to_the_intent() {
        let p = conventional_facts_path(Path::new("/tmp/dom/sweeper-register.intent"));
        assert_eq!(p, PathBuf::from("/tmp/dom/sweeper-register.facts.md"));
    }
}
