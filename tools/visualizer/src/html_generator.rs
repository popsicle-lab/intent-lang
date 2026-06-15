/// HTML generator for interactive visualizations

use anyhow::Result;
use intent_syntax::ast::Program;
use std::path::PathBuf;

/// Lazy-render Mermaid diagrams when their tab becomes visible.
/// Hidden tabs (`display: none`) fail with `startOnLoad: true` in Mermaid 10.
fn mermaid_tab_script() -> &'static str {
    r#"
        mermaid.initialize({ startOnLoad: false, theme: 'default' });

        let mermaidRenderCounter = 0;

        async function renderMermaidIn(container) {
            if (container.dataset.rendered === 'true') return;
            const source = container.textContent.trim();
            if (!source) return;
            const id = 'mermaid-diagram-' + (mermaidRenderCounter++);
            try {
                const { svg } = await mermaid.render(id, source);
                container.innerHTML = svg;
                container.dataset.rendered = 'true';
            } catch (err) {
                container.innerHTML = '<pre class="mermaid-error">' + String(err) + '</pre>';
                container.dataset.rendered = 'error';
            }
        }

        async function renderActiveTabDiagrams() {
            const active = document.querySelector('.tab-content.active');
            if (!active) return;
            const pending = active.querySelectorAll('.mermaid:not([data-rendered])');
            for (const el of pending) {
                await renderMermaidIn(el);
            }
        }

        document.addEventListener('DOMContentLoaded', () => {
            renderActiveTabDiagrams();
        });

        function switchTab(index) {
            const tabs = document.querySelectorAll('.tab');
            const contents = document.querySelectorAll('.tab-content');

            tabs.forEach((tab, i) => {
                if (i === index) {
                    tab.classList.add('active');
                    contents[i].classList.add('active');
                } else {
                    tab.classList.remove('active');
                    contents[i].classList.remove('active');
                }
            });

            renderActiveTabDiagrams();
        }
"#
}

pub fn generate_interactive_html(program: &Program, source: &str) -> Result<String> {
    let goal_graph = crate::goal_graph::build_goal_graph(program);
    let intent_graph = crate::intent_graph::build_intent_graph(program);
    let coverage_matrix = crate::coverage_matrix::build_coverage_matrix(program);

    use crate::mermaid::MermaidRenderable;

    let goal_graph_mermaid = goal_graph.to_mermaid();
    let intent_graph_mermaid = intent_graph.to_mermaid();

    // Remove markdown code fence markers for HTML embedding
    let goal_mermaid = goal_graph_mermaid
        .trim()
        .trim_start_matches("```mermaid")
        .trim_end_matches("```")
        .trim();
    let intent_mermaid = intent_graph_mermaid
        .trim()
        .trim_start_matches("```mermaid")
        .trim_end_matches("```")
        .trim();
    let coverage_table = crate::coverage_matrix::render_html_table(&coverage_matrix);

    let html = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Intent-Lang Visualization</title>
    <script src="https://cdn.jsdelivr.net/npm/mermaid@10/dist/mermaid.min.js"></script>
    <style>
        * {{ margin: 0; padding: 0; box-sizing: border-box; }}
        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            background: #f5f5f5;
            padding: 20px;
        }}
        .container {{
            max-width: 1400px;
            margin: 0 auto;
            background: white;
            border-radius: 8px;
            box-shadow: 0 2px 8px rgba(0,0,0,0.1);
        }}
        .header {{
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
            padding: 30px;
            border-radius: 8px 8px 0 0;
        }}
        .header h1 {{
            font-size: 32px;
            margin-bottom: 10px;
        }}
        .header p {{
            opacity: 0.9;
            font-size: 16px;
        }}
        .tabs {{
            display: flex;
            background: #f8f9fa;
            border-bottom: 2px solid #e0e0e0;
        }}
        .tab {{
            padding: 15px 30px;
            cursor: pointer;
            font-weight: 500;
            transition: all 0.3s;
            border-bottom: 3px solid transparent;
        }}
        .tab:hover {{
            background: #e9ecef;
        }}
        .tab.active {{
            background: white;
            border-bottom-color: #667eea;
            color: #667eea;
        }}
        .tab-content {{
            padding: 30px;
            display: none;
        }}
        .tab-content.active {{
            display: block;
        }}
        .visualization-section {{
            margin-bottom: 40px;
        }}
        .visualization-section h2 {{
            color: #333;
            margin-bottom: 20px;
            font-size: 24px;
            border-left: 4px solid #667eea;
            padding-left: 15px;
        }}
        .mermaid {{
            background: #fafafa;
            border: 1px solid #e0e0e0;
            border-radius: 4px;
            padding: 20px;
            overflow-x: auto;
        }}
        .mermaid-error {{
            color: #c62828;
            white-space: pre-wrap;
            font-family: ui-monospace, monospace;
            font-size: 13px;
        }}
        .coverage-matrix {{
            border-collapse: collapse;
            width: 100%;
            margin: 20px 0;
        }}
        .coverage-matrix th,
        .coverage-matrix td {{
            border: 1px solid #ddd;
            padding: 12px;
            text-align: center;
        }}
        .coverage-matrix th {{
            background: #667eea;
            color: white;
            font-weight: 600;
        }}
        .coverage-matrix td.covered {{
            background: #4caf50;
            color: white;
        }}
        .coverage-matrix td.uncovered {{
            background: #ffeb3b;
        }}
        .coverage-stats {{
            background: #e8f5e9;
            border-left: 4px solid #4caf50;
            padding: 15px;
            margin: 20px 0;
            border-radius: 4px;
        }}
        .coverage-stats p {{
            margin: 8px 0;
        }}
        .dimension-list {{
            list-style: none;
            padding: 0;
        }}
        .dimension-list li {{
            padding: 10px;
            margin: 8px 0;
            background: #f8f9fa;
            border-left: 3px solid #667eea;
            border-radius: 4px;
        }}
        .source-code {{
            background: #282c34;
            color: #abb2bf;
            padding: 20px;
            border-radius: 4px;
            overflow-x: auto;
            font-family: 'Menlo', 'Monaco', 'Courier New', monospace;
            font-size: 14px;
            line-height: 1.6;
        }}
        .legend {{
            display: flex;
            gap: 20px;
            margin: 20px 0;
            flex-wrap: wrap;
        }}
        .legend-item {{
            display: flex;
            align-items: center;
            gap: 8px;
        }}
        .legend-box {{
            width: 20px;
            height: 20px;
            border-radius: 3px;
        }}
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>🎯 Intent-Lang Visualization</h1>
            <p>Interactive business intent analysis and dependency graphs</p>
        </div>

        <div class="tabs">
            <div class="tab active" onclick="switchTab(0)">Goal Graph</div>
            <div class="tab" onclick="switchTab(1)">Intent Graph</div>
            <div class="tab" onclick="switchTab(2)">Coverage Matrix</div>
            <div class="tab" onclick="switchTab(3)">Source Code</div>
        </div>

        <div class="tab-content active">
            <div class="visualization-section">
                <h2>Goal Dependency Graph</h2>
                <p style="color: #666; margin-bottom: 20px;">
                    Shows how business goals are realized through safety rules, intents, and theorems.
                </p>
                <div class="legend">
                    <div class="legend-item">
                        <div class="legend-box" style="background: #e1f5ff; border: 2px solid #01579b;"></div>
                        <span>Goal</span>
                    </div>
                    <div class="legend-item">
                        <div class="legend-box" style="background: #fff3e0; border: 2px solid #e65100;"></div>
                        <span>Safety</span>
                    </div>
                    <div class="legend-item">
                        <div class="legend-box" style="background: #f3e5f5; border: 2px solid #4a148c;"></div>
                        <span>Intent</span>
                    </div>
                    <div class="legend-item">
                        <div class="legend-box" style="background: #e8f5e9; border: 2px solid #1b5e20;"></div>
                        <span>Theorem</span>
                    </div>
                </div>
                <div class="mermaid">
{}
                </div>
            </div>
        </div>

        <div class="tab-content">
            <div class="visualization-section">
                <h2>Intent Relationship Graph</h2>
                <p style="color: #666; margin-bottom: 20px;">
                    Shows intents grouped by implementation status and their data flow relationships.
                </p>
                <div class="mermaid">
{}
                </div>
            </div>
        </div>

        <div class="tab-content">
            <div class="visualization-section">
                <h2>Coverage Matrix</h2>
                <p style="color: #666; margin-bottom: 20px;">
                    Shows the dimensions of test coverage and combination statistics.
                </p>
{}
            </div>
        </div>

        <div class="tab-content">
            <div class="visualization-section">
                <h2>Source Code</h2>
                <pre class="source-code">{}</pre>
            </div>
        </div>
    </div>

    <script>
{}
    </script>
</body>
</html>"#,
        goal_mermaid.trim(),
        intent_mermaid.trim(),
        coverage_table,
        html_escape(source),
        mermaid_tab_script()
    );

    Ok(html)
}

pub fn generate_index_html(output_dir: &PathBuf) -> Result<String> {
    // Check if .mmd files exist
    let goal_mmd = output_dir.join("goalgraph.mmd");
    let intent_mmd = output_dir.join("intentgraph.mmd");
    let safety_mmd = output_dir.join("safetynetwork.mmd");
    let coverage_mmd = output_dir.join("coveragematrix.mmd");

    // Read mermaid files
    let goal_content = std::fs::read_to_string(&goal_mmd)
        .unwrap_or_default()
        .trim()
        .trim_start_matches("```mermaid")
        .trim_end_matches("```")
        .trim()
        .to_string();

    let intent_content = std::fs::read_to_string(&intent_mmd)
        .unwrap_or_default()
        .trim()
        .trim_start_matches("```mermaid")
        .trim_end_matches("```")
        .trim()
        .to_string();

    let safety_content = std::fs::read_to_string(&safety_mmd)
        .unwrap_or_default()
        .trim()
        .trim_start_matches("```mermaid")
        .trim_end_matches("```")
        .trim()
        .to_string();

    let coverage_content = std::fs::read_to_string(&coverage_mmd)
        .unwrap_or_default()
        .trim()
        .trim_start_matches("```mermaid")
        .trim_end_matches("```")
        .trim()
        .to_string();

    Ok(format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Intent-Lang Visualizations</title>
    <script src="https://cdn.jsdelivr.net/npm/mermaid@10/dist/mermaid.min.js"></script>
    <style>
        * {{ margin: 0; padding: 0; box-sizing: border-box; }}
        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            background: #f5f5f5;
            padding: 20px;
        }}
        .container {{
            max-width: 1400px;
            margin: 0 auto;
            background: white;
            border-radius: 8px;
            box-shadow: 0 2px 8px rgba(0,0,0,0.1);
        }}
        .header {{
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
            padding: 30px;
            border-radius: 8px 8px 0 0;
        }}
        .header h1 {{
            font-size: 32px;
            margin-bottom: 10px;
        }}
        .tabs {{
            display: flex;
            background: #f8f9fa;
            border-bottom: 2px solid #e0e0e0;
            overflow-x: auto;
        }}
        .tab {{
            padding: 15px 30px;
            cursor: pointer;
            font-weight: 500;
            transition: all 0.3s;
            border-bottom: 3px solid transparent;
            white-space: nowrap;
        }}
        .tab:hover {{
            background: #e9ecef;
        }}
        .tab.active {{
            background: white;
            border-bottom-color: #667eea;
            color: #667eea;
        }}
        .tab-content {{
            padding: 30px;
            display: none;
        }}
        .tab-content.active {{
            display: block;
        }}
        .viz-section h2 {{
            color: #333;
            margin-bottom: 20px;
            font-size: 24px;
            border-left: 4px solid #667eea;
            padding-left: 15px;
        }}
        .mermaid {{
            background: #fafafa;
            border: 1px solid #e0e0e0;
            border-radius: 4px;
            padding: 20px;
            overflow-x: auto;
        }}
        .mermaid-error {{
            color: #c62828;
            white-space: pre-wrap;
            font-family: ui-monospace, monospace;
            font-size: 13px;
        }}
        .download-links {{
            margin-top: 20px;
            padding: 15px;
            background: #f8f9fa;
            border-radius: 4px;
        }}
        .download-links a {{
            display: inline-block;
            margin-right: 15px;
            color: #667eea;
            text-decoration: none;
        }}
        .download-links a:hover {{
            text-decoration: underline;
        }}
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>🎯 Intent-Lang Visualizations</h1>
            <p>Interactive visualization suite</p>
        </div>

        <div class="tabs">
            <div class="tab active" onclick="switchTab(0)">📊 Goal Graph</div>
            <div class="tab" onclick="switchTab(1)">🔄 Intent Graph</div>
            <div class="tab" onclick="switchTab(2)">🛡️ Safety Network</div>
            <div class="tab" onclick="switchTab(3)">📈 Coverage Matrix</div>
        </div>

        <div class="tab-content active">
            <div class="viz-section">
                <h2>Goal Dependency Graph</h2>
                <p style="color: #666; margin-bottom: 20px;">
                    Shows how business goals are realized through safety rules, intents, and theorems.
                </p>
                <div class="mermaid">
{}
                </div>
                <div class="download-links">
                    <a href="goalgraph.mmd" download>⬇️ Download Mermaid</a>
                </div>
            </div>
        </div>

        <div class="tab-content">
            <div class="viz-section">
                <h2>Intent Relationship Graph</h2>
                <p style="color: #666; margin-bottom: 20px;">
                    Shows intents grouped by implementation status and their data flow relationships.
                </p>
                <div class="mermaid">
{}
                </div>
                <div class="download-links">
                    <a href="intentgraph.mmd" download>⬇️ Download Mermaid</a>
                </div>
            </div>
        </div>

        <div class="tab-content">
            <div class="viz-section">
                <h2>Safety Rule Network</h2>
                <p style="color: #666; margin-bottom: 20px;">
                    Shows safety rules and the types they constrain.
                </p>
                <div class="mermaid">
{}
                </div>
                <div class="download-links">
                    <a href="safetynetwork.mmd" download>⬇️ Download Mermaid</a>
                </div>
            </div>
        </div>

        <div class="tab-content">
            <div class="viz-section">
                <h2>Coverage Matrix</h2>
                <p style="color: #666; margin-bottom: 20px;">
                    Shows the dimensions of test coverage and combination statistics.
                </p>
                <div class="mermaid">
{}
                </div>
                <div class="download-links">
                    <a href="coveragematrix.mmd" download>⬇️ Download Mermaid</a>
                </div>
            </div>
        </div>
    </div>

    <script>
{}
    </script>
</body>
</html>"#,
        goal_content,
        intent_content,
        safety_content,
        coverage_content,
        mermaid_tab_script()
    ))
}

fn html_escape(text: &str) -> String {
    text.replace("&", "&amp;")
        .replace("<", "&lt;")
        .replace(">", "&gt;")
        .replace("\"", "&quot;")
        .replace("'", "&#39;")
}
