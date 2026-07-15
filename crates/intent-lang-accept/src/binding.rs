//! `.intent.bind.toml` — the semantic map between abstract requirements
//! and a concrete implementation (acceptance RFC 4.2, D3/D4).
//!
//! Declarative only: a binding is data, not a program. Reviewable,
//! diffable, committed to git. `{placeholder}`s are filled by codegen
//! with Z3 witness values (D8) or example values (D5).

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Binding {
    pub meta: Meta,
    /// Type name → constructor template, e.g.
    /// `Account` → `bank_demo.Account(owner={owner}, balance={balance})`
    #[serde(default)]
    pub types: BTreeMap<String, TypeBinding>,
    /// `"Account.balance"` → how to read that abstract state from a
    /// live object. Undeclared state = unobservable → manual item (D7).
    #[serde(default)]
    pub state: BTreeMap<String, StateBinding>,
    /// Intent name → operation binding.
    #[serde(default)]
    pub ops: BTreeMap<String, OpBinding>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Meta {
    pub intent_file: String,
    pub adapter: String,
    /// Python module(s) to import in generated tests.
    pub target: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TypeBinding {
    /// Constructor template with `{field}` placeholders.
    pub construct: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StateBinding {
    /// Read template with `{self}` placeholder, e.g. `{self}.balance`.
    pub read: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpBinding {
    /// Call template with `{param}` placeholders.
    pub call: String,
    /// D3 (rfc-modeling-integrity): the requirement layer declares
    /// "violation ⇒ reject + state unchanged" via `else reject`; the
    /// binding only answers *what rejection looks like here*.
    /// `"raises"` (default) or `"returns_error:<pattern>"`.
    #[serde(default)]
    pub reject_signal: Option<String>,
    /// Exception type for `reject_signal = "raises"`.
    #[serde(default)]
    pub error_type: Option<String>,
}

#[derive(Debug)]
pub enum BindingError {
    Io(std::io::Error),
    Parse(toml::de::Error),
    Invalid(String),
}

impl std::fmt::Display for BindingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BindingError::Io(e) => write!(f, "cannot read binding file: {e}"),
            BindingError::Parse(e) => write!(f, "invalid binding TOML: {e}"),
            BindingError::Invalid(m) => write!(f, "invalid binding: {m}"),
        }
    }
}

impl std::error::Error for BindingError {}

pub fn load_binding(path: &Path) -> Result<Binding, BindingError> {
    let text = std::fs::read_to_string(path).map_err(BindingError::Io)?;
    let binding: Binding = toml::from_str(&text).map_err(BindingError::Parse)?;
    validate(&binding)?;
    Ok(binding)
}

fn validate(b: &Binding) -> Result<(), BindingError> {
    if b.meta.adapter != "python-pytest" {
        return Err(BindingError::Invalid(format!(
            "unsupported adapter `{}` (only `python-pytest` in M-A1)",
            b.meta.adapter
        )));
    }
    for (name, op) in &b.ops {
        if let Some(sig) = &op.reject_signal {
            let ok = sig == "raises" || sig.starts_with("returns_error:");
            if !ok {
                return Err(BindingError::Invalid(format!(
                    "ops.{name}.reject_signal must be `raises` or `returns_error:<pattern>`, got `{sig}`"
                )));
            }
            if sig == "raises" && op.error_type.is_none() {
                return Err(BindingError::Invalid(format!(
                    "ops.{name}: reject_signal = \"raises\" requires error_type"
                )));
            }
        }
    }
    Ok(())
}

/// Fill `{placeholder}`s in a template. Missing keys are left intact so
/// the caller can detect them.
pub fn fill_template(template: &str, values: &BTreeMap<String, String>) -> String {
    let mut out = template.to_string();
    for (k, v) in values {
        out = out.replace(&format!("{{{k}}}"), v);
    }
    out
}
