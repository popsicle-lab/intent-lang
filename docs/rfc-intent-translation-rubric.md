# RFC: 自然语言需求到 intent 的忠实翻译 Rubric

Status: Draft
Created: 2026-07-17
Authors: intent-lang contributors
Related:
- `docs/lang/LLM.md`
- `docs/lang/POSITIONING.md`
- `docs/protocol/artifacts.md`
- `docs/rfc-modeling-integrity.md`
- `.agents/skills/write-intent/SKILL.md`

---

## 1. 摘要

本 RFC 定义一套评估“自然语言需求是否被忠实翻译为 intent-lang”的 Rubric、
工件协议与演进机制。

核心原则：

> **Z3 判断翻译后的规则彼此是否一致；Rubric 判断这些规则是否忠实来自原始需求。两者不能互相替代。**

因此：

- `intent check` 通过，不代表翻译忠实；
- `intent check` 失败，不代表翻译不忠实；
- 原始需求若包含矛盾，忠实翻译应保留矛盾并让工具暴露它；
- 为了让验证通过而删除规则、补充前置条件或选择某一种歧义解释，属于翻译失真；
- 忠实度与形式质量在同一报告中呈现，但使用独立命名空间，互不折算。

本 RFC 不要求标准化整份 PRD。每次评测只要求提供一份经过人工确认的、
结构化自然语言原子需求清单（requirement atoms），作为语义真值。

第一阶段 Rubric 仅用于辅助评审，不作为合并或发布的强制门禁。

---

## 2. 动机

intent-lang 已能机械检查：

- 语法与类型是否正确；
- intent 子句是否自相矛盾；
- 多条规则组合后是否违反 safety / invariant；
- example 是否与公式一致；
- 状态机是否存在结构问题；
- 哪些子句可进入后续 testspec 和 acceptance。

但这些能力都无法回答：

> 候选 `.intent` 是否准确、完整、无臆造地表达了原始需求？

以下候选可能全部通过 `intent check`，却仍然是不忠实翻译：

- 原文是 `balance >= amount`，候选写成 `balance > amount`；
- 漏掉一条必须拒绝的安全规则；
- 凭空添加每日转账上限；
- 把操作级授权条件写成全局 `safety`；
- 原文有两条冲突结论，候选只保留其中一条；
- 原文有歧义，候选在没有人工决策时自行选择一种解释。

反过来，原文若自相矛盾，候选把冲突完整翻译为两条并列子句，
`intent check` 应报告 `self-contradictory`。这是忠实翻译和正确诊断，
而不是翻译失败。

所以必须在机械验证之外，建立独立的语义忠实度评测面。

---

## 3. 目标

本 RFC 的目标是：

1. 为每次“需求 → `.intent`”翻译提供可复现的辅助评审。
2. 检测漏译、语义反转、无依据臆造、边界偏移、作用域变化等高风险错误。
3. 显式检查原始冲突和歧义是否被保留。
4. 为每个结论提供 requirement atom、intent clause 和文本证据。
5. 允许评委在证据不足时拒绝判定并升级人工。
6. 防止翻译模型同时担任自己的评委。
7. 通过固定回归集、不可变版本和人工裁决治理未来调整。
8. 保持 intent-lang 核心工具确定性、离线和模型供应商无关。

---

## 4. 非目标

本 RFC 不做以下事情：

1. 不把完整 PRD 标准化为固定模板。
2. 不定义“唯一正确”的标准答案 `.intent`。
3. 不证明自然语言与逻辑公式在数学上完全等价。
4. 不把 LLM Judge 嵌入 `intent check`。
5. 不用单一总分决定需求是否可合并。
6. 不让人工反馈直接在线更新评委。
7. 不用形式质量分修正或抵消忠实度结论。
8. 不替产品负责人解决原始需求中的歧义和冲突。
9. 第一阶段不将 Rubric 作为 CI 强制门禁。

---

## 5. 术语

### 5.1 原始需求（source requirement）

翻译任务收到的自然语言材料，可以来自 PRD、Issue、访谈结论、RFC 或其他文档。
原始需求保留为证据，但不直接作为自动评分时唯一的语义真值。

### 5.2 原子需求（requirement atom）

从原始需求中提取的一条最小、可独立判断是否被表达的自然语言承诺。
atom 经过人工确认后成为本次评测的语义真值。

### 5.3 候选翻译（candidate）

被评测的一个或多个 `.intent` 文件。

### 5.4 语义子句（semantic clause）

候选中会改变业务承诺的结构，包括但不限于：

- `require`
- `ensure`
- `invariant`
- `safety`
- `theorem`
- `axiom`
- `modifies` / frame 语义
- `example` 中的业务预期

注释、名称、`@doc` 不能单独作为语义覆盖证据。

### 5.5 评委（judge）

根据确认后的 atoms 与候选 clauses 进行证据映射和忠实度判定的独立模型。
生成候选翻译的模型或会话不能担任评委。

### 5.6 裁决者（adjudicator）

处理评委分歧、低置信度和领域语义问题的人类。通常由产品负责人、领域专家或
Rubric maintainer 担任。

---

## 6. 语义真值：人工确认的 atoms

### 6.1 为什么不能直接用原始 PRD

直接让评委自由阅读原始 PRD 会产生不可控变量：

- PRD 中常同时包含背景、目标、方案、规则和开放问题；
- 同一句自然语言可能存在多种合理解释；
- 翻译模型和评委模型可能以不同方式拆分语义；
- 章节变化会影响检索上下文；
- 评委可能把常识或领域惯例误当成需求原文。

因此，本 RFC 采用两阶段方法：

1. 独立模型或人类从原始需求中提取 atoms；
2. 产品负责人审核、增删、澄清并确认 atoms；
3. 候选翻译只依据确认后的 atoms 评测；
4. 原始文本继续作为每个 atom 的证据。

### 6.2 atoms 只做语义结构化

atoms 使用结构化自然语言，不提前写成 intent 公式。否则真值制作过程本身
就会完成一次形式化翻译，并可能把同一个建模错误复制到答案和评测中。

建议 schema：

```yaml
schema_version: 1.0.0
case_id: REQ-TRANSFER-001
source:
  kind: prd
  locator: docs/transfer-prd.md
  revision: "git:abc123"

atoms:
  - id: R1
    statement: 余额小于转账金额时必须拒绝转账
    modality: must
    scope: Transfer
    source_evidence:
      quote: "当账户余额不足时，系统必须拒绝转账"
      locator: docs/transfer-prd.md#insufficient-balance
    status: confirmed
    relations:
      conflicts_with: []
      depends_on: []

  - id: R2
    statement: 余额等于转账金额时允许继续转账
    modality: must
    scope: Transfer
    source_evidence:
      quote: "余额等于转账金额不属于余额不足"
      locator: "decision:2026-07-17-01"
    status: confirmed
    relations:
      conflicts_with: []
      depends_on: [R1]
```

### 6.3 atom 必需字段

- `id`：评测案例内稳定且唯一的 ID。
- `statement`：一条原子化自然语言承诺。
- `modality`：`must | must_not | may | should`。
- `scope`：适用操作、实体或能力。
- `source_evidence`：原文引文及定位信息。
- `status`：至少支持 `draft | confirmed | rejected`。
- `relations`：至少支持 `conflicts_with` 和 `depends_on`。

只有 `status: confirmed` 的 atoms 参与正式评测。

### 6.4 atom 质量要求

一个合格 atom 应：

- 只表达一个可独立核对的承诺；
- 保留原文中的条件、结果、否定、范围和模态；
- 不提前选择具体 intent 语法；
- 不补充原文没有的常识性规则；
- 能指出原始文本或人工决策证据；
- 与其他 atom 冲突时显式声明，而不是在 atom 提取阶段修复。

---

## 7. 双向证据映射

Rubric 采用 atoms 与 semantic clauses 的双向、多对多映射。

### 7.1 Atom → Clause

每个 confirmed atom 必须映射到零个或多个 clauses，用于检测：

- 完整覆盖；
- 部分覆盖；
- 语义偏移；
- 完全漏译。

一个 atom 可以由多条 clause 共同表达。例如“余额不足时拒绝且状态不变”
可能同时依赖 `require ... else reject` 和 frame 语义。

### 7.2 Clause → Atom

每条 semantic clause 必须映射到零个或多个 atoms，用于检测：

- 有充分需求依据的规则；
- 无依据臆造；
- 一个 clause 是否混合了多个不相关承诺。

### 7.3 证据要求

每个映射结论必须包含：

- atom ID；
- clause 稳定 ID 或源码范围；
- atom 中的文本证据；
- clause 中的公式证据；
- 判定理由；
- 置信度；
- 是否需要复评。

仅引用 `@doc`、名称或注释不能证明语义覆盖。

### 7.4 映射示例

```yaml
atom_mappings:
  - atom_id: R1
    clauses:
      - Transfer/funds
      - Transfer/reject_frame
    verdict: faithful
    confidence: 0.97
    rationale: require 的边界与 atom 一致，else reject 保证拒绝路径

clause_mappings:
  - clause_id: Transfer/daily_limit
    atoms: []
    verdict: unsupported
    confidence: 0.99
    rationale: confirmed atoms 中没有每日限额规则
```

---

## 8. 忠实度 Rubric

Rubric 不输出可抵消严重错误的单一总分。它输出分维度指标、逐项 finding 和
整体风险等级。

### 8.1 完整性（completeness）

检查每个 confirmed atom 是否被候选完整表达。

典型 finding：

- `omitted_rule`
- `partially_covered`
- `missing_condition`
- `missing_outcome`
- `missing_rejection_behavior`

### 8.2 语义准确性（faithfulness）

检查条件、结果、方向、状态、量词、模态和逻辑关系是否保持不变。

典型 finding：

- `semantic_reversal`
- `boundary_shift`
- `modality_change`
- `state_phase_confusion`
- `boolean_structure_change`
- `quantifier_scope_change`

### 8.3 无臆造（no invention / groundedness）

检查每条 semantic clause 是否有 confirmed atom 依据。

典型 finding：

- `unsupported_rule`
- `unsupported_precondition`
- `unsupported_outcome`
- `unsupported_global_invariant`

必要的纯技术结构若不改变业务承诺，可以标为 `structural`，但必须说明原因。

### 8.4 作用域保持（scope preservation）

检查局部操作规则、实体约束、角色权限和全局不变量是否处于正确作用域。

典型 finding：

- `scope_expansion`
- `scope_narrowing`
- `operation_to_global`
- `global_to_operation`
- `actor_scope_change`

### 8.5 冲突保持（conflict preservation）

若 atoms 声明冲突，候选应忠实表达冲突双方。

典型 finding：

- `conflict_suppression`
- `conflict_side_dropped`
- `conflict_hidden_by_precondition`
- `conflict_diagnostic_mismatch`

Rubric 不负责决定冲突哪一方正确。

### 8.6 歧义保持（ambiguity preservation）

若需求尚未确认一种解释，候选不得静默选择。

可接受处理包括：

- 明确列为待确认项，不进入正式承诺；
- 并列呈现候选解释并请求人工决定；
- 返回无法忠实形式化，而不是猜测。

典型 finding：

- `ambiguity_resolution_without_basis`
- `uncertainty_dropped`
- `open_question_treated_as_rule`

### 8.7 可追踪性（traceability）

检查 atom 与 clause 是否有稳定、可审计的映射。

该维度影响评审质量，但命名或映射格式问题本身通常不改变业务语义，
默认不应升级为 `critical`。

---

## 9. Finding 严重级别

### 9.1 Critical

以下类型默认是 `critical`，因为它们会改变需求承诺：

- `omitted_rule`：漏掉明确的 MUST、MUST NOT 或安全约束；
- `semantic_reversal`：允许/禁止、增加/减少、前态/后态等语义反转；
- `unsupported_rule`：凭空加入会改变业务行为的约束或结果；
- `boundary_shift`：边界变化导致合法行为集合改变；
- `scope_change`：局部与全局、角色或对象范围发生变化；
- `conflict_suppression`：删除、改写或补条件以隐藏原始冲突；
- `ambiguity_resolution_without_basis`：没有人工依据时擅自选择解释。

### 9.2 Major

`major` 表示明显降低精确性或可审计性，但尚不能证明核心业务承诺已经改变，
例如：

- atom 只被部分表达；
- 映射证据不足；
- 模态弱化但当前上下文无法确认是否改变执行行为；
- 多条规则被混入一个难以独立追踪的 clause；
- 拒绝原因或副作用表达不完整。

### 9.3 Minor

`minor` 只用于：

- 命名不清；
- `@doc` 不完整；
- clause ID 不稳定；
- 组织结构影响评审但不改变语义；
- 其他纯可读性问题。

### 9.4 严重级别约束

- 严重级别由是否改变需求承诺决定；
- 不能因为候选能通过 Z3 而降低忠实度 finding；
- 不能因为候选不能解析而自动把所有忠实度 finding 提升为 critical；
- 评委若无法证明严重级别，应拒绝判定或请求复评。

---

## 10. 整体结论

报告不提供单一总分，使用以下结论：

- `clean`：没有发现忠实度问题；
- `review`：存在 major/minor、歧义或需人工确认项；
- `high-risk`：至少存在一个 critical finding；
- `unevaluable`：语义真值、候选证据或领域知识不足，无法可靠评测。

分维度数值可以用于趋势观察，但不得用加权平均抵消 finding。

示例：

```yaml
fidelity:
  verdict: high-risk
  dimensions:
    completeness: 92
    faithfulness: 100
    no_invention: 85
    conflict_preservation: 100
    ambiguity_preservation: 80
  findings:
    - severity: critical
      type: omitted_rule
      atom_id: R7
```

---

## 11. 形式质量：独立命名空间

忠实度报告同时携带形式质量事实，但二者完全隔离：

```yaml
formal_quality:
  parse_status: valid
  typecheck_status: valid
  consistency_status: self-contradictory
  expected_consistency: contradictory
  diagnostic_match: true
  clause_ids_stable: true
  explicit_modifies: true
  testspec_status: generated
```

规则：

1. `intent check` 失败不降低忠实度。
2. `intent check` 通过不提高忠实度。
3. check 结果只与 atoms 中确认的冲突预期比较。
4. 无法解析属于形式不可用，但评委仍应尽可能评估可读源码中的语义。
5. 语言语法变化可以改变形式质量结果，不应自动改变历史忠实度结论。

### 11.1 预期诊断匹配

当 confirmed atoms 明确包含冲突时：

```yaml
expected_consistency: contradictory
actual_consistency: self-contradictory
diagnostic_match: true
```

如果候选为了通过 check 而隐藏冲突：

```yaml
expected_consistency: contradictory
actual_consistency: verified
diagnostic_match: false
```

此时应同时产生 `conflict_suppression` finding。

---

## 12. 评委协议

### 12.1 角色隔离

- 翻译模型 A 生成候选 intent；
- atoms 提取模型 B 只从原始需求提取 atoms；
- 人类确认 atoms；
- 主评委 C 评估 atoms 与候选；
- 条件复评模型 D 必须与当前翻译会话隔离；
- 人类裁决未解决的分歧。

模型可以属于同一模型家族，但不得共享生成候选时的隐藏推理或对话上下文。

### 12.2 主评委

主评委全量执行：

1. 读取 confirmed atoms；
2. 读取候选 AST/clauses 和必要源码；
3. 生成 Atom → Clause 映射；
4. 生成 Clause → Atom 映射；
5. 给出逐项 verdict、证据、置信度和 findings；
6. 标记需要复评或人工裁决的项目。

### 12.3 条件复评

以下情况触发独立复评：

- 主评委报告 critical finding；
- 置信度低于当前 Rubric 版本阈值；
- 主评委返回 `unevaluable`；
- 涉及复杂量词、定理间接实现或领域语义；
- 随机抽取的普通案例。

第一阶段建议随机复评约 10% 的非风险案例，用于监测评委漂移。

### 12.4 分歧处理

- 主评委和复评委一致：保留结论及双方证据；
- 两者不一致：不得平均分或多数表决；
- 分歧项进入人工裁决；
- 人工裁决必须记录角色、结论、理由和证据；
- 裁决结果不直接修改在线 Rubric。

### 12.5 拒绝判定

评委可以返回 `unevaluable`。典型原因：

- atom 本身仍有歧义；
- 需要外部领域知识；
- 复杂形式化结构无法可靠映射；
- 候选上下文不完整；
- 无法提供具体 atom/clause 证据。

`unevaluable` 既不算错误，也不算通过，而是明确的人工待办。

---

## 13. 工件协议

建议新增独立工件 `intent.translation_evaluation`。
它由外部 evaluator 产生，不是 `intent check` 的输出。

最小 JSON 形态：

```json
{
  "kind": "intent.translation_evaluation",
  "schema_version": "1.0.0",
  "case_id": "REQ-TRANSFER-001",
  "created_at": "2026-07-17T00:00:00+08:00",
  "environment": {
    "rubric_version": "1.0.0",
    "atoms_schema_version": "1.0.0",
    "judge_prompt_version": "1.0.0",
    "primary_judge": "provider/model@revision",
    "secondary_judge": null,
    "intent_lang_version": "0.x.y"
  },
  "fidelity": {
    "verdict": "high-risk",
    "dimensions": {
      "completeness": 92,
      "faithfulness": 100,
      "no_invention": 85,
      "scope_preservation": 100,
      "conflict_preservation": 100,
      "ambiguity_preservation": 80
    },
    "findings": [
      {
        "id": "F1",
        "severity": "critical",
        "type": "omitted_rule",
        "atom_ids": ["R7"],
        "clause_ids": [],
        "evidence": {
          "source": "R7 requires rejection for frozen receivers",
          "candidate": "no matching clause"
        },
        "confidence": 0.98,
        "review_status": "primary-only"
      }
    ]
  },
  "mappings": {
    "atom_to_clause": [],
    "clause_to_atom": []
  },
  "formal_quality": {
    "parse_status": "valid",
    "consistency_status": "verified",
    "expected_consistency": "unknown",
    "diagnostic_match": null
  },
  "manual_review": {
    "required": true,
    "items": ["F1"]
  }
}
```

### 13.1 工件设计约束

- 所有 findings 必须有稳定 ID；
- findings 必须引用 atom IDs 和 clause IDs；
- 无法引用 clause ID 时使用源码 span；
- 所有环境版本必须固定记录；
- 报告不得只保存自然语言总结；
- 历史报告不可被新版本原地覆盖；
- 新旧 Rubric 重跑结果应并列保存。

正式落地时，应把最终 schema 补入 `docs/protocol/artifacts.md`。

---

## 14. 工具架构

LLM Judge 不进入 intent-lang 核心 CLI。

推荐边界：

### 14.1 intent CLI

继续提供确定性事实：

- 解析和类型检查；
- AST/clauses 的稳定 JSON；
- consistency report；
- coverage/testspec 等机械工件；
- clause ID、源码 span 和生命周期信息。

若 evaluator 所需的 clause 导出尚不存在，可以新增确定性的 AST/IR 导出命令，
但该命令不得调用 LLM。

### 14.2 独立 evaluator

外层工具负责：

- 读取 atoms；
- 读取候选和 CLI 工件；
- 调用主评委和条件复评；
- 校验评委输出 schema；
- 汇总 findings 与人工待办；
- 写出 `intent.translation_evaluation`。

概念命令：

```bash
intent-eval translation \
  --atoms requirements/REQ-TRANSFER-001.atoms.yaml \
  --candidate intents/transfer.intent \
  --format json
```

### 14.3 Agent Skill

独立 Skill 负责指导：

- 从需求提取 atoms；
- 请求产品负责人逐项确认；
- 运行 evaluator；
- 解释 findings；
- 收集人工裁决；
- 明确禁止翻译会话自评。

该 Skill 与 `write-intent` 分离。`write-intent` 只负责翻译和机械验证，
不能修改 atoms 来迁就候选，也不能覆盖评委结论。

---

## 15. 运行流程

### 15.1 每次需求翻译

```text
原始需求
  │
  ├─ 独立提取 atoms
  │      └─ 产品负责人确认
  │
  └─ 翻译模型生成候选 .intent
          │
          ├─ intent CLI 输出机械事实
          │
          └─ 独立 evaluator
                 ├─ 主评委全量映射
                 ├─ 风险项/抽样项复评
                 └─ 人工处理分歧与 unevaluable
```

### 15.2 Shadow mode

第一阶段：

- Rubric 只输出辅助意见；
- 不阻止 PR 合并；
- 产品负责人可以确认或推翻 finding；
- 所有推翻必须附理由；
- 记录误报、漏报、分歧、成本和延迟；
- 不根据个别案例即时修改 prompt。

### 15.3 修正候选

候选作者可以根据 finding 修正 `.intent`，但：

- 不得直接修改 confirmed atom 来消除 finding；
- 若确认是 atom 错误，必须由产品负责人单独修订 atom；
- 修订 atom 后应记录需求真值版本变化；
- 修正候选后重新生成完整报告，旧报告保留。

---

## 16. 回归评测集

Rubric 的每个新版本必须在四类固定数据上评测。

### 16.1 Gold cases

人工确认过 atoms、映射和 findings 的真实案例。

用途：

- 测量与人工裁决的一致性；
- 覆盖真实语言和领域复杂度；
- 防止只针对合成错误优化。

### 16.2 Mutants

从忠实候选中注入一个或少量已知错误：

- 删除一条必须规则；
- 反转允许/禁止；
- 把 `<` 改为 `<=`；
- 去掉 `else reject`；
- 增加无来源上限；
- 把操作级授权改为全局 safety；
- 删除冲突的一方；
- 增加前置条件以隐藏冲突；
- 把 primed 后态写成前态；
- 扩大或缩小角色作用域。

mutant 必须记录注入点、预期 finding 类型和严重级别。

### 16.3 Disputes

历史上主评委、复评委或人工发生分歧的案例。

用途：

- 防止已解决问题复发；
- 检验 prompt 修改是否真正提高判定质量；
- 识别跨领域的系统性歧义。

### 16.4 Clean variants

同一组 atoms 的多种忠实形式化写法，例如：

- 等价布尔表达式；
- 合理拆分/合并 clauses；
- 辅助函数与内联表达式；
- `x'` 与 `after(x)`；
- 不同但等价的命名和结构组织。

用途是防止 Rubric 退化为“与某个标准答案做文本相似度比较”。

---

## 17. 调整与发布治理

### 17.1 不可变版本包

每份报告必须记录：

- `rubric_version`
- `atoms_schema_version`
- `judge_prompt_version`
- 主/复评委模型及 revision
- `intent_lang_version`
- evaluator 版本
- 回归集版本

已发布版本不可原地修改。

### 17.2 语义化版本

- major：改变 finding 语义、严重级别、总体 verdict 或兼容性；
- minor：新增检查项或向后兼容字段；
- patch：修正文案、格式或不改变判定语义的缺陷。

模型升级、prompt 修改、Rubric 修改和 schema 修改必须分别记录，
不能用一个模糊的“评测器更新”覆盖。

### 17.3 新版本发布条件

新版本必须满足：

- critical mutants 检出率不得下降；
- clean variants 误报率不得上升；
- 与人工 gold 裁决的一致性达到当前发布阈值；
- 同一输入重复评测的 verdict 与 critical findings 足够稳定；
- 新 finding 能提供 atom、clause 和具体证据；
- disputes 回归结果不出现未解释退化；
- 成本和延迟变化被明确记录。

第一版不在 RFC 中锁死具体百分比。应先通过真实 shadow 数据建立基线，
再在 Rubric 版本说明中设定发布阈值。

### 17.4 新旧版本并列重跑

调整 Rubric 时：

1. 固定同一输入、atoms 和候选；
2. 分别运行旧版与新版；
3. 生成结果差异；
4. 对新增、消失和严重级别变化的 findings 做人工抽查；
5. 发布版本说明；
6. 不覆盖历史报告。

---

## 18. 人工反馈治理

人工裁决不自动更新 Rubric。

建议流程：

1. 裁决者提交 `confirm | overturn | refine_atom | defer`；
2. 记录裁决者角色、理由和证据；
3. 案例进入候选反馈池；
4. Rubric maintainer 定期审核；
5. 合格案例进入 gold 或 disputes；
6. 统一提出新 Rubric/prompt 版本；
7. 运行完整回归集；
8. 通过发布条件后才升级。

禁止：

- 针对单个失败案例直接给 prompt 追加特例；
- 未经审核自动训练或在线学习；
- 为迁就候选翻译而修改 confirmed atoms；
- 用产品偏好推翻逻辑证据而不记录原因；
- 只保留推翻结果、不保留原评委输出。

---

## 19. 监测指标

Shadow 阶段至少监测：

- critical finding 的人工确认率；
- critical 错误漏检率；
- clean candidate 误报率；
- 主评委与复评委一致率；
- 评委与人工裁决一致率；
- `unevaluable` 比例及原因分布；
- 同一输入重复评测的稳定性；
- atom 覆盖率；
- clause groundedness；
- 每次评测成本与延迟；
- 人工处理时间；
- 按领域拆分的指标，防止总体数据掩盖领域退化。

不能只优化“与人工一致率”。人工裁决本身也可能错误，因此 gold 变更必须保留审计记录。

---

## 20. 隐私与供应商边界

独立 evaluator 必须支持数据策略配置：

- 是否允许把原始需求发送到外部模型；
- 是否只发送 confirmed atoms 和最小候选片段；
- 是否要求自托管评委；
- 日志是否包含原始业务文本；
- 报告中的证据是否需要脱敏；
- 模型供应商是否允许保留或训练数据。

数据策略不得静默改变。相关配置和模型供应商应进入评测环境记录。

---

## 21. 第一阶段 Dogfood

### 21.1 主案例

使用 ticket 需求作为主案例，因为它包含：

- 较完整的叙事 PRD；
- 状态机；
- 权限与角色；
- 正常路径和拒绝路径；
- SLA/边界规则；
- 已有 `ticket.intent`。

当前 `ticket.intent` 只是候选，不自动视为标准答案。

### 21.2 初始数据规模

建议：

- 人工确认 30–50 个 ticket atoms；
- 构造至少 20 个单点 mutants；
- 构造 5–10 个 clean variants；
- 从 smarthome、billing、transfer 各抽取少量案例；
- 对所有 critical finding 做条件复评；
- 对普通案例随机复评约 10%。

### 21.3 成功标准

第一阶段的目标不是得到一个漂亮总分，而是回答：

- atoms 能否被产品负责人低成本确认；
- 评委能否稳定给出双向证据映射；
- critical mutants 是否被可靠检出；
- clean variants 是否很少误报；
- 哪些领域语义经常触发 `unevaluable`；
- 人工裁决成本是否可接受；
- 报告是否真正帮助作者发现翻译失真。

---

## 22. 分阶段路线图

### Phase 0：协议与样例

- 确认 atoms YAML schema；
- 确认 evaluation JSON schema；
- 编写主评委和复评委固定 prompt；
- 从 ticket 制作首批人工确认 atoms；
- 建立 mutants 和 clean variants。

### Phase 1：Shadow evaluator

- 实现独立 `intent-eval` 原型或 Agent Skill；
- 接入现有 CLI JSON 工件；
- 输出双向映射、findings 和人工待办；
- 不接 CI 强制门禁；
- 收集 30–50 个真实翻译案例。

### Phase 2：校准与版本化

- 建立 gold/disputes 数据集；
- 固定 Rubric v1.0.0；
- 测量稳定性、误报、漏报、成本和领域差异；
- 把 artifact schema 正式加入 `docs/protocol/artifacts.md`；
- 为历史报告提供新旧版本并列重跑工具。

### Phase 3：产品化

- 在 PR 或 IDE 中展示 atom ↔ clause 映射；
- 支持 finding 的人工确认/推翻工作流；
- 支持最小上下文与隐私策略；
- 提供趋势面板，但不展示误导性的单一总分。

### Phase 4：可选门禁

只有在长期 shadow 数据证明稳定后，才讨论把少量 high-confidence critical findings
升级为门禁。是否启用门禁必须由新的 RFC 或本 RFC 的 major 版本决定。

---

## 23. 风险与缓解

### 23.1 评委把常识当需求

缓解：Clause → Atom 反向映射必须引用 confirmed atom；无证据即 unsupported 或 abstain。

### 23.2 Rubric 只认一种写法

缓解：维护 clean variants；禁止使用标准答案文本相似度作为主要判据。

### 23.3 人工确认 atoms 成本过高

缓解：模型先提取，人类只审核；按能力分批；测量每个 atom 的确认耗时。

### 23.4 翻译者修改真值迁就候选

缓解：atoms 与候选分开版本控制；atom 修改必须由产品负责人审批并保留历史。

### 23.5 同一模型家族造成关联偏差

缓解：角色和会话隔离；风险项使用独立复评；维护人工 gold 与 mutants。

### 23.6 Prompt 越修越复杂

缓解：所有修改版本化；先进入 disputes；完整回归后发布，禁止在线打补丁。

### 23.7 数值指标产生虚假安全感

缓解：不设单一总分；critical finding 不可被其他维度抵消；报告必须展示证据。

### 23.8 原始需求本身错误

缓解：忠实翻译保留错误、冲突和歧义；Rubric 不把“合理化需求”当作翻译质量。

---

## 24. 待决问题

以下问题留待 dogfood 数据回答：

1. atoms 的 `modality` 是否需要扩展为领域自定义枚举？
2. 哪些纯结构性 clauses 可以免做 Clause → Atom 映射？
3. 复杂 theorem 间接实现 atom 时，证据路径如何表示？
4. 主评委的低置信度复评阈值应是多少？
5. 普通案例随机复评比例是否长期保持 10%？
6. 报告中的维度数值是否保留，还是只保留离散 verdict？
7. 同一 atom 跨多个 intent 文件实现时，评测上下文如何最小化？
8. 何种人工一致性和 mutant 检出水平足以讨论门禁？
9. atoms 与原始 PRD 的稳定定位是否另立 source-trace RFC？
10. evaluation artifact 最终由本仓库维护，还是由外部工具维护？

---

## 25. 决策摘要

本 RFC 选择：

- 人工确认的结构化自然语言 atoms 作为语义真值；
- 不维护标准答案公式；
- 忠实度与形式质量隔离；
- atoms ↔ clauses 双向多对多证据映射；
- 辅助评审优先；
- 主评委全量、风险项和抽样项独立复评；
- 分维度结果与严重级别，不设单一总分；
- 允许 `unevaluable`；
- 固定 critical 失真类型；
- gold、mutants、disputes、clean variants 四类回归集；
- 不可变版本包与新旧版本并列重跑；
- 语言核心提供确定性事实，外部 evaluator 负责 LLM 评测；
- 人工反馈经审核和完整回归后才进入新版；
- ticket 为首个主 dogfood 案例，并加入跨领域样本。

最终目标不是证明候选“看起来合理”，而是建立一条可审计的证据链：

```text
原始需求证据
  → 人工确认 atom
  → intent semantic clause
  → 忠实度 finding / verdict
  → 独立形式质量与 Z3 诊断
```

只有把“翻译忠实”和“逻辑一致”分开评估，intent-lang 才能既忠实暴露需求问题，
又避免形式验证结果被误解为自然语言翻译正确性的证明。
