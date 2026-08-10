//! HTML generator for the interactive visualization page.
//!
//! One template, one entry point (`generate_interactive_html`), used by both
//! `--interactive -o file.html` (single self-contained file) and `--all
//! --output-dir dir/` (same page, written as `dir/index.html`, alongside the
//! `.mmd` exports `main.rs` writes separately). There used to be two
//! templates with ~75% overlapping content and no clear division of
//! responsibility between them; see `examples/viz-demo/README.md` for the
//! reasoning behind collapsing them into one.
//!
//! Tab order is a reading order, business-first: state machine (the
//! lifecycle, and the one big "what are all the operations" table) → goal
//! traceability → safety rules (as an auditable checklist, not a diagram —
//! a two-color bipartite graph of 6 types didn't carry enough structure to
//! earn a diagram) → coverage (an honest dimension memo, no invented
//! covered/missing counts) → annotated source.

use anyhow::Result;
use intent_lang_syntax::ast::{Declaration, Program};

use crate::goal_graph::{GoalGraph, NodeType};
use crate::model::{build_doc_model, DocModel};
use crate::state_machine::StateMachine;
use crate::{html_escape, html_escape_attr};

const STYLE: &str = r#"
* { margin: 0; padding: 0; box-sizing: border-box; }
body {
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
    background: #f5f5f5;
}
.container { max-width: 1400px; margin: 0 auto; background: #fff; min-height: 100vh; }
.header {
    background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
    color: #fff; padding: 28px 30px;
}
.header h1 { font-size: 28px; margin-bottom: 6px; }
.header p { opacity: .9; font-size: 14px; }
.tabs { display: flex; background: #f8f9fa; border-bottom: 2px solid #e0e0e0; overflow-x: auto; }
.tab {
    padding: 14px 26px; cursor: pointer; font-weight: 500; white-space: nowrap;
    border-bottom: 3px solid transparent; transition: background .2s, color .2s;
}
.tab:hover { background: #e9ecef; }
.tab.active { background: #fff; border-bottom-color: #667eea; color: #667eea; }
.tab-content { padding: 28px 30px; display: none; }
.tab-content.active { display: block; }
.section h2 {
    color: #333; margin-bottom: 8px; font-size: 22px;
    border-left: 4px solid #667eea; padding-left: 14px;
}
.section-desc { color: #666; margin-bottom: 18px; font-size: 14px; }
.muted { color: #888; font-size: 13px; }

.diagram-frame { margin-bottom: 20px; }
.diagram-toolbar { display: flex; gap: 6px; margin-bottom: 8px; }
.diagram-toolbar button {
    border: 1px solid #ddd; background: #fff; border-radius: 4px; width: 30px; height: 28px;
    cursor: pointer; font-size: 14px; color: #555;
}
.diagram-toolbar button:hover { background: #f0f0ff; border-color: #667eea; color: #667eea; }
.diagram-toolbar .dz-reset { width: auto; padding: 0 10px; }
.diagram-viewport {
    background: #fafafa; border: 1px solid #e0e0e0; border-radius: 4px;
    overflow: hidden; position: relative; height: 620px; cursor: grab;
}
.diagram-viewport.dragging { cursor: grabbing; }
.diagram-viewport .mermaid {
    padding: 20px; transform-origin: 0 0; width: max-content;
}
.mermaid-error { color: #c62828; white-space: pre-wrap; font-family: ui-monospace, monospace; font-size: 13px; padding: 16px; }
.legend { display: flex; gap: 18px; margin-bottom: 16px; flex-wrap: wrap; font-size: 13px; }
.legend-item { display: flex; align-items: center; gap: 6px; }
.legend-box { width: 16px; height: 16px; border-radius: 3px; flex: none; }

table.data-table { border-collapse: collapse; width: 100%; margin-top: 6px; font-size: 14px; }
table.data-table th, table.data-table td { border: 1px solid #e3e3e3; padding: 9px 10px; text-align: left; vertical-align: top; }
table.data-table th { background: #667eea; color: #fff; font-weight: 600; }
table.data-table tbody tr { cursor: pointer; }
table.data-table tbody tr:hover { background: #f0f0ff; }
table.data-table td.name-cell { font-family: ui-monospace, Menlo, Monaco, monospace; white-space: nowrap; font-weight: 600; color: #4a148c; }
.group-heading { margin: 22px 0 6px; font-size: 16px; color: #333; }
.group-heading:first-child { margin-top: 0; }
.chip { display: inline-block; background: #eef; color: #333; border-radius: 999px; padding: 2px 10px; margin: 2px; font-size: 12px; }
button.chip-link {
    display: inline-block; background: #eef2ff; color: #3949ab; border: 1px solid #c5cae9; border-radius: 999px;
    padding: 3px 11px; margin: 2px; font-size: 12px; cursor: pointer;
}
button.chip-link:hover { background: #dde3ff; }

.coverage-block { margin-bottom: 32px; }
.coverage-block h3 { font-size: 16px; color: #333; margin-bottom: 4px; }
.dim-chips { margin-top: 8px; }
.cov-switch { display: flex; gap: 18px; margin: 10px 0 14px; flex-wrap: wrap; }
.cov-switch-group { display: flex; align-items: center; gap: 4px; flex-wrap: wrap; }
.cov-switch-label { font-size: 12px; color: #666; margin-right: 4px; }
.cov-switch-btn {
    border: 1px solid #ccc; background: #fff; border-radius: 4px; padding: 4px 10px;
    font-size: 12px; cursor: pointer; color: #444;
}
.cov-switch-btn.active { background: #667eea; border-color: #667eea; color: #fff; }
table.coverage-grid { border-collapse: collapse; margin-top: 4px; }
table.coverage-grid th, table.coverage-grid td { border: 1px solid #e3e3e3; padding: 8px 12px; text-align: center; font-size: 13px; }
table.coverage-grid th { background: #f3f4f8; color: #444; font-weight: 600; }
table.coverage-grid td.cov-cell { background: repeating-linear-gradient(45deg, #fafafa, #fafafa 6px, #f2f2f2 6px, #f2f2f2 12px); min-width: 46px; }

.source-view {
    background: #282c34; color: #abb2bf; border-radius: 6px; overflow-x: auto;
    font-family: 'Menlo', 'Monaco', 'Courier New', monospace; font-size: 13px; line-height: 1.6;
    max-height: 78vh; overflow-y: auto;
}
.src-line { display: flex; padding: 0 12px; white-space: pre; }
.src-line .ln { flex: none; width: 3.5em; text-align: right; margin-right: 14px; color: #5c6370; user-select: none; }
.src-line .code { flex: 1 1 auto; }
.src-line.src-flash { background: #3a3f4b; animation: flash-fade 1.6s ease-out; }
@keyframes flash-fade { from { background: #55597a; } to { background: transparent; } }
.tok-kw { color: #c678dd; }
.tok-lit { color: #d19a66; }
.tok-num { color: #d19a66; }
.tok-str { color: #98c379; }
.tok-ann { color: #e06c75; }
.tok-op { color: #56b6c2; }
.tok-cmt { color: #5c6370; font-style: italic; }
.tok-ident { color: #abb2bf; }

.side-panel {
    position: fixed; top: 0; right: 0; height: 100%; width: 420px; max-width: 92vw;
    background: #fff; box-shadow: -6px 0 20px rgba(0,0,0,.18); transform: translateX(100%);
    transition: transform .25s ease; z-index: 1000; overflow-y: auto; padding: 26px 24px;
}
.side-panel.open { transform: translateX(0); }
.side-panel-close {
    position: absolute; top: 14px; right: 16px; border: none; background: none;
    font-size: 22px; cursor: pointer; color: #888; line-height: 1;
}
.side-panel-close:hover { color: #333; }
.side-panel-body h3 { font-size: 19px; color: #222; margin-bottom: 8px; padding-right: 24px; }
.side-panel-body h4 { font-size: 13px; color: #667eea; margin: 18px 0 6px; text-transform: uppercase; letter-spacing: .03em; }
.panel-doc { color: #555; font-size: 14px; line-height: 1.5; margin-bottom: 6px; }
.panel-meta { font-size: 12px; color: #999; }
.panel-meta a { color: #667eea; cursor: pointer; text-decoration: none; }
.panel-code { font-family: ui-monospace, Menlo, Monaco, monospace; font-size: 13px; background: #f6f6fa; padding: 8px 10px; border-radius: 4px; word-break: break-word; }
ul.clause-list { list-style: none; margin-top: 4px; }
ul.clause-list li.clause {
    border-left: 3px solid #ccc; background: #f9f9fb; border-radius: 0 4px 4px 0;
    padding: 8px 10px; margin-bottom: 6px; font-size: 13px;
}
li.clause-require { border-left-color: #e65100; }
li.clause-ensure { border-left-color: #1b5e20; }
li.clause-invariant { border-left-color: #4a148c; }
.clause-kind { font-weight: 700; text-transform: uppercase; font-size: 11px; color: #666; margin-right: 6px; }
.clause-label { font-style: italic; color: #888; }
.clause-list code { display: block; margin-top: 3px; font-family: ui-monospace, Menlo, Monaco, monospace; white-space: pre-wrap; word-break: break-word; }
.tag-reject { display: inline-block; margin-left: 6px; background: #ffe0b2; color: #a05a00; border-radius: 4px; padding: 1px 6px; font-size: 11px; }
.panel-source-btn {
    display: inline-block; margin-top: 18px; border: 1px solid #667eea; color: #667eea; background: #fff;
    border-radius: 4px; padding: 6px 14px; font-size: 13px; cursor: pointer;
}
.panel-source-btn:hover { background: #667eea; color: #fff; }
.chooser { margin-top: 8px; }
"#;

fn mermaid_page_script() -> String {
    r#"
mermaid.initialize({ startOnLoad: false, theme: 'default', securityLevel: 'loose' });
let mermaidRenderCounter = 0;

async function renderMermaidIn(container) {
    if (container.dataset.rendered === 'true') return;
    const source = container.textContent.trim();
    if (!source) return;
    const id = 'mermaid-diagram-' + (mermaidRenderCounter++);
    try {
        const { svg, bindFunctions } = await mermaid.render(id, source);
        container.innerHTML = svg;
        if (bindFunctions) bindFunctions(container);
        applyInteractivity(container);
        container.dataset.rendered = 'true';
    } catch (err) {
        container.innerHTML = '<pre class="mermaid-error">' + String(err) + '</pre>';
        container.dataset.rendered = 'error';
    }
    const frame = container.closest('.diagram-frame');
    if (frame) initPanZoom(frame);
}

// Hover tooltip (native SVG <title>) + click-through to the side panel, for
// both node shapes (goal graph) and transition edge labels (state machine).
// Keyed by the node/edge's visible text against window.__MODEL.
function applyInteractivity(container) {
    const model = window.__MODEL || { intents: {}, goals: {}, safety: {} };
    const svg = container.querySelector('svg');
    if (!svg) return;

    const kindOf = (name) => {
        if (model.intents && model.intents[name]) return 'intent';
        if (model.goals && model.goals[name]) return 'goal';
        if (model.safety && model.safety[name]) return 'safety';
        return null;
    };
    const docOf = (name) => {
        const bucket = model.intents[name] || model.goals[name] || model.safety[name];
        return bucket ? bucket.doc : null;
    };
    const setSvgTitle = (el, text) => {
        if (!el || !text) return;
        let t = el.querySelector(':scope > title');
        if (!t) {
            t = document.createElementNS('http://www.w3.org/2000/svg', 'title');
            el.insertBefore(t, el.firstChild);
        }
        t.textContent = text;
    };

    svg.querySelectorAll('g.node').forEach((g) => {
        const label = g.querySelector('.nodeLabel, .label, foreignObject, text');
        const name = (label ? label.textContent : '').trim();
        const kind = kindOf(name);
        const doc = docOf(name);
        if (doc) setSvgTitle(g, doc);
        if (kind) {
            g.style.cursor = 'pointer';
            g.addEventListener('click', () => openPanel(kind, name));
        }
    });

    svg.querySelectorAll('.edgeLabel, span.edgeLabel, .edgeLabels .label').forEach((el) => {
        const text = (el.textContent || '').trim();
        if (!text) return;
        const names = text.split('/').map((s) => s.trim()).filter((n) => model.intents[n]);
        if (!names.length) return;
        const doc = docOf(names[0]);
        if (doc) el.setAttribute('title', doc);
        el.style.cursor = 'pointer';
        el.addEventListener('click', () => {
            if (names.length === 1) openPanel('intent', names[0]);
            else openPanelChooser(names);
        });
    });
}

// ── Pan / zoom (wheel to zoom, drag to pan, per-diagram reset) ──────────
function initPanZoom(frame) {
    if (frame.dataset.panzoomInit === 'true') return;
    const viewport = frame.querySelector('.diagram-viewport');
    const stage = frame.querySelector('.mermaid');
    if (!viewport || !stage) return;
    let scale = 1, tx = 0, ty = 0, dragging = false, lastX = 0, lastY = 0;
    const apply = () => { stage.style.transform = `translate(${tx}px, ${ty}px) scale(${scale})`; };
    const zoomBy = (factor) => { scale = Math.min(4, Math.max(0.2, scale * factor)); apply(); };

    viewport.addEventListener('wheel', (e) => {
        e.preventDefault();
        zoomBy(e.deltaY < 0 ? 1.1 : 0.9);
    }, { passive: false });
    viewport.addEventListener('mousedown', (e) => {
        dragging = true; lastX = e.clientX; lastY = e.clientY;
        viewport.classList.add('dragging');
    });
    window.addEventListener('mousemove', (e) => {
        if (!dragging) return;
        tx += e.clientX - lastX; ty += e.clientY - lastY;
        lastX = e.clientX; lastY = e.clientY;
        apply();
    });
    window.addEventListener('mouseup', () => { dragging = false; viewport.classList.remove('dragging'); });

    const zin = frame.querySelector('.dz-in');
    const zout = frame.querySelector('.dz-out');
    const reset = frame.querySelector('.dz-reset');
    if (zin) zin.addEventListener('click', () => zoomBy(1.25));
    if (zout) zout.addEventListener('click', () => zoomBy(0.8));
    if (reset) reset.addEventListener('click', () => { scale = 1; tx = 0; ty = 0; apply(); });

    frame.dataset.panzoomInit = 'true';
}

async function renderActiveTabDiagrams() {
    const active = document.querySelector('.tab-content.active');
    if (!active) return;
    const pending = active.querySelectorAll('.mermaid:not([data-rendered])');
    for (const el of pending) {
        await renderMermaidIn(el);
    }
}

function switchTab(index) {
    const tabs = document.querySelectorAll('.tab');
    const contents = document.querySelectorAll('.tab-content');
    tabs.forEach((tab, i) => {
        tab.classList.toggle('active', i === index);
        contents[i].classList.toggle('active', i === index);
    });
    renderActiveTabDiagrams();
}

// ── Side panel ───────────────────────────────────────────────────────────
function esc(s) {
    return String(s == null ? '' : s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}
function escAttr(s) { return esc(s).replace(/"/g, '&quot;'); }
function chip(kind, name) {
    return `<button class="chip-link" onclick="openPanel('${kind}','${escAttr(name)}')">${esc(name)}</button>`;
}
function sourceButton(line) {
    return `<button type="button" class="panel-source-btn" onclick="jumpToSource(${line})">查看源码 → 第 ${line} 行</button>`;
}
function renderClauses(clauses) {
    if (!clauses || !clauses.length) return '';
    let html = '<h4>契约</h4><ul class="clause-list">';
    for (const c of clauses) {
        html += `<li class="clause clause-${c.kind}"><span class="clause-kind">${c.kind}</span>`;
        if (c.label) html += `<span class="clause-label">${esc(c.label)}</span>`;
        html += `<code>${esc(c.text)}</code>`;
        if (c.else_reject) html += '<span class="tag-reject">else reject</span>';
        html += '</li>';
    }
    return html + '</ul>';
}
function renderGoalRefs(title, refs) {
    if (!refs || !refs.length) return '';
    const links = refs.map((r) => chip(r.kind || 'goal', r.name)).join(' ');
    return `<h4>${title}</h4><div>${links}</div>`;
}
function renderIntentPanel(d) {
    let html = `<h3>${esc(d.name)}</h3>`;
    if (d.doc) html += `<p class="panel-doc">${esc(d.doc)}</p>`;
    if (d.params && d.params.length) html += `<h4>参数</h4><p class="panel-code">${d.params.map(esc).join(', ')}</p>`;
    if (d.modifies && d.modifies.length) html += `<h4>modifies</h4><p class="panel-code">${d.modifies.map(esc).join(', ')}</p>`;
    html += renderClauses(d.clauses);
    html += renderGoalRefs('服务于目标', d.goals);
    html += sourceButton(d.line);
    return html;
}
function renderSafetyPanel(d) {
    let html = `<h3>${esc(d.name)}</h3>`;
    if (d.params && d.params.length) html += `<h4>参数</h4><p class="panel-code">${d.params.map(esc).join(', ')}</p>`;
    if (d.invariants && d.invariants.length) {
        html += '<h4>不变量</h4><ul class="clause-list">';
        for (const inv of d.invariants) html += `<li class="clause clause-invariant"><code>${esc(inv)}</code></li>`;
        html += '</ul>';
    }
    html += renderGoalRefs('服务于目标', d.goals);
    html += sourceButton(d.line);
    return html;
}
function renderGoalPanel(d) {
    let html = `<h3>${esc(d.name)}</h3>`;
    const kindLabel = d.kind === 'capability' ? '能力' : d.kind === 'guardrail' ? '护栏' : null;
    if (kindLabel || d.group) {
        html += `<p class="panel-meta">${kindLabel ? esc(kindLabel) : ''}${d.group ? ' · ' + esc(d.group) : ''}</p>`;
    }
    if (d.rationale) html += `<h4>为什么</h4><p class="panel-doc">${esc(d.rationale)}</p>`;
    if (d.measure) html += `<h4>怎么算做到</h4><p class="panel-doc">${esc(d.measure)}</p>`;
    if (d.stakeholder && d.stakeholder.length) html += `<h4>利益相关方</h4><p>${d.stakeholder.map(esc).join(', ')}</p>`;
    html += renderGoalRefs('由以下实现', d.realized_by);
    html += sourceButton(d.line);
    return html;
}
function bucketFor(kind) { return kind === 'intent' ? 'intents' : kind === 'goal' ? 'goals' : 'safety'; }
function openPanel(kind, name) {
    const model = window.__MODEL || {};
    const data = (model[bucketFor(kind)] || {})[name];
    const body = document.getElementById('sidePanelBody');
    if (!data) {
        body.innerHTML = `<h3>${esc(name)}</h3><p class="muted">未找到详情（可能是分组或跨主题辅助节点）。</p>`;
    } else if (kind === 'intent') {
        body.innerHTML = renderIntentPanel(data);
    } else if (kind === 'safety') {
        body.innerHTML = renderSafetyPanel(data);
    } else {
        body.innerHTML = renderGoalPanel(data);
    }
    document.getElementById('sidePanel').classList.add('open');
}
function openPanelChooser(names) {
    const body = document.getElementById('sidePanelBody');
    body.innerHTML = '<h3>此转换由多个操作触发</h3><div class="chooser">' +
        names.map((n) => chip('intent', n)).join('') + '</div>';
    document.getElementById('sidePanel').classList.add('open');
}
function closePanel() { document.getElementById('sidePanel').classList.remove('open'); }
function jumpToSource(line) {
    closePanel();
    switchTab(4);
    requestAnimationFrame(() => {
        const el = document.getElementById('L' + line);
        if (!el) return;
        el.scrollIntoView({ block: 'center' });
        el.classList.add('src-flash');
        setTimeout(() => el.classList.remove('src-flash'), 1600);
    });
}

// ── Coverage dimension switcher ─────────────────────────────────────────
const covState = {};
function covSwitch(covIdx, btn) {
    covState[covIdx] = covState[covIdx] || {};
    covState[covIdx][btn.dataset.dim] = btn.dataset.value;
    btn.parentElement.querySelectorAll('.cov-switch-btn').forEach((b) => b.classList.remove('active'));
    btn.classList.add('active');
    document.querySelectorAll('table.coverage-grid[data-cov="' + covIdx + '"]').forEach((table) => {
        const combo = table.dataset.combo || '';
        const pairs = combo.split('|').filter(Boolean).map((p) => p.split(':'));
        const match = pairs.every(([di, val]) => covState[covIdx][di] === undefined || covState[covIdx][di] === val);
        table.style.display = match ? '' : 'none';
    });
}
function initCoverageDefaults() {
    document.querySelectorAll('.cov-switch').forEach((group) => {
        const covIdx = group.dataset.cov;
        covState[covIdx] = covState[covIdx] || {};
        group.querySelectorAll('.cov-switch-group').forEach((g) => {
            const firstBtn = g.querySelector('.cov-switch-btn');
            if (firstBtn) covState[covIdx][firstBtn.dataset.dim] = firstBtn.dataset.value;
        });
    });
}

document.addEventListener('DOMContentLoaded', () => {
    initCoverageDefaults();
    renderActiveTabDiagrams();
});
"#
    .to_string()
}

/// Build the shared detail model plus every rendered fragment, then compose
/// the final page. Used for both `--interactive` (single file) and `--all`
/// (written as `index.html` alongside the `.mmd` exports).
pub fn generate_interactive_html(program: &Program, source: &str) -> Result<String> {
    use crate::mermaid::MermaidRenderable;

    let goal_graph = crate::goal_graph::build_goal_graph(program);
    // Every declared `@lifecycle` gets its own diagram. Showing only one meant
    // a second lifecycle (e.g. an SN-lookup phase alongside a registration
    // phase) was simply invisible on the page.
    let mut state_machines = crate::state_machine::lifecycle_state_machines(program);
    if state_machines.is_empty() {
        state_machines.push(crate::state_machine::build_state_machine(program));
    }
    let coverage_matrices = crate::coverage_matrix::build_all_coverage_matrices(program);
    let doc_model = build_doc_model(program, source);

    let flowchart = crate::flowchart::build_flowchart(program);

    let goal_mermaid = crate::unfence_mermaid(&goal_graph.to_mermaid());
    let flow_mermaid = crate::unfence_mermaid(&flowchart.to_mermaid());

    let state_tab: String = state_machines
        .iter()
        .map(|sm| render_state_machine_tab(sm, &doc_model, program))
        .collect();
    let flowchart_tab = render_flowchart_tab(&flow_mermaid, &state_machines[0]);
    let goal_tab = render_goal_graph_tab(&goal_mermaid);
    let safety_tab = render_safety_tab(&goal_graph, &doc_model);
    let coverage_tab = render_coverage_tab(&coverage_matrices);
    let source_tab = crate::source_view::render_source_html(source);

    let model_json = serde_json::to_string(&doc_model)?.replace("</", "<\\/");

    let counts = format!(
        "{} 个操作 · {} 个目标 · {} 条安全规则",
        doc_model.intents.len(),
        doc_model.goals.len(),
        doc_model.safety.len()
    );

    Ok(format!(
        r#"<!DOCTYPE html>
<html lang="zh">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Intent-Lang Visualization</title>
<script src="https://cdn.jsdelivr.net/npm/mermaid@10/dist/mermaid.min.js"></script>
<style>{STYLE}</style>
</head>
<body>
<div class="container">
    <div class="header">
        <h1>🎯 Intent-Lang Visualization</h1>
        <p>{counts}</p>
    </div>
    <div class="tabs">
        <div class="tab active" onclick="switchTab(0)">状态机</div>
        <div class="tab" onclick="switchTab(1)">业务流程图</div>
        <div class="tab" onclick="switchTab(2)">目标追溯</div>
        <div class="tab" onclick="switchTab(3)">安全规则</div>
        <div class="tab" onclick="switchTab(4)">覆盖备忘</div>
        <div class="tab" onclick="switchTab(5)">源码</div>
    </div>
    <div class="tab-content active">{state_tab}</div>
    <div class="tab-content">{flowchart_tab}</div>
    <div class="tab-content">{goal_tab}</div>
    <div class="tab-content">{safety_tab}</div>
    <div class="tab-content">{coverage_tab}</div>
    <div class="tab-content"><div class="section"><h2>源码</h2><div class="source-view">{source_tab}</div></div></div>
</div>
<div class="side-panel" id="sidePanel">
    <button class="side-panel-close" onclick="closePanel()">&times;</button>
    <div class="side-panel-body" id="sidePanelBody"></div>
</div>
<script>
window.__MODEL = {model_json};
{script}
</script>
</body>
</html>"#,
        STYLE = STYLE,
        counts = counts,
        state_tab = state_tab,
        flowchart_tab = flowchart_tab,
        goal_tab = goal_tab,
        safety_tab = safety_tab,
        coverage_tab = coverage_tab,
        source_tab = source_tab,
        model_json = model_json,
        script = mermaid_page_script(),
    ))
}

fn diagram_frame(id: &str, mermaid_body: &str) -> String {
    format!(
        r#"<div class="diagram-frame" data-frame="{id}">
    <div class="diagram-toolbar">
        <button type="button" class="dz-in" title="放大">＋</button>
        <button type="button" class="dz-out" title="缩小">－</button>
        <button type="button" class="dz-reset" title="复位">复位</button>
    </div>
    <div class="diagram-viewport">
        <div class="mermaid">
{mermaid_body}
        </div>
    </div>
</div>"#
    )
}

fn render_state_machine_tab(sm: &StateMachine, doc_model: &DocModel, program: &Program) -> String {
    use crate::mermaid::MermaidRenderable;

    let mermaid_body = crate::unfence_mermaid(&sm.to_mermaid());
    let title = match &sm.state_enum {
        Some(name) => format!("生命周期状态机：{}", html_escape(name)),
        None => "生命周期状态机".to_string(),
    };
    let frame_id = match &sm.state_enum {
        Some(name) => format!("state-machine-{}", crate::mermaid::sanitize_id(name)),
        None => "state-machine".to_string(),
    };
    let mut out = format!(
        "<div class=\"section\"><h2>{title}</h2>\
         <p class=\"section-desc\">由 require 前置状态 → ensure 后置状态自动推导；点击流转边或下方操作可查看完整契约。</p>"
    );
    if !sm.conflicts.is_empty() {
        out.push_str(
            "<div style=\"margin:12px 0;padding:12px 16px;border-left:4px solid #c62828;\
             background:#fdecea;border-radius:4px;\">\
             <strong style=\"color:#c62828;\">⚠ 状态机存在自相矛盾（结构级 V0020）</strong>\
             <ul style=\"margin:8px 0 0;padding-left:20px;\">",
        );
        for c in &sm.conflicts {
            out.push_str(&format!(
                "<li><code>{}</code> 无条件同时要求 status' == {}——不可同时成立，需澄清需求。</li>",
                html_escape(&c.intent),
                html_escape(&c.targets.join(" 且 status' == ")),
            ));
        }
        out.push_str("</ul></div>");
    }

    out.push_str(&diagram_frame(&frame_id, &mermaid_body));

    if sm.state_enum.is_none() {
        out.push_str("<p class=\"muted\">未检测到状态型流转（模型中没有 status 枚举的转换）。</p></div>");
        return out;
    }

    // Names that actually drive a lifecycle transition (creation included),
    // in declaration order — matches how a reader scans the .intent file.
    let mut names: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    for t in sm.transitions.iter().chain(sm.creation.iter()) {
        for n in t.label.split('/') {
            names.insert(n.trim());
        }
    }

    out.push_str("<h3 class=\"group-heading\">操作清单</h3>");
    out.push_str("<table class=\"data-table\"><thead><tr><th>操作</th><th>说明</th></tr></thead><tbody>");
    for decl in &program.declarations {
        let Declaration::Intent(i) = &decl.node else { continue };
        if !names.contains(i.name.as_str()) {
            continue;
        }
        let doc = doc_model
            .intents
            .get(&i.name)
            .and_then(|v| v.doc.clone())
            .unwrap_or_default();
        out.push_str(&format!(
            "<tr onclick=\"openPanel('intent','{name_attr}')\"><td class=\"name-cell\">{name}</td><td>{doc}</td></tr>",
            name_attr = html_escape_attr(&i.name),
            name = html_escape(&i.name),
            doc = html_escape(&doc),
        ));
    }
    out.push_str("</tbody></table></div>");
    out
}

fn render_flowchart_tab(mermaid_body: &str, sm: &StateMachine) -> String {
    let mut out = String::from(
        "<div class=\"section\"><h2>业务流程图</h2>\
         <p class=\"section-desc\">操作为方框、分支状态为判定菱形、起止为胶囊——与状态机同源，改需求自动更新。</p>",
    );

    if !sm.conflicts.is_empty() {
        out.push_str(
            "<div style=\"margin:12px 0;padding:12px 16px;border-left:4px solid #c62828;\
             background:#fdecea;border-radius:4px;\">\
             <strong style=\"color:#c62828;\">⚠ 流程存在自相矛盾（结构级 V0020）</strong>\
             <ul style=\"margin:8px 0 0;padding-left:20px;\">",
        );
        for c in &sm.conflicts {
            out.push_str(&format!(
                "<li><code>{}</code> 无条件同时要求 status' == {}——不可同时成立，需澄清需求。</li>",
                html_escape(&c.intent),
                html_escape(&c.targets.join(" 且 status' == ")),
            ));
        }
        out.push_str("</ul></div>");
    }

    out.push_str(
        "<div class=\"legend\">\
        <div class=\"legend-item\"><div class=\"legend-box\" style=\"background:#f3e5f5;border:1px solid #4a148c;\"></div><span>操作</span></div>\
        <div class=\"legend-item\"><div class=\"legend-box\" style=\"background:#fff8e1;border:2px solid #f57f17;\"></div><span>判定（分支状态）</span></div>\
        <div class=\"legend-item\"><div class=\"legend-box\" style=\"background:#eceff1;border:2px solid #37474f;\"></div><span>起止/终态</span></div>\
        <div class=\"legend-item\"><div class=\"legend-box\" style=\"background:#fdecea;border:2px solid #c62828;\"></div><span>冲突操作</span></div>\
        </div>",
    );
    out.push_str(&diagram_frame("flowchart", mermaid_body));
    out.push_str("</div>");
    out
}

fn render_goal_graph_tab(mermaid_body: &str) -> String {
    let mut out = String::from(
        "<div class=\"section\"><h2>目标追溯图</h2>\
         <p class=\"section-desc\">展示业务目标如何被安全规则与操作意图落地；点击任意节点查看完整依据。</p>",
    );
    out.push_str(
        "<div class=\"legend\">\
        <div class=\"legend-item\"><div class=\"legend-box\" style=\"background:#e8f5e9;border:2px solid #1b5e20;\"></div><span>能力目标</span></div>\
        <div class=\"legend-item\"><div class=\"legend-box\" style=\"background:#fff3e0;border:2px solid #e65100;\"></div><span>护栏目标</span></div>\
        <div class=\"legend-item\"><div class=\"legend-box\" style=\"background:#fbe9e7;border:1px solid #bf360c;\"></div><span>安全规则</span></div>\
        <div class=\"legend-item\"><div class=\"legend-box\" style=\"background:#f3e5f5;border:2px solid #4a148c;\"></div><span>操作意图</span></div>\
        </div>",
    );
    out.push_str(&diagram_frame("goal-graph", mermaid_body));
    out.push_str("</div>");
    out
}

fn render_safety_tab(graph: &GoalGraph, doc_model: &DocModel) -> String {
    let mut out = String::from(
        "<div class=\"section\"><h2>安全规则清单</h2>\
         <p class=\"section-desc\">按目标分组的不变量核对表——审计场景要逐条核对，表格比关系图更适合打勾。点击一行查看完整不变量。</p>",
    );

    let row = |name: &str, doc_model: &DocModel| -> String {
        let Some(v) = doc_model.safety.get(name) else { return String::new() };
        let params = v.params.join(", ");
        let invariant_preview = v.invariants.join("；  ");
        format!(
            "<tr onclick=\"openPanel('safety','{name_attr}')\"><td class=\"name-cell\">{name}</td><td>{params}</td><td>{inv}</td></tr>",
            name_attr = html_escape_attr(name),
            name = html_escape(name),
            params = html_escape(&params),
            inv = html_escape(&invariant_preview),
        )
    };
    let table_head =
        "<table class=\"data-table\"><thead><tr><th>规则</th><th>参数</th><th>不变量</th></tr></thead><tbody>";

    if graph.clusters.is_empty() {
        let mut rows = String::new();
        for name in doc_model.safety.keys() {
            rows.push_str(&row(name, doc_model));
        }
        if rows.is_empty() {
            out.push_str("<p class=\"muted\">未定义安全规则。</p></div>");
            return out;
        }
        out.push_str(table_head);
        out.push_str(&rows);
        out.push_str("</tbody></table></div>");
        return out;
    }

    let node_type: std::collections::HashMap<&str, &NodeType> =
        graph.nodes.iter().map(|n| (n.id.as_str(), &n.node_type)).collect();

    let mut any = false;
    for cluster in &graph.clusters {
        let names: Vec<&str> = cluster
            .node_ids
            .iter()
            .filter(|id| matches!(node_type.get(id.as_str()), Some(NodeType::Safety)))
            .map(|s| s.as_str())
            .collect();
        if names.is_empty() {
            continue;
        }
        any = true;
        out.push_str(&format!("<h3 class=\"group-heading\">{}</h3>", html_escape(&cluster.title)));
        out.push_str(table_head);
        for name in names {
            out.push_str(&row(name, doc_model));
        }
        out.push_str("</tbody></table>");
    }
    if !any {
        out.push_str("<p class=\"muted\">未定义安全规则。</p>");
    }
    out.push_str("</div>");
    out
}

fn render_coverage_tab(matrices: &[crate::coverage_matrix::CoverageMatrix]) -> String {
    let mut out = String::from(
        "<div class=\"section\"><h2>覆盖场景备忘</h2>\
         <p class=\"section-desc\">列出 <code>coverage</code> 声明的维度组合，供评审时人工核对是否遗漏——\
         不是已验证的测试覆盖率（蕴含式规则天然不会逐组合点名，真实验收证据见 <code>intent testspec</code> / <code>intent accept</code>）。</p>",
    );
    if matrices.is_empty() {
        out.push_str("<p class=\"muted\">未定义 coverage 场景。</p></div>");
        return out;
    }
    for (idx, matrix) in matrices.iter().enumerate() {
        out.push_str(&crate::coverage_matrix::render_html_grid(matrix, idx));
    }
    out.push_str("</div>");
    out
}
