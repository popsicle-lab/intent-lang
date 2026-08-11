# intent-lang 产品能力 功能点事实

## Meta
- domain: intent-lang 产品能力
- domain_abbrev: IL
- pinned: intent-lang@f8cded686928a80dad963c1813e9a47caa825fc7
- extracted_at: 2026-08-11
- reextracted_at: 2026-08-11
- skill_version: extract-facts/0.2.0
- aligned_with: intent-lang.intent（SSOT 合流 v2）
- tools: rg ✓, cargo test ✓ [运行验证过]

## 能力清单
| 能力 | 用户价值 | 主要入口 |
| 需求一致性 | 形式化需求经 Z3 验证无逻辑矛盾 | `intent check` |
| 结构完备性 | goal/生命周期/ example 结构可被检测 | `intent check --strict` |
| 需求基线 | 存量事实 → @asis intent → trace 防漏译 | extract-facts / write-intent / `intent trace` |
| 可执行验收 | testspec + binding → pytest + clause 报告 | `intent accept gen/run` |
| 变更分析 | coverage/diff/impact 纯 AST 分析 | `intent coverage` 等 |
| 已知限制 | 引擎未实现或绑定限制如实入库 | vcgen / binding |

## 术语表
| 术语 | 含义（来自文档/注释） |
| ModelSpec | 用户待验证的 .intent 需求模型（对应 intent-lang.intent 中类型） |
| FactsBaseline | extract-facts 产出并经人工确认的事实基线 |
| VerificationPhase | check 管线阶段：Unverified → … → Verified / Failed |
| BaselinePhase | 基线阶段：Draft → Confirmed → Formalized → Accepted |

## 实体与状态
- ModelSpec：字段 phase ∈ VerificationPhase，structurallySound / logicallyConsistent: Bool（intent-lang.intent）
- FactsBaseline：phase ∈ BaselinePhase，traceClean: Bool（intent-lang.intent）
- VerificationPhase：Unverified, Parsed, TypeChecked, StructureChecked, Verified, Failed（@f8cded68:crates/intent-lang-cli/src/main.rs#L268-L634 管线顺序）

## 状态流转
| 源态 | 操作 | 次态 | 锚点 |
| （无） | LoadModelSpec | Unverified | @f8cded68:crates/intent-lang-cli/src/main.rs#L276-L317 |
| Unverified | ParseModelSpec | Parsed | @f8cded68:crates/intent-lang-cli/src/main.rs#L283-L317 |
| Parsed | TypeCheckModelSpec | TypeChecked | @f8cded68:crates/intent-lang-cli/src/main.rs#L319-L337 |
| TypeChecked | CheckStructureModelSpec | StructureChecked | @f8cded68:crates/intent-lang-cli/src/main.rs#L339-L349 |
| StructureChecked | VerifyModelSpec | Verified | @f8cded68:crates/intent-lang-cli/src/main.rs#L365-L534 |
| StructureChecked | RejectVacuousRules | Failed | @f8cded68:crates/intent-lang-core/src/smt.rs#L660-L664 |
| （无） | StartBaseline | Draft | extract-facts 技能产出 status: draft |
| Draft | ConfirmFacts | Confirmed | 人工确认关口 |
| Confirmed | FormalizeBaseline | Formalized | write-intent 产出 @asis 子句 |
| Formalized | GenerateAcceptance | Accepted | @f8cded68:crates/intent-lang-cli/src/main.rs#L1010-L1071 |

## 操作

### 操作：LoadModelSpec
- source: @f8cded68:crates/intent-lang-cli/src/main.rs#L268-L281
- 职责：载入待验证需求文件，启动 check 管线
- example 候选: `examples/basics/transfer.intent` 可读且进入 check（[运行验证过]）

#### 前置检查
- fact_id: F-IL-BEH-001
  statement: 源文件无法读取时打印 error 并以 exit 1 终止，不进入后续阶段
  modality: must
  status: confirmed
  source: @f8cded68:crates/intent-lang-cli/src/main.rs#L161-L172
  evidence: `read_file` 失败 → `process::exit(1)`

#### 状态效果
- fact_id: F-IL-BEH-002
  statement: 成功读取后进入 parse 阶段（对应 VerificationPhase 从 Unverified 向前）
  modality: must
  status: confirmed
  source: @f8cded68:crates/intent-lang-cli/src/main.rs#L276-L317
  evidence: read 成功后调用 `parse(&source)`

### 操作：ParseModelSpec
- source: @f8cded68:crates/intent-lang-cli/src/main.rs#L283-L317
- 职责：将 .intent 源文本解析为 AST

#### 前置检查
- fact_id: F-IL-BEH-003
  statement: 解析失败时打印带位置的错误并以 exit 1 终止
  modality: must
  status: confirmed
  source: @f8cded68:crates/intent-lang-cli/src/main.rs#L283-L316
  evidence: parse Err → exit 1

#### 状态效果
- fact_id: F-IL-BEH-004
  statement: 解析成功后进入 typecheck 阶段
  modality: must
  status: confirmed
  source: @f8cded68:crates/intent-lang-cli/src/main.rs#L319-L324
  evidence: parse Ok 后继续 `check_program(&prog)`

### 操作：TypeCheckModelSpec
- source: @f8cded68:crates/intent-lang-cli/src/main.rs#L319-L337
- 职责：类型检查，Error 级诊断阻断后续验证

#### 状态效果
- fact_id: F-IL-BEH-005
  statement: 类型检查产生 Error 级诊断时跳过后续验证并以 exit 1 终止
  modality: must
  status: confirmed
  source: @f8cded68:crates/intent-lang-cli/src/main.rs#L319-L337
  evidence: `has_errors` → exit 1

### 操作：CheckStructureModelSpec
- source: @f8cded68:crates/intent-lang-core/src/structure.rs#L57-L143
- 职责：结构门控 S0001–S0008，在 Z3 之前运行

#### 前置检查
- fact_id: F-IL-BEH-006
  statement: 结构检查在 typecheck 通过之后、Z3 验证之前运行
  modality: must
  status: confirmed
  source: @f8cded68:crates/intent-lang-cli/src/main.rs#L339-L365
  evidence: check_structure 位于 typecheck 与 generate_vcs 之间

#### 状态效果
- fact_id: F-IL-BEH-007
  statement: S0004 与 S0007 无条件为 Error；其余结构发现在非 strict 下为 Warning
  modality: must
  status: confirmed
  source: @f8cded68:crates/intent-lang-core/src/structure.rs#L12-L29
  evidence: 模块 severity policy

- fact_id: F-IL-BEH-008
  statement: 结构 Error 级诊断导致 intent check exit 1；仅 Warning 时不单独因此 exit 1
  modality: must
  status: confirmed
  source: @f8cded68:crates/intent-lang-cli/src/main.rs#L344-L349
  evidence: `structure_failed` 仅计 Error 级

### 操作：VerifyModelSpec
- source: @f8cded68:crates/intent-lang-cli/src/main.rs#L365-L534
- 职责：对 intent/theorem 生成 VC 并调用 Z3

#### 状态效果
- fact_id: F-IL-BEH-009
  statement: 默认跳过带 @asis 注解的 intent VC，除非传入 --include-asis
  modality: must
  status: confirmed
  source: @f8cded68:crates/intent-lang-cli/src/main.rs#L351-L397
  evidence: `is_asis && !include_asis` 分支 skip

- fact_id: F-IL-BEH-010
  statement: VC 标记 unsupported 时跳过 Z3 验证
  modality: must
  status: confirmed
  source: @f8cded68:crates/intent-lang-cli/src/main.rs#L422-L441
  evidence: `vc.unsupported` 分支

- fact_id: F-IL-BEH-011
  statement: 全部 VC 与 example 通过且结构无 Error 时 check exit 0
  modality: must
  status: confirmed
  source: @f8cded68:crates/intent-lang-cli/src/main.rs#L618-L633
  evidence: `ok = all_ok && !structure_failed`

### 操作：RejectVacuousRules
- source: @f8cded68:crates/intent-lang-core/src/smt.rs#L648-L664
- 职责：V0020 反 vacuity

#### 状态效果
- fact_id: F-IL-BEH-012
  statement: intent 的 assumes 不可满足时，结果报告为 SelfContradictory 而非 Verified
  modality: must
  status: confirmed
  source: @f8cded68:crates/intent-lang-core/src/smt.rs#L660-L664
  evidence: `SatOutcome::Unsat => SelfContradictory`

### 操作：PinExamples
- source: @f8cded68:crates/intent-lang-core/src/example.rs#L48-L68
- 职责：example 块与所属 intent 子句一致性（V0021）

#### 状态效果
- fact_id: F-IL-BEH-013
  statement: check 管线对每个 example 块调用 check_examples 并用 Z3 校验
  modality: must
  status: confirmed
  source: @f8cded68:crates/intent-lang-cli/src/main.rs#L536-L612
  evidence: `example::check_examples(&prog)`

- fact_id: F-IL-BEH-014
  statement: example 与某一子句矛盾时报告 V0021 并导致 check 失败
  modality: must
  status: confirmed
  source: @f8cded68:crates/intent-lang-cli/src/main.rs#L554-L573
  evidence: ExampleStatus::Violates 分支

### 操作：StartBaseline
- source: @f8cded68:.agents/skills/extract-facts/SKILL.md
- 职责：extract-facts 产出 facts.md，全部条目初始 status 为 draft

#### 状态效果
- fact_id: F-IL-BEH-015
  statement: extract-facts 只产出 facts.md，不写入 .intent
  modality: must
  status: confirmed
  source: @f8cded68:.agents/skills/extract-facts/SKILL.md#L49-L58
  evidence: 反勾结边界

### 操作：ConfirmFacts
- source: @f8cded68:.agents/skills/extract-facts/SKILL.md#L217-L225
- 职责：人工将 draft 条目翻为 confirmed / rejected / deferred

#### 状态效果
- fact_id: F-IL-BEH-016
  statement: 仅 confirmed 条目允许进入 write-intent 翻译
  modality: must
  status: confirmed
  source: @f8cded68:.agents/skills/write-intent/SKILL.md#L188-L195
  evidence: 只翻译 confirmed

### 操作：FormalizeBaseline
- source: @f8cded68:.agents/skills/write-intent/SKILL.md#L183-L210
- 职责：write-intent 将 confirmed 事实译为 @asis 子句

#### 状态效果
- fact_id: F-IL-BEH-017
  statement: 逆向路径默认将 confirmed 行为译为 @asis intent
  modality: must
  status: confirmed
  source: @f8cded68:.agents/skills/write-intent/SKILL.md#L195-L196
  evidence: 默认标 @asis

- fact_id: F-IL-BEH-018
  statement: 一个业务操作对应一个 intent，多条 fact 映射到同一 intent 的多个子句
  modality: must
  status: confirmed
  source: @f8cded68:.agents/skills/write-intent/SKILL.md#L24-L31
  evidence: 合流纪律第 2 条

### 操作：AuditTrace
- source: @f8cded68:crates/intent-lang-cli/src/facts.rs#L292-L354
- 职责：intent trace 审计 facts 与 .intent 映射

#### 状态效果
- fact_id: F-IL-BEH-019
  statement: confirmed 事实若在 .intent 中无 fact_id 引用，列入 confirmed_without_clause
  modality: must
  status: confirmed
  source: @f8cded68:crates/intent-lang-cli/src/facts.rs#L310-L315
  evidence: audit 逻辑

- fact_id: F-IL-BEH-020
  statement: draft 状态的 SUS/UNK 事实阻塞 trace 通过
  modality: must
  status: confirmed
  source: @f8cded68:crates/intent-lang-cli/src/facts.rs#L321-L328
  evidence: undecided_suspicions

- fact_id: F-IL-BEH-021
  statement: 默认 facts 路径为与 .intent 同目录同 stem 的 `<stem>.facts.md`
  modality: must
  status: confirmed
  source: @f8cded68:crates/intent-lang-cli/src/facts.rs#L360-L366
  evidence: conventional_facts_path

### 操作：GenerateAcceptance
- source: @f8cded68:crates/intent-lang-cli/src/main.rs#L1010-L1071
- 职责：accept gen 生成 pytest 与 manifest

#### 前置检查
- fact_id: F-IL-BEH-022
  statement: accept 管线在 typecheck Error 时 exit 2，不生成测试
  modality: must
  status: confirmed
  source: @f8cded68:crates/intent-lang-cli/src/main.rs#L1019-L1034
  evidence: goal A gates goal B

- fact_id: F-IL-BEH-023
  statement: M-A1 仅支持 python-pytest adapter，其他 adapter 加载报错
  modality: must
  status: confirmed
  source: @f8cded68:crates/intent-lang-accept/src/binding.rs#L93
  evidence: unsupported adapter 消息

#### 状态效果
- fact_id: F-IL-BEH-024
  statement: gen 写入 out/test_acceptance.py 与 acceptance_manifest.json
  modality: must
  status: confirmed
  source: @f8cded68:crates/intent-lang-cli/src/main.rs#L1061-L1068
  evidence: 文件路径

### 操作：RunAcceptanceGate
- source: @f8cded68:crates/intent-lang-accept/src/report.rs#L323-L363
- 职责：accept run strict gate 裁决

#### 状态效果
- fact_id: F-IL-BEH-025
  statement: strict gate 下存在 failed 或 manual_pending 时 verdict 为 fail
  modality: must
  status: confirmed
  source: @f8cded68:crates/intent-lang-accept/src/report.rs#L323-L329
  evidence: gate 分支

- fact_id: F-IL-BEH-026
  statement: gate verdict 为 fail 时 accept run exit 1
  modality: must
  status: confirmed
  source: @f8cded68:crates/intent-lang-cli/src/main.rs#L1201-L1203
  evidence: process::exit(1)

### 操作：RunAcceptanceLenient
- source: @f8cded68:crates/intent-lang-accept/src/report.rs#L331-L338
- 职责：lenient gate 允许 pass-with-pending

#### 状态效果
- fact_id: F-IL-BEH-027
  statement: lenient gate 下仅有 manual_pending 无 failed 时 verdict 为 pass-with-pending
  modality: must
  status: confirmed
  source: @f8cded68:crates/intent-lang-accept/src/report.rs#L331-L338
  evidence: lenient 分支

### 操作：AnalyzeCoverageDimensions
- source: @f8cded68:crates/intent-lang-core/src/analysis.rs#L1-L5
- 职责：coverage/diff/impact 等纯 AST 分析，不调用 Z3

#### 状态效果
- fact_id: F-IL-BEH-028
  statement: analysis 模块声明为确定性纯 AST 分析，不调用 Z3
  modality: must
  status: confirmed
  source: @f8cded68:crates/intent-lang-core/src/analysis.rs#L1-L5
  evidence: 模块文档

- fact_id: F-IL-BEH-029
  statement: intent coverage 对 coverage 块报告 covered/total 及 uncovered 组合
  modality: must
  status: confirmed
  source: @f8cded68:crates/intent-lang-cli/src/main.rs#L779-L817
  evidence: cmd_coverage 循环

## 全局不变量

- fact_id: F-IL-BEH-030
  statement: Verified 阶段的 ModelSpec 必须 logicallyConsistent
  modality: must
  status: confirmed
  source: @f8cded68:intent-lang.intent#L135-L137
  evidence: safety VerifiedSpecIsConsistent

- fact_id: F-IL-BEH-031
  statement: StructureChecked 或 Verified 阶段的 ModelSpec 必须 structurallySound
  modality: must
  status: confirmed
  source: @f8cded68:intent-lang.intent#L139-L142
  evidence: safety StructuredSpecIsSound

## 疑似问题区

- fact_id: F-IL-SUS-001
  statement: 含 struct-typed quantifiers 的 theorem 被标记 unsupported 并跳过 Z3
  modality: may
  status: confirmed
  source: @f8cded68:crates/intent-lang-core/src/vcgen.rs#L492-L494
  evidence: transfer.intent TransferPreservesTotal skipped [运行验证过]

- fact_id: F-IL-SUS-002
  statement: safety 规则按参数名+类型匹配 intent 参数，异名同型可能逃逸
  modality: may
  status: confirmed
  source: @f8cded68:crates/intent-lang-core/src/vcgen.rs#L307-L318
  evidence: 注释 "real limitation"

- fact_id: F-IL-SUS-003
  statement: accept_generate 注释声称 verify(V0020) 会阻塞，实现仅 typecheck 未调用 verify_vc
  modality: may
  status: confirmed
  source: @f8cded68:crates/intent-lang-cli/src/main.rs#L1007-L1055
  evidence: doc comment vs 实现

## 存疑区

- fact_id: F-IL-UNK-001
  statement: (unknown — needs human input) diff/impact 是否应在 typecheck 失败时阻断输出
  modality: (unknown)
  status: deferred
  source: @f8cded68:crates/intent-lang-cli/src/main.rs#L860-L863
  evidence: 仅 parse_or_die，未 check_program

## 未覆盖操作
| 操作 | 原因 |
| GateAcceptanceOnVerification | @tobe 应然缺口，非当前代码实然（见 SUS-003） |
| intent-lang-visualizer | 可视化工具，不在本产品能力域 SSOT |

## Extraction Checklist
- [x] 已输出能力清单且与 intent-lang.intent goal 同名
- [x] 域边界是产品能力，不是 CLI 子命令列表
- [x] 每个操作条目三栏完整或有意为空
- [x] 每条 BEH/SUS 有锚点
- [x] 确认关口：BEH/SUS confirmed；UNK deferred
- [x] 待裁决清单已输出（见下方）
