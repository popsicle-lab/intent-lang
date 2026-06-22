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

/// Build HTML table representation
pub fn render_html_table(matrix: &CoverageMatrix) -> String {
    let mut html = String::new();

    html.push_str(&format!("<h3>Coverage: {}</h3>\n", matrix.name));

    if matrix.dimensions.is_empty() {
        html.push_str("<p><i>No dimensions defined</i></p>");
        return html;
    }

    // For 2D matrix, render as table
    if matrix.dimensions.len() == 2 {
        html.push_str("<table class='coverage-matrix'>\n");
        html.push_str("<thead><tr><th></th>");

        for val in &matrix.dimensions[1].values {
            html.push_str(&format!("<th>{}</th>", val));
        }
        html.push_str("</tr></thead>\n<tbody>\n");

        for row_val in &matrix.dimensions[0].values {
            html.push_str(&format!("<tr><th>{}</th>", row_val));
            for _ in &matrix.dimensions[1].values {
                html.push_str("<td class='uncovered'>?</td>");
            }
            html.push_str("</tr>\n");
        }

        html.push_str("</tbody></table>\n");
    } else {
        // For N-dimensional, show dimension list
        html.push_str("<ul class='dimension-list'>\n");
        for dim in &matrix.dimensions {
            html.push_str(&format!(
                "<li><b>{}</b>: {} ({})</li>\n",
                dim.name,
                dim.values.join(", "),
                dim.values.len()
            ));
        }
        html.push_str("</ul>\n");
    }

    if let Some(stats) = &matrix.stats {
        html.push_str(&format!(
            "<div class='coverage-stats'>\
            <p>Total combinations: <b>{}</b></p>\
            <p>Covered: <b>{}</b></p>\
            <p>Missing: <b>{}</b></p>\
            </div>\n",
            stats.total_combinations,
            stats.covered_combinations,
            stats.missing_combinations
        ));
    }

    html
}
