/// Goal dependency graph builder
///
/// Builds a graph showing:
/// - Goals at the top level
/// - Safety rules, Intents, and Theorems that realize each goal
/// - Cross-references between declarations

use intent_lang_syntax::ast::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Serialize, Deserialize)]
pub struct GoalGraph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    /// Theme clusters for subgraph rendering. Empty = render flat (no goal
    /// carried a `@capability`/`@guardrail("group")` annotation).
    #[serde(default)]
    pub clusters: Vec<Cluster>,
}

/// A subgraph cluster: an ordered box of node ids sharing a theme group.
#[derive(Debug, Serialize, Deserialize)]
pub struct Cluster {
    pub title: String,
    pub node_ids: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub label: String,
    pub node_type: NodeType,
    pub metadata: NodeMetadata,
    /// For goal nodes: whether this is a positive capability or a guardrail.
    #[serde(default)]
    pub goal_kind: Option<GoalKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GoalKind {
    Capability,
    Guardrail,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum NodeType {
    Goal,
    Safety,
    Intent,
    Theorem,
    Axiom,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NodeMetadata {
    pub rationale: Option<String>,
    pub stakeholders: Vec<String>,
    pub annotations: Vec<String>,
    /// Human-authored one-line description from `@doc("...")`, shown as a
    /// legend / tooltip so opaque node names (e.g. `CreateTicketSoftReview`)
    /// carry meaning. Not verified — supplementary prose.
    #[serde(default)]
    pub doc: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Edge {
    pub from: String,
    pub to: String,
    pub edge_type: EdgeType,
    pub label: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum EdgeType {
    Realizes,      // Goal → Safety/Intent/Theorem
    Validates,     // Theorem → Intent
    Enforces,      // Safety → Intent
    References,    // Generic reference
}

/// Special cluster titles for nodes that don't map to a single theme group.
const SHARED_CLUSTER: &str = "跨主题共享";
const UNCLAIMED_CLUSTER: &str = "未被 goal 认领";
const UNGROUPED_CLUSTER: &str = "未分组";

/// Parsed `@capability("group")` / `@guardrail("group")` marker on a goal.
pub(crate) struct GoalMark {
    pub(crate) kind: GoalKind,
    pub(crate) group: Option<String>,
}

/// Extract the `@doc("...")` one-line description from a set of annotations.
pub(crate) use intent_lang_syntax::structure::doc_of;

/// Read the capability/guardrail annotation off a goal, if any.
pub(crate) fn goal_mark(goal: &GoalDecl) -> Option<GoalMark> {
    for ann in &goal.annotations {
        let kind = match ann.name.as_str() {
            "capability" => GoalKind::Capability,
            "guardrail" => GoalKind::Guardrail,
            _ => continue,
        };
        // First positional string arg = theme group name.
        let group = ann.args.iter().find_map(|a| match a {
            AnnotationArg::Positional(e) => match &e.node {
                Expr::StringLit(s) => Some(s.clone()),
                _ => None,
            },
            _ => None,
        });
        return Some(GoalMark { kind, group });
    }
    None
}

pub fn build_goal_graph(program: &Program) -> GoalGraph {
    // ── Index realizable declarations (goal graph excludes theorem/axiom) ──
    let mut realizer_kind: HashMap<&str, NodeType> = HashMap::new();
    for decl in &program.declarations {
        match &decl.node {
            Declaration::Safety(s) => {
                realizer_kind.insert(s.name.as_str(), NodeType::Safety);
            }
            Declaration::Intent(i) => {
                realizer_kind.insert(i.name.as_str(), NodeType::Intent);
            }
            _ => {}
        }
    }

    // ── Pass over goals: collect marks + per-realizer group references ──
    struct GoalInfo<'a> {
        name: &'a str,
        mark: Option<GoalMark>,
        rationale: Option<String>,
        stakeholder: Vec<String>,
        doc: Option<String>,
        realized_by: &'a [String],
    }
    let mut goals: Vec<GoalInfo> = Vec::new();
    for decl in &program.declarations {
        if let Declaration::Goal(g) = &decl.node {
            goals.push(GoalInfo {
                name: &g.name,
                mark: goal_mark(g),
                rationale: g.rationale.clone(),
                stakeholder: g.stakeholder.clone(),
                doc: doc_of(&g.annotations),
                realized_by: &g.realized_by,
            });
        }
    }

    let grouping_active = goals
        .iter()
        .any(|g| g.mark.as_ref().and_then(|m| m.group.as_ref()).is_some());

    // Membership: for each realizer, the distinct capability groups and the
    // first guardrail group (declaration order) that reference it.
    let mut cap_groups: HashMap<&str, Vec<String>> = HashMap::new();
    let mut first_guard_group: HashMap<&str, String> = HashMap::new();
    for g in &goals {
        let Some(mark) = &g.mark else { continue };
        let Some(group) = &mark.group else { continue };
        for r in g.realized_by {
            if !realizer_kind.contains_key(r.as_str()) {
                continue;
            }
            match mark.kind {
                GoalKind::Capability => {
                    let e = cap_groups.entry(r.as_str()).or_default();
                    if !e.contains(group) {
                        e.push(group.clone());
                    }
                }
                GoalKind::Guardrail => {
                    first_guard_group
                        .entry(r.as_str())
                        .or_insert_with(|| group.clone());
                }
            }
        }
    }

    // Resolve each realizer's home cluster.
    let realizer_home = |name: &str| -> String {
        match cap_groups.get(name) {
            Some(gs) if gs.len() >= 2 => SHARED_CLUSTER.to_string(),
            Some(gs) if gs.len() == 1 => gs[0].clone(),
            _ => match first_guard_group.get(name) {
                Some(g) => g.clone(),
                None => UNCLAIMED_CLUSTER.to_string(),
            },
        }
    };

    // ── Build nodes + edges, and record cluster membership in order ──
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut cluster_order: Vec<String> = Vec::new();
    let mut cluster_members: HashMap<String, Vec<String>> = HashMap::new();
    let mut assigned: HashSet<String> = HashSet::new();

    let push_member = |order: &mut Vec<String>,
                           members: &mut HashMap<String, Vec<String>>,
                           title: String,
                           id: String| {
        if !order.contains(&title) {
            order.push(title.clone());
        }
        members.entry(title).or_default().push(id);
    };

    // Goal nodes first (cluster = own group; kind drives color).
    for g in &goals {
        let goal_kind = g.mark.as_ref().map(|m| m.kind);
        nodes.push(Node {
            id: g.name.to_string(),
            label: g.name.to_string(),
            node_type: NodeType::Goal,
            metadata: NodeMetadata {
                rationale: g.rationale.clone(),
                stakeholders: g.stakeholder.clone(),
                annotations: vec![],
                doc: g.doc.clone(),
            },
            goal_kind,
        });
        if grouping_active {
            let title = g
                .mark
                .as_ref()
                .and_then(|m| m.group.clone())
                .unwrap_or_else(|| UNGROUPED_CLUSTER.to_string());
            push_member(&mut cluster_order, &mut cluster_members, title, g.name.to_string());
        }
        for r in g.realized_by {
            if realizer_kind.contains_key(r.as_str()) {
                edges.push(Edge {
                    from: g.name.to_string(),
                    to: r.clone(),
                    edge_type: EdgeType::Realizes,
                    label: None,
                });
            }
        }
    }

    // Realizer nodes (intent/safety) in declaration order.
    for decl in &program.declarations {
        let (id, node_type, annotations, doc) = match &decl.node {
            Declaration::Safety(s) => (s.name.clone(), NodeType::Safety, vec![], None),
            Declaration::Intent(i) => (
                i.name.clone(),
                NodeType::Intent,
                i.annotations.iter().map(|a| a.name.clone()).collect(),
                doc_of(&i.annotations),
            ),
            _ => continue,
        };
        if !assigned.insert(id.clone()) {
            continue;
        }
        nodes.push(Node {
            id: id.clone(),
            label: id.clone(),
            node_type,
            metadata: NodeMetadata {
                rationale: None,
                stakeholders: vec![],
                annotations,
                doc,
            },
            goal_kind: None,
        });
        if grouping_active {
            let title = realizer_home(&id);
            push_member(&mut cluster_order, &mut cluster_members, title, id);
        }
    }

    // Order clusters: theme groups (first-appearance) then the three specials.
    let clusters = if grouping_active {
        let specials = [SHARED_CLUSTER, UNGROUPED_CLUSTER, UNCLAIMED_CLUSTER];
        let mut ordered: Vec<String> = cluster_order
            .iter()
            .filter(|t| !specials.contains(&t.as_str()))
            .cloned()
            .collect();
        for s in specials {
            if cluster_order.iter().any(|t| t == s) {
                ordered.push(s.to_string());
            }
        }
        ordered
            .into_iter()
            .filter_map(|title| {
                cluster_members.remove(&title).map(|node_ids| Cluster { title, node_ids })
            })
            .collect()
    } else {
        Vec::new()
    };

    GoalGraph { nodes, edges, clusters }
}

pub fn build_safety_network(program: &Program) -> GoalGraph {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut type_nodes: HashSet<String> = HashSet::new();

    // Collect all type definitions
    for decl in &program.declarations {
        if let Declaration::Type(type_decl) = &decl.node {
            type_nodes.insert(type_decl.name.clone());

            nodes.push(Node {
                id: format!("type_{}", type_decl.name),
                label: type_decl.name.clone(),
                node_type: NodeType::Safety,
                metadata: NodeMetadata {
                    rationale: Some("Domain type".to_string()),
                    stakeholders: vec![],
                    annotations: vec![],
                    doc: None,
                },
                goal_kind: None,
            });
        }
    }

    // Process safety rules
    for decl in &program.declarations {
        if let Declaration::Safety(safety) = &decl.node {
            nodes.push(Node {
                id: safety.name.clone(),
                label: safety.name.clone(),
                node_type: NodeType::Safety,
                metadata: NodeMetadata {
                    rationale: Some(format!("{} rules", safety.invariants.len())),
                    stakeholders: vec![],
                    annotations: vec![],
                    doc: None,
                },
                goal_kind: None,
            });

            // Link safety rules to the types they constrain
            for param in &safety.params {
                if let TypeExpr::Named(type_name) = &param.ty {
                    if type_nodes.contains(type_name) {
                        edges.push(Edge {
                            from: safety.name.clone(),
                            to: format!("type_{}", type_name),
                            edge_type: EdgeType::Enforces,
                            label: Some(format!("param: {}", param.name)),
                        });
                    }
                }
            }
        }
    }

    GoalGraph { nodes, edges, clusters: Vec::new() }
}

impl crate::GraphData for GoalGraph {
    fn to_json(&self) -> anyhow::Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(src: &str) -> Program {
        intent_lang_syntax::parse(src).expect("parse")
    }

    #[test]
    fn flat_when_no_group_annotation() {
        let src = r#"
            type A { x: Int }
            goal "g" { realized_by: [Op] }
            intent Op(a: A) { ensure a.x' == a.x + 1 }
        "#;
        let g = build_goal_graph(&parse(src));
        assert!(g.clusters.is_empty(), "no annotation → flat");
    }

    #[test]
    fn clusters_by_theme_shared_and_unclaimed() {
        let src = r#"
            type A { x: Int }
            @capability("闭环") goal "cap1" { realized_by: [Op1, Shared] }
            @capability("流转") goal "cap2" { realized_by: [Op2, Shared] }
            @guardrail("闭环") goal "gd1" { realized_by: [Op1] }
            intent Op1(a: A) { ensure a.x' == a.x + 1 }
            intent Op2(a: A) { ensure a.x' == a.x + 1 }
            intent Shared(a: A) { ensure a.x' == a.x + 1 }
            intent Lonely(a: A) { ensure a.x' == a.x + 1 }
        "#;
        let g = build_goal_graph(&parse(src));
        let find = |t: &str| {
            g.clusters
                .iter()
                .find(|c| c.title == t)
                .unwrap_or_else(|| panic!("cluster {t} missing"))
        };
        // Shared referenced by two capability groups → shared cluster.
        assert!(find(SHARED_CLUSTER).node_ids.contains(&"Shared".to_string()));
        // Op1: one capability group (闭环) wins over guardrail reference.
        assert!(find("闭环").node_ids.contains(&"Op1".to_string()));
        // Op2 belongs to its only capability group.
        assert!(find("流转").node_ids.contains(&"Op2".to_string()));
        // Lonely: no goal references it → unclaimed.
        assert!(find(UNCLAIMED_CLUSTER).node_ids.contains(&"Lonely".to_string()));
    }

    #[test]
    fn guardrail_only_node_goes_to_first_guardrail_group() {
        let src = r#"
            type A { x: Int }
            @guardrail("组甲") goal "g1" { realized_by: [OpG] }
            @guardrail("组乙") goal "g2" { realized_by: [OpG] }
            intent OpG(a: A) { ensure a.x' == a.x + 1 }
        "#;
        let g = build_goal_graph(&parse(src));
        let jia = g.clusters.iter().find(|c| c.title == "组甲").unwrap();
        assert!(jia.node_ids.contains(&"OpG".to_string()));
    }

    #[test]
    fn theorems_and_axioms_excluded() {
        let src = r#"
            type A { x: Int }
            @capability("g") goal "cap" { realized_by: [Op] }
            intent Op(a: A) { ensure a.x' == a.x + 1 }
            theorem T { forall a: A, a.x >= 0 }
        "#;
        let g = build_goal_graph(&parse(src));
        assert!(g
            .nodes
            .iter()
            .all(|n| !matches!(n.node_type, NodeType::Theorem | NodeType::Axiom)));
    }

    #[test]
    fn doc_annotation_captured_and_in_legend() {
        let src = r#"
            type A { x: Int }
            @capability("g") goal "cap" { realized_by: [Op] }
            @doc("客户建单主流程") intent Op(a: A) { ensure a.x' == a.x + 1 }
        "#;
        let g = build_goal_graph(&parse(src));
        let op = g.nodes.iter().find(|n| n.id == "Op").unwrap();
        assert_eq!(op.metadata.doc.as_deref(), Some("客户建单主流程"));

        let legend = crate::mermaid::goal_doc_legend(&g).expect("legend");
        assert!(legend.contains("Op"));
        assert!(legend.contains("客户建单主流程"));
    }

    #[test]
    fn no_doc_means_no_legend() {
        let src = r#"
            type A { x: Int }
            @capability("g") goal "cap" { realized_by: [Op] }
            intent Op(a: A) { ensure a.x' == a.x + 1 }
        "#;
        let g = build_goal_graph(&parse(src));
        assert!(crate::mermaid::goal_doc_legend(&g).is_none());
    }

    #[test]
    fn goal_kind_recorded_for_coloring() {
        let src = r#"
            type A { x: Int }
            @capability("g") goal "cap" { realized_by: [Op] }
            @guardrail("g") goal "gd" { realized_by: [Op] }
            intent Op(a: A) { ensure a.x' == a.x + 1 }
        "#;
        let g = build_goal_graph(&parse(src));
        let kind = |name: &str| {
            g.nodes
                .iter()
                .find(|n| n.id == name)
                .and_then(|n| n.goal_kind)
        };
        assert_eq!(kind("cap"), Some(GoalKind::Capability));
        assert_eq!(kind("gd"), Some(GoalKind::Guardrail));
    }
}
