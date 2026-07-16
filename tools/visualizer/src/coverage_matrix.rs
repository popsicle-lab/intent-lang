/// Coverage matrix builder
///
/// Builds visualization data for coverage declarations showing:
/// - Multi-dimensional test coverage matrices
/// - Covered vs. uncovered combinations
/// - Statistics about coverage completeness

use intent_lang_syntax::ast::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct CoverageMatrix {
    pub name: String,
    pub dimensions: Vec<Dimension>,
    pub stats: Option<CoverageStats>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Dimension {
    pub name: String,
    pub values: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CoverageStats {
    pub total_combinations: usize,
    pub covered_combinations: usize,
    pub missing_combinations: usize,
}

pub fn build_coverage_matrix(program: &Program) -> CoverageMatrix {
    // Find first coverage declaration
    for decl in &program.declarations {
        if let Declaration::Coverage(coverage) = &decl.node {
            return build_matrix_from_coverage(coverage);
        }
    }

    // Return empty matrix if no coverage found
    CoverageMatrix {
        name: "No coverage declarations found".to_string(),
        dimensions: vec![],
        stats: None,
    }
}

/// All `coverage` declarations in the program (a `.intent` file commonly
/// declares more than one scenario-dimension group — [`build_coverage_matrix`]
/// only sees the first, which silently drops the rest from any view built
/// on top of it).
pub fn build_all_coverage_matrices(program: &Program) -> Vec<CoverageMatrix> {
    program
        .declarations
        .iter()
        .filter_map(|decl| match &decl.node {
            Declaration::Coverage(coverage) => Some(build_matrix_from_coverage(coverage)),
            _ => None,
        })
        .collect()
}

fn build_matrix_from_coverage(coverage: &CoverageDecl) -> CoverageMatrix {
    let mut dimensions = Vec::new();

    for dim in &coverage.dimensions {
        let values: Vec<String> = dim.values
            .iter()
            .map(|v| format_expr_simple(&v.node))
            .collect();

        dimensions.push(Dimension {
            name: dim.name.clone(),
            values,
        });
    }

    // Calculate total combinations
    let total_combinations: usize = dimensions
        .iter()
        .map(|d| d.values.len())
        .product();

    let stats = Some(CoverageStats {
        total_combinations,
        covered_combinations: 0, // Would need semantic analysis
        missing_combinations: total_combinations,
    });

    CoverageMatrix {
        name: coverage.name.clone(),
        dimensions,
        stats,
    }
}

fn format_expr_simple(expr: &Expr) -> String {
    match expr {
        Expr::Ident(name) => name.clone(),
        Expr::IntLit(n) => n.to_string(),
        Expr::BoolLit(b) => b.to_string(),
        Expr::StringLit(s) => s.clone(),
        _ => "?".to_string(),
    }
}

impl crate::GraphData for CoverageMatrix {
    fn to_json(&self) -> anyhow::Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }
}

/// All value combinations of `dims`, as `(dimension_index, value)` pairs —
/// dimension_index is the index *within `dims`*, used later to build a
/// stable `data-combo` key for the client-side switcher.
fn cartesian(dims: &[Dimension]) -> Vec<Vec<(usize, String)>> {
    let mut combos: Vec<Vec<(usize, String)>> = vec![Vec::new()];
    for (di, dim) in dims.iter().enumerate() {
        let mut next = Vec::with_capacity(combos.len() * dim.values.len().max(1));
        for combo in &combos {
            for v in &dim.values {
                let mut c = combo.clone();
                c.push((di, v.clone()));
                next.push(c);
            }
        }
        combos = next;
    }
    combos
}

/// Render a coverage declaration as an honest "did we forget a dimension
/// combination?" grid — no covered/missing counts, since static reference
/// analysis can't tell a real gap from an implication-style rule that
/// legitimately never spells out every combination by name (see the
/// project README's coverage caveat). `idx` must be unique across all
/// coverage blocks rendered on the same page (used to scope the JS
/// switcher's DOM lookups).
pub fn render_html_grid(matrix: &CoverageMatrix, idx: usize) -> String {
    use crate::{html_escape, html_escape_attr};

    let mut html = format!("<div class=\"coverage-block\">\n<h3>{}</h3>\n", html_escape(&matrix.name));

    if matrix.dimensions.is_empty() {
        html.push_str("<p class=\"muted\">未定义维度</p></div>\n");
        return html;
    }

    let total: usize = matrix.dimensions.iter().map(|d| d.values.len()).product();
    html.push_str(&format!(
        "<p class=\"muted\">{} 个维度，共 {} 种组合 — 供人工核对是否遗漏，不是已验证的覆盖率</p>\n",
        matrix.dimensions.len(),
        total
    ));

    if matrix.dimensions.len() == 1 {
        html.push_str("<div class=\"dim-chips\">");
        for v in &matrix.dimensions[0].values {
            html.push_str(&format!("<span class=\"chip\">{}</span>", html_escape(v)));
        }
        html.push_str("</div></div>\n");
        return html;
    }

    let row_dim = &matrix.dimensions[0];
    let col_dim = &matrix.dimensions[1];
    let extra_dims = &matrix.dimensions[2..];

    if !extra_dims.is_empty() {
        html.push_str(&format!("<div class=\"cov-switch\" data-cov=\"{idx}\">\n"));
        for (di, dim) in extra_dims.iter().enumerate() {
            html.push_str("<div class=\"cov-switch-group\">");
            html.push_str(&format!(
                "<span class=\"cov-switch-label\">{}:</span>",
                html_escape(&dim.name)
            ));
            for (vi, v) in dim.values.iter().enumerate() {
                let active = if vi == 0 { " active" } else { "" };
                html.push_str(&format!(
                    "<button type=\"button\" class=\"cov-switch-btn{active}\" data-dim=\"{di}\" data-value=\"{}\" onclick=\"covSwitch({idx},this)\">{}</button>",
                    html_escape_attr(v),
                    html_escape(v)
                ));
            }
            html.push_str("</div>\n");
        }
        html.push_str("</div>\n");
    }

    for (ci, combo) in cartesian(extra_dims).into_iter().enumerate() {
        let combo_key = combo
            .iter()
            .map(|(di, v)| format!("{di}:{}", html_escape_attr(v)))
            .collect::<Vec<_>>()
            .join("|");
        let style = if ci == 0 { "" } else { " style=\"display:none\"" };
        html.push_str(&format!(
            "<table class=\"coverage-grid\" data-cov=\"{idx}\" data-combo=\"{combo_key}\"{style}>\n<thead><tr><th></th>"
        ));
        for cv in &col_dim.values {
            html.push_str(&format!("<th>{}</th>", html_escape(cv)));
        }
        html.push_str("</tr></thead>\n<tbody>\n");
        for rv in &row_dim.values {
            html.push_str(&format!("<tr><th>{}</th>", html_escape(rv)));
            for _ in &col_dim.values {
                html.push_str("<td class=\"cov-cell\"></td>");
            }
            html.push_str("</tr>\n");
        }
        html.push_str("</tbody></table>\n");
    }

    html.push_str("</div>\n");
    html
}
