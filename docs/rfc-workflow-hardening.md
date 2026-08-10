# RFC: 工作流交付门槛硬化（结构 lint / 追溯审计 / 生命周期声明）

Status: Accepted —— P1 已实现，经真实金对与第二个真实项目验证（2026-08-10）；
P2 两项启动前置均已解除，P2/P3 未动
Created: 2026-08-10
Revised: 2026-08-10（第一性原理复审，见附录 B；金对实验后修订 D10，见 §10.2；
第二个真实项目取得 telemetry 基线并暴露三个工具缺陷，见 §10.4–§10.5；
其中三项已修、只剩 `function` 未编码，见 §10.5(1)(3)(4)）
Authors: intent-lang contributors
Source: iot-device-register（扫地机设备注册域）存量逆向建模实战反馈，2026-07-23；
第二次实战 wgpu-web-exp（刚体重力仿真域），2026-08-10，见 §10.4
Related:
- `docs/rfc-fact-extraction.md`（facts 协议与确认关口）
- `docs/rfc-modeling-integrity.md`
- `docs/lang/LLM.md`（反勾结原则）
- `.agents/skills/extract-facts/SKILL.md`
- `.agents/skills/write-intent/SKILL.md`
- `tools/visualizer/`

---

## 1. 摘要

一次真实的存量逆向建模（Java 服务 → `sweeper-register.facts.md` →
`sweeper-register.intent`）暴露出：**`intent check` 全绿的产物可以是一份
建模上几乎无效的需求文件**——goal 图大量未认领、状态机只有一条边、
流程图只剩孤立节点，最终整个 `.intent` 被推倒重写。

两个诊断：

> **诊断一：工具没缺能力，缺的是检查在正确时机以正确强制力运行。**
> 第一版的缺陷**本来就是被现有工具发现的**——`--check-states` 报了
> unreachable，goal 图显示了未认领。但这些检查住在一个「可选、需自行编译、
> 且要用户主动想起来跑」的二进制里，于是它们在交付之后才发声。

> **诊断二：写在 SKILL.md 中段的规则不产生行为。**
> 三次失败同一形态——W1（反模式 §3 已写未执行）、W5（checklist 已写未强制）、
> E4（路径约定已写未读到）。文档里有，行为上没有。

由此确立四条取舍原则（§3），并据此做出决策。**因证据样本为 n=1，
决策分三批**：P1 直接修复已观察到的失败，P2 待 P1 在下一个真实项目验证后再定，
P3 是与主线无关的独立小修。

本 RFC 只做决策记录，不含实现。

---

## 2. 动机与现场证据

### 2.1 失败链条

| 步骤 | 结果 | 问题 |
|------|------|------|
| extract-facts → facts.md | 47 条 BEH | 项目无相关自动化测试；SUS/UNK 未逐条裁决即批量 confirmed |
| write-intent 第一版 | `intent check --include-asis` 一次通过 | 大量 Bool 标志位替代生命周期 enum；goal 骨架后置；无 Bootstrap intent |
| visualizer 第一版 | 暴露问题 | 大量「未被 goal 认领」；`--check-states` 报 Absent/Present 均 unreachable |
| 补丁式补注解 | 仍不可用 | 补的是注解，不是建模粒度 |
| write-intent 重写 | 可用 | 生命周期优先重建，6 态 5 边、12 操作节点、4 主题组无未认领 |

关键观察：**第一版和第二版的 Z3 结论完全一样（全绿）**。Z3 验证子句之间的
逻辑一致性，对「这份需求是否刻画了业务的生命周期」一无所知。当前工具链把
Z3 绿灯当作唯一门槛，等于把建模质量完全交给执行者的自觉。

### 2.2 已核对的代码事实

以下事实经核对，作为决策依据（`intent` 0.2.0 / 本仓库 HEAD）：

- **`intent` CLI 无 `viz` / `trace` 子命令**，也不输出「未挂 goal 的 intent」。
  现有子命令：`check / parse / coverage / testspec / diff / impact / explain / accept`。
- **`dist/` 只有 `intent-macos-aarch64` 一个二进制**，release 工作流不打包
  visualizer——技能把可视化写成「可选」，实际是「需自行 clone 仓库 cargo build」。
- **状态 enum 是猜出来的**：`tools/visualizer/src/state_machine.rs` 取
  「primed 等式中变体出现次数最多」的 enum 作为 dominant state enum，无覆盖手段。
- **静默跳过是主流而非例外**：`--check-states` 跑遍 8 个 `examples/*.intent`，
  只有 `ticket.intent` 真正被检查（TicketStatus，6 态 11 边，全绿），
  其余 7 个打印 `No dominant status enum detected — skipping` 后 **exit 0**。
- **自家范例就有未认领 intent**：`ticket.intent` 的 15 个 intent 中，
  `CreateTicketSoftReview` 与 `SupervisorSetCritical` 未出现在任何
  `realized_by` 里。前者正是 `write-intent/SKILL.md` 用来演示 `@doc` 的范例。
- **coverage 计数是硬编码**：`tools/visualizer/src/coverage_matrix.rs`
  的 `covered_combinations: 0` 旁挂注释 `// Would need semantic analysis`；
  而同文件的 mermaid 渲染路径刻意不给计数，两条路径立场相反。
- **flowchart 立场明确**：文件头注释写明
  「so it never invents flow that isn't in the `.intent`」，节点完全派生自
  状态机迁移数据。
- **E4 的约定其实已存在**：`extract-facts/SKILL.md` 边界节已写
  「只允许产出 `<业务域>.facts.md`（与未来的 `.intent`、`.intent.bind.toml`
  同目录并排）」，但使用者第一次放置仍靠猜。

其中第 4、5 条各自否证了一个直觉：

- 第 4 条说明**当前的 `--check-states` 对 Bool 标志位建模给出的不是报错，
  而是一句 ℹ️ 加一个绿灯**。sweeper-register 第一版能看到 unreachable，
  是因为它凑巧留了 Absent/Present 这个 enum。
- 第 5 条说明「未挂 goal」若默认 error，**本仓库自己的示范文件立刻变红**。

---

## 3. 设计原则（第一性）

**P-1 检查属于验证器，不属于渲染器。**
一个检查放在哪个二进制里，决定了它在什么时机、以什么强制力发声。
结构检查不需要渲染，就不该住在画图工具里。

**P-2 宁可漏报，不可假阳性。**
对正确的建模发警告，只会训练人忽略警告，之后连真信号一起被忽略。
凡是工具无法区分「缺陷」与「该域本来如此」的信号，一律不得默认报错——
典型如「没有状态机」（转账、排序本来就没有生命周期）、
「没有终态」（长期存活实体合法）、「无 creation 边」（可能只建模生命周期中段）。

**P-3 不逼人发明结构。**
任何「每个 X 必须有 Y」的硬性要求，都会在 X 本来不该有 Y 时逼出假的 Y。
需求被工具的形状扭曲，比图不好看严重得多。

**P-4 DoD 只写机器验不了的。**
机器能验的交给命令拦截，不进 checklist；checklist 的长度由此推导，而非拍定。

---

## 4. P1：第一批决策（直接修复已观察到的失败）

### D1 结构检查下沉到核心 CLI

`intent check` 内建两类结构检查——**未被任何 goal 的 `realized_by` 认领的
intent/safety**、**状态机结构问题（不可达 / 死态 / 无终态 / 无 creation 边 /
结构自相矛盾）**；visualizer 从此只负责渲染。

- 反馈来源：W1、W5、V1、§4.3-2、§2.3.1、§3.3-1
- 依据 P-1。这两类检查本身不需要渲染：`build_state_machine` +
  `analyze_state_machine` 是纯图算法，「未认领」是集合差集。它们住在
  visualizer 里是历史位置，不是技术必然。
- 附带解决 §2.3.1 与 V1 的矛盾：原反馈要求「visualizer 非零退出则不得交付」，
  却又抱怨 visualizer 不随 CLI 分发——不能把需要用户自行编译的二进制定为门槛。
- 落点：`crates/intent-lang-core` 新增结构分析模块 + `intent check` 输出。

### D2 严重度：默认宽松，`--strict` 收紧

| 检查 | 默认 | `--strict` |
|------|------|-----------|
| 结构自相矛盾（一个 intent 无条件断言两个不同次态） | **error** | error |
| 状态不可达（**且文件中存在 creation 边**） | **error** | error |
| 无 creation 边 / 无终态 / 死态 / 未挂 goal | **warning** | **error** |

技能 DoD 规定使用 `intent check --strict`。

- 反馈来源：§2.3.3
- 依据 P-2 与 P-3。**这是对初版 RFC 的修正**：初版把「无 creation 边」定为
  默认 error，与 D5「不要逼人发明状态」直接冲突——它逼出来的是假的 Bootstrap
  intent，与 D5 拒绝的假 phase 迁移是同一类错误。工具无法区分「缺 Bootstrap」
  与「本文件只建模生命周期中段」（分文件建模、创建发生在上游系统均合法）。
  同理「无终态」对 `Active ↔ Frozen` 这类长期存活实体是正常的。
- 保留严格性的方式是 `--strict` 而非默认值：默认对存量用户与教学片段友好，
  技能场景一条可复制的命令行即可收紧。W4（无 Bootstrap）在 `--strict` 下
  仍会被拦截。
- 依据 §2.2 事实 5：若「未挂 goal」默认 error，本仓库 `ticket.intent` 即变红。

### D3 `@lifecycle`：状态机检查的 opt-in 声明

新增注解标在 enum 上，**可声明多个**。语义是**「我声明这是生命周期，请检查它」**：

- 声明了 → 每个被声明的 enum 各自跑结构检查、各自出一张状态机图；
- 一个都没声明 → **静默跳过**，不报 warning，不回退启发式。

- 反馈来源：V2（`RegistrationPhase` 与 `SnQueryPhase` 并存时 SN 查询链不出图）
- **正当性来自消除假阳性，不是来自多图并列**（对初版 RFC 理由的更正，见附录 B）。
  依据 P-2：「没有状态 enum」既可能是 W2 式缺陷，也可能是该域确实没有生命周期
  （transfer / sorting / auth），工具无从区分，因此不能默认报警。有了 opt-in
  声明，D1/D2 的状态机检查才有了明确且无歧义的适用边界。
- **明确承认：`@lifecycle` 抓不到 W2。** 一个用 `siteAvailable`、
  `extendInserted` 建模的人压根不会去写这个声明。W2 是建模品味，机器判不了，
  由 D4 的反模式文档承担。
- 不采用「对所有参与 primed 等式的 enum 建状态机」这一零语法方案：
  `Priority` 之类非生命周期 enum 会被误当状态机，报出「Low 不可达」
  「High 无终态」的假阳性，违反 P-2。
- 代价：语法扩展涉及 lexer / parser / ast / `docs/lang/SPEC.md` / 两个技能。
- 兼容：旧文件不写则行为等同现状（无检查），向后兼容。

### D4 「生命周期优先」升为规定起草顺序

write-intent 的起草顺序由「建议」改为「规定」：

```text
enum/type（含 @lifecycle）→ @capability/@guardrail goal（含 realized_by 骨架）
  → Bootstrap/初始态 intent（无 require 源态 → creation 边）
  → 各操作 require 源态 / ensure 次态
  → Z3 闭环 → intent check --strict
```

同时新增反模式：**「Bool 标志位替代生命周期 enum」**。

- 反馈来源：W2、W3、W4、§2.3.2、§6（重写时验证有效的顺序）
- 门槛只能拦住结果，拦不住浪费——第一版整个推倒重写的代价，正是没有起草顺序
  造成的。这也是 W2 的唯一抓手（见 D3）。

### D5 `@asis` 逆向的 phase 迁移要求只作启发式

从 facts 翻译时「每个操作至少一条 phase 迁移」写为**启发式提示，不作硬性要求**。

- 反馈来源：§2.3.4
- 依据 P-3。V4 自己提供了反例：`V2Route`、`V3Reform` 这类分流/改制逻辑本来
  就没有 phase 迁移，这是建模的真实形状。

### D6 新增 `intent trace`，纯审计不生成

```bash
intent trace --facts <domain>.facts.md <file>.intent
```

输出：

1. 「confirmed 但在 `.intent` 中无对应 clause」；
2. 「引用了不存在 / 非 confirmed 的 fact_id」；
3. **「本次从 facts.md 解析到 N 条事实（BEH n1 / SUS n2 / UNK n3）」**。

非零退出。**不生成任何内容。**

- 反馈来源：W6（47 条 confirmed 中约 6 条未落 clause 却已标 confirmed）、
  C3、§2.3.5、§4.3-1
- 拒绝 §4.3-1 的生成式方案（`intent import-facts` 产骨架）：facts → 骨架的
  自动生成会把「翻译」退化成「填空」，而翻译恰恰是最需要判断的一步——
  质量规则 6 要求把冲突**如实并列翻译**而非顺手修好，这种判断无法生成。
- 第 3 项计数是**对解析可靠性的自证**，替代初版 RFC 的 schema 版本契约：
  宽松解析的失败模式与要抓的 bug 同形（都表现为「这条 fact 不存在」），
  而一个「解析到 41 条」的数字能让人立刻发现应该是 47 条。成本低一个数量级，
  且更直接。
- **适用条件**：仅存量逆向路径（存在 facts.md 时）。正向建模场景无此命令，
  DoD 对应条目必须写明这一条件性（初版 RFC 的漏洞，见附录 B）。
- 实现成本低：`// F-RF-BEH-001` 是行注释，纯文本扫源文件即可，不需要碰 AST。

### D7 确认关口机器化，`status` 增加 `deferred`

1. extract-facts 末尾输出**「待人工裁决清单」**（所有 SUS + UNK 单列）；
2. `intent trace` 发现仍有 `draft` 状态的 SUS/UNK 即报错——
   「不裁决不得 write-intent」从口头约定变成机器门槛；
3. `status` 增加第三个取值 **`deferred`**：语义为「人已看过，明确决定不裁决、
   也不进 `.intent`」，与「还没看」区分。

- 反馈来源：E3、X2、§1.3
- X2 观察到的正是「agent 批量 confirmed」，SUS-001（V1 register 的 Kafka
  `country` 字段写 siteCode）因此被跳过而非被 Z3 检出。**纯文档禁令拦不住一个
  想推进任务的 agent**。
- `deferred` 是必要配套：否则「并发重复退款行为未确定」这类本来就裁决不了的
  UNK 会永久堵住门槛。
- 不采用交互式 `intent facts confirm`：agent 会话里 agent 可自行管道喂入
  应答绕过，人并不在终端前。

### D8 `intent trace` 按约定自动定位 facts.md

默认在 `.intent` 同目录查找 `<domain>.facts.md`，找不到时报错并打印约定。

- 反馈来源：E4
- E4 的约定在 `extract-facts/SKILL.md` 里**已经写了**却没生效（§2.2 末条）。
  让工具的默认行为成为约定的载体，比在文档里再写一句强。

### D9 write-intent：DoD 前置，内容由 P-4 推导

把交付标准 checklist 提到紧跟「边界」之后的位置。**依 P-4，DoD 只写机器验不了的**：

```text
□ example 用的是真实业务值（向用户要，不自己编）
□ intent testspec 的场景符合业务预期（happy path + 每条 require 的违反路径）
□ 文档中的冲突已如实并列翻译，未私自挑一方修好（质量规则 6）
□ [仅存量逆向] intent trace --facts <domain>.facts.md <file>.intent 通过
```

机器能验的（结构、追溯、Z3）不进 checklist，由
`intent check --strict` 与 `intent trace` 拦截。

- 反馈来源：W1、W5、§2.3.1、§2.3.3
- 条数是推导结果而非拍定（对初版「≤8 条」的修正）。
- **不拆 references/**：语法速查是高频刚需而非低频参考，拆走会增加语法错误；
  `write-intent/SKILL.md` 现有的「本文档语法部分自足，无需任何仓库内文件」
  是有意设计（技能会被装到别的项目使用）。文档瘦身移入 P2 且效果存疑
  （见附录 B）。

### D10 无 example 的 intent 报 warning

结构 lint 新增一项：intent 无任何 `example` 块 → warning（`--strict` 下 error）。

- 反馈来源：E2（项目无自动化测试，`example` 候选无真实业务值来源）
- 依 P-4，这条从「在技能里提示索要真实值」改为机器强制，因为它可验。
  但「值是否真实」机器验不了，故 DoD 保留对应人工项（D9 第一条）。

---

## 5. Telemetry 与 Eval

`AGENT.md` 要求「在做任何事情前考虑 telemetry 和 eval」。初版 RFC 遗漏此节。

### 5.1 指标

`intent check --format json` 增加结构检查结果字段，使以下指标可被采集：

| 指标 | 定义 | 用途 |
|------|------|------|
| 结构 lint 触发率 | 每类检查（未认领 / 不可达 / 无 creation / 无 example）的触发文件占比 | 判断某类检查是否噪音过大（P-2 违反信号） |
| `--strict` 失败率 | 技能会话中首次 `check --strict` 的失败占比 | 门槛是否真的在拦东西 |
| 漏译检出率 | `trace` 报出的「confirmed 无 clause」条数 / confirmed 总数 | 直接量化 W6 类缺陷 |
| 首过率 | `.intent` 首次达成 DoD 全绿、无需推倒重写的会话占比 | **本 RFC 的核心结果指标** |
| 重写率 | 同一业务域 `.intent` 被整体重写的次数 | 第一版失败的直接度量 |

### 5.2 Eval：把本次失败收进回归夹具

sweeper-register 的两个版本构成一对现成的 golden pair：

- **negative fixture**：第一版 `.intent`（Z3 全绿、建模无效）
  → 断言 `intent check --strict` **必须失败**，且失败项包含未认领 intent
  与状态机结构问题；
- **positive fixture**：重写版 `.intent`（6 态 5 边、4 主题组无未认领）
  → 断言 `intent check --strict` **必须通过**。

这对夹具直接检验本 RFC 的核心主张——「新门槛能区分这两版」。若 negative
fixture 在 `--strict` 下仍然通过，说明 D1/D2/D3 的设计没有命中真实缺陷。

前置动作：向反馈来源索取两版 `.intent`（脱敏后）作为
`examples/fixtures/` 或 `crates/intent-lang-core/tests/` 的数据。
**在拿到这对夹具之前，P2 不应启动。**

---

## 6. P2：待验证后再定（不在本轮实施）

样本量为 n=1，以下决策缺乏第二个样本支撑，等 P1 在下一个真实项目跑通、
且 §5 的 eval 夹具就位后再判断。

| 项 | 内容 | 存疑点 |
|----|------|--------|
| D11 | facts.md 定义刚性机读子集 + `facts_schema: 1` 版本位 | schema 尚未稳定就先版本化是 YAGNI；D6 的解析计数自证可能已经够用 |
| D12 | visualizer 合入主 CLI 成 `intent viz`，输出落 `viz/<git-sha>/` | 便利性优化而非问题修复。D1 已论证「图不是门槛」，让验证器吞下 732 行 `html_generator` 承担 UI 职责代价不小；V1 的痛点 90% 可由一行 `cargo install --git` 解决 |
| D13 | write-intent 语法速查/反模式拆入技能目录内 `references/` | 若 agent 不读中段散文，凭什么相信它会读另一个文件——可能只是把「读不到」换个地方。真正有效的是 D9 的 DoD 前置 |

---

## 7. P3：独立小修（与主线无关，随时可做）

这些不是过度设计，是成本低、无架构影响的正确修复，只是不构成主线。

| 项 | 内容 | 反馈来源 |
|----|------|----------|
| D14 | coverage 撤掉硬编码假计数：JSON/stats 不再输出 `covered`/`missing`（或标 `not_computed`），与 mermaid 渲染路径的诚实做法对齐 | V3 |
| D15 | flowchart 图下增加「未参与任何 phase 迁移的操作」清单，不动图中的边 | V4 |
| D16 | E0007 从 primed ensure 反推并列出缺失的 `modifies` 字段名；E0006 提示重名标签所在行 | C1、C2 |
| D17 | README 澄清技能安装路径 `.agents/skills/` 与 `.cursor/skills/` 均可 | 反馈附录 |

D14 说明：V3 看到的「Covered: 0 / 36 维全 Missing」不是分类错误，是**一个从没
算过的数字被当成结果输出**。不选「真做语义分析算 covered」——静态引用分析分不清
真缺口与蕴含式规则合法地不逐一列举，算出一个原理上不准的数，会制造**比 0 更
危险的、更可信的假数字**。

D15 说明：守住 `flowchart.rs` 文件头「never invents flow that isn't in the
`.intent`」的立场。旁挂清单不画任何不存在的边；按 V4 原意渲染游离注释节点则会
退回第一版「流程图几乎只有孤立 `RegisterTrigger`」的观感。

---

## 8. 明确不做

| 项 | 理由 |
|----|------|
| `intent import-facts` 生成 `.intent` 骨架（§4.3-1） | 把翻译退化成填空，绕过质量规则 6 要求的判断（D6） |
| coverage 真做语义分析算 covered（V3 备选） | 静态分析原理上不准，会制造更可信的假数字（D14） |
| `@asis` 硬性要求每个操作有 phase 迁移（§2.3.4 强化版） | 违反 P-3（D5） |
| 「无 creation 边 / 无终态」默认 error | 违反 P-2 与 P-3（D2） |
| 未声明 `@lifecycle` 时报 warning | 违反 P-2：转账/排序本来就没有生命周期（D3） |
| 对所有参与 primed 等式的 enum 建状态机（D3 零语法备选） | `Priority` 类 enum 假阳性，违反 P-2 |
| 把无迁移 intent 渲染进流程图（V4 原意） | 违反「不发明流程」立场，孤儿框让读图更糟（D15） |
| visualizer 独立二进制随 release 多平台打包（V1 备选） | 若要做则 D12 的合入方案更彻底；两者都在 P2 |
| E1 未指定业务域时给默认选域建议 | 纯交互体验，无法机器强制、也挤不进 P-4 推导出的 DoD。记为「已知会失效，不做」 |
| 交互式 `intent facts confirm`（X2 备选） | agent 可自行管道喂入应答绕过，人不在终端（D7） |

---

## 9. 兼容性与影响面

- **破坏性：无（默认路径）**。D2 把所有可能误伤的检查放在 `--strict` 之后；
  D3 未声明 `@lifecycle` 即等同现状。实测 `examples/` 全部 8 个文件：默认路径
  退出码与改动前完全一致（`sorting` / `basics/transfer` / `smarthome` 三个原本
  就失败，原因分别是 VC 不成立、故意的反例演示、E0004 类型错误，与本次改动无关）。
  `--strict` 下 4 个文件失败，全是真实缺口：未认领的 intent/safety 与缺 example。
- **`ticket.intent` 已标注 `@lifecycle TicketStatus`** 作为 dogfooding：其
  6 状态 11 迁移结构干净（无 S0003–S0007），同文件的 `Priority`、`TicketType`
  等 5 个普通枚举被正确忽略——这正是旧启发式（猜主导枚举）会误判的场景。
- **语法扩展**：D3 的 `@lifecycle` 为新增注解，旧文件不写则跳过检查。
  它与 SPEC 中「除 `@tobe`/`@asis` 外注解均不参与验证」的表述有张力——
  `@lifecycle` 不参与 Z3，但参与 `check` 的退出码；已在 SPEC §6.7.2 与技能
  语法速查里改为「不参与 Z3 公式，但影响 `intent check` 行为」。
- **facts 格式**：D7 的 `deferred` 是 `status` 的新增取值，旧文档不含该值，
  不受影响。刚性 schema 推迟到 P2。
- **技能**：D4/D9/D10 改写 `write-intent/SKILL.md`；D7 改写
  `extract-facts/SKILL.md`（待裁决清单 + `deferred`）。
- **JSON 输出**：§5.1 要求 `check --format json` 增加结构检查字段，
  下游消费者需兼容新增字段。

---

## 10. 落地顺序

```text
D3（@lifecycle 语法）
  └─→ D1（结构检查下沉）──→ D2（严重度 + --strict）──→ D10（无 example 检查）
                                    │
                                    └─→ D9（DoD 前置，引用 --strict 作为门槛）
                                          └─→ D4/D5（起草顺序与反模式）

D6（intent trace + 解析计数）──→ D7（确认关口 + deferred）
                              └─→ D8（约定定位）

§5 eval 夹具（已取得，见 §10.2）—— P2 前置之一已解除

P3（D14–D17）彼此独立，随时可做
```

关键依赖：**D1 必须先于 D9**——DoD 里写「`intent check --strict` 必须绿」的
前提是 `check` 真的会检查这些结构问题，否则又是一条写了不生效的规则，
正是本反馈诊断出的病。

### 10.1 P1 落地记录（2026-08-10）

| 决策 | 落点 |
|------|------|
| D3 | `EnumDecl.annotations` + `is_lifecycle()`；SPEC §2.3 / §6.7.2 |
| D1 | 推导 `intent-lang-syntax::structure`（无 z3，visualizer 可复用）；策略 `intent-lang-core::structure` |
| D2 | `intent check --strict`；S0001–S0007，S0004/S0007 默认 error |
| D10 | S0002（§10.2 后修订：`@asis` intent 豁免） |
| §5.1 | `check --format json` 增加 `structure` 段 |
| D6/D8 | `intent trace`（`intent-lang-cli::facts`），按约定定位 `<域>.facts.md`，报告开头打印解析计数 |
| D7 | `status: deferred` 取值；`draft` 的 SUS/UNK 拦截；extract-facts 输出待裁决清单 |
| D4/D5/D9 | `write-intent/SKILL.md`：DoD 前置、规定起草顺序、Bool 标志位反模式 |
| V2 | `lifecycle_state_machines`：每条生命周期各出一张图（HTML 分节 + `statemachine-<enum>.mmd`） |

**推导层为何落在 syntax 而非 core**：core 依赖 z3（vendored，需 cmake）。若让
visualizer 依赖 core 以复用推导，"只想看图"的用户被迫承担验证器的构建成本，
恰好加重了 V1 抱怨的分发问题。因此纯 AST 推导放 syntax（两个消费者共用一份实现，
不会漂移），严重度策略放 core（P-1：门槛属于验证器）。

**已知未被机器覆盖**（P-4 要求显式记录）：D5 的 Bool 标志位反模式**在彻底形态下
无法被 `--strict` 检出**。真实金对夹具（§10.2）证实：Bool 版本压根没有生命周期可
分析，S0003–S0007 全不触发，它被拦下靠的是 S0001（未认领），与 Bool 建模无关。
若作者补齐 goal 认领，彻底的 Bool 建模可以通过门槛。这条目前只靠技能散文兜底，并由
`structure_gate.rs::boolean_flag_modeling_is_the_gate_s_known_blind_spot`
把它钉成一条会失败的测试——哪天有检查学会了识别它，那个测试会红，提示回来改本节。

**半途形态已由 S0008 覆盖**（§10.3）：enum 建了但个别迁移的前提写成 Bool 时，
那条边失去源态、被推导成额外的 creation 边，机器可见。

### 10.2 §5.2 金对实验结果（2026-08-10）

夹具已取得，不必再向反馈来源索取：源项目 `docs/` 从未提交进 git，但那次会话的
逐条编辑记录完整保留，回放后得到重写前后两版。脱敏方式为**域转置**（业务名词与
内部接口名替换，声明/子句/注解逐条保留），转置后结构信号计数与原件完全一致，
落在 `crates/intent-lang-core/tests/fixtures/provisioning-{unsound,sound}.intent`。

| 夹具 | `check` | `--strict` | 结构信号 |
|------|---------|-----------|---------|
| unsound（作者弃用的第一版，369 行） | **0** | 1 | S0001 ×9（6 intent + 3 safety 未认领）；无生命周期可分析 |
| sound（重写版，631 行） | 0 | **0** | 2 条 `@lifecycle`（6 态 5 迁移 / 3 态 2 迁移），干净 |

两版**都 Z3 全绿**，这正是本 RFC 的前提：验证通过不代表建模成立。§5.2 的判据
（negative 在 `--strict` 下必须被拦下）**满足**，D1/D2 在非本人编写的真实数据上
成立。

但实验暴露一处 P1 设计缺陷并已修订：**D10 原样实现会让门槛失去区分能力**。
四个真实业务域的 intent 100% 是 `@asis`，example 只有 3–6 个，于是三个候选夹具
在 `--strict` 下全部因 S0002 失败、失败原因相同。更糟的是 D9 写进技能的 DoD
（`--strict` 必须绿）与同一份技能「数值必须向用户要真实值」互相矛盾——agent 只剩
编造数值或永远过不了门槛两条路。D10 的依据 E2「无测试时索要真实值」针对的是
**正向建模**；逆向一次性索要 30 组产线数据不现实。

修订：**S0002 豁免 `@asis` intent**。实然行为的权威示例是产线数据与存量测试，
由验收环节采集；建模期逼出的只会是假数据，而假数据比缺失更坏（P-2：宁可沉默也
不要假信号）。修订后金对立即恢复区分能力（1 vs 0），上表即修订后的结果。

`@lifecycle` 的价值另有一条旁证：sound 版的 `SnQueryPhase` 在旧启发式下完全不可
见——文件顶部注释写着「以 RegistrationPhase 为可视化主状态机（ensure 命中最多）」，
说明作者当时得手工推理启发式会猜中哪个枚举。声明后两条生命周期各自成图，另外
7 个非生命周期枚举零误报。

### 10.3 S0008：生命周期多入口（金对实验的衍生发现）

给源项目四个域补 `@lifecycle` 后（5 条生命周期），状态机检查全部报「✅ 所有状态
可达且能终止」，但 `CleanPhase` 的图是这样的：

```
[*] --> CleanCompleted: CompleteClean
[*] --> Initial: BootstrapClean
[*] --> LabelsProcessed: ProcessDeviceLabels
Initial --> DevicesResolved: ParseCleanRequest
```

清理流程「可以凭空从已完成状态开始」，而 S0004 判它干净——因为每个孤立状态都自带
一条 creation 边，**可达性因此空洞成立**。根因是 `ProcessDeviceLabels` 断言
`ensure ctx.phase' == LabelsProcessed` 却把前提写成 `require ctx.devicesFound`
（Bool）而非 `require ctx.phase == DevicesResolved`：这条边没有源态，与 Bootstrap
无从区分。

creation 边数量在 5 条真实生命周期上完全区分成立与破碎：

| 生命周期 | creation 边 | 建模 |
|---------|-----------|------|
| `RegistrationPhase` / `SnQueryPhase` / `RenovatePhase` | 1 | 成立 |
| `ActivatePhase` | 2 | 链条部分断裂 |
| `CleanPhase` | 3 | 链条断成三段 |

因此新增 **S0008：一条声明的生命周期有多于一个入口**，默认 warning、`--strict`
升 error（与 D2 severity 模型一致——双入口确实可能合法，如实体既可注册创建也可
导入创建，故不设为默认 error）。它是 Bool 标志位反模式**唯一机器可见的形态**，
部分关闭了 §10.1 记录的盲区。

判定层落在 core（`report.creation_targets.len() > 1`），事实层
（`StateMachineReport.creation_targets`）落在 syntax，与 D1 分层一致，visualizer
的 `--check-states` 读同一份事实，不会漂移。

### 10.4 第二个真实项目的 telemetry 基线（2026-08-10）

P2 的第二项启动前置：在一个此前从未建模过的项目上跑一遍 P1 全流程。选的是
`wgpu-web-exp` 的**刚体重力仿真**域（Rust，`app/src/physics.rs` 125 行 + 唯一调用方）。
与金对来源（Java 业务服务）刻意不同域、不同语言、不同性质——它是数值计算，没有
业务状态机。

**流程如实执行**：extract-facts 与 write-intent 分处独立上下文（后者是子代理，
只拿到 `facts.md`，看不到读码过程），中间走人工确认关口。

| 指标 | 值 |
|------|-----|
| 事实条数 | 37（30 BEH / 5 SUS / 2 UNK） |
| 裁决结果 | 31 confirmed / 4 rejected / 2 deferred |
| `.intent` 规模 | 425 行；1 enum（`@lifecycle`）/ 4 type / 6 goal / 1 safety / 7 intent（全 `@asis`）/ 5 example（负数字面量修复后补第 6 条，见 §10.5(4)） |
| **首过率** | **否**——`check` 迭代 6 次 |
| 失败归因 | 6 次里 3 次源于工具缺陷（见 §10.5），**0 次源于建模错误** |
| **`--strict` 首过率** | **是**——S0001–S0008 零发现 |
| 文件级重写 | **0 次**（金对的破碎版曾需整体重写） |
| 设计阶段推倒 | 2 次，均由动笔前的探针触发，未产生返工 |
| `trace` 缺口 | 0（31 条 confirmed 全部落到子句） |

**被点名直接避免返工的三条**，都是 P1 的产物：D4 的规定起草顺序（goal 先写全，
S0001 从未触发）、D5 反模式 3（据此把 `paused: bool` 判定为「真正独立的开关」而
**不是**生命周期，避免建出一条无终态的 Running↔Paused 环，`--strict` 下会吃 S0005
并被逼编造假终态）、以及 §10.2 修订的 S0002 `@asis` 豁免（省掉 7 个 intent 的假
example）。

**测试套件缺席的处理**：该项目零 `#[test]`，D10「向用户要真实值」无处可要。改用只读
探针——把 `physics.rs` 逐字复制进独立 crate 运行，产出 12 组实测值，`facts.md` 里标
`[运行验证过]`。5 个 example 的数值全部来自它。这条路径值得写进 extract-facts 技能：
**无测试套件时，纯函数域可用只读探针替代**（技能已允许探针，但没说它能顶替 example
候选这一用途）。

**新暴露的技能缺口（未修）**：被建模代码是浮点数时零指引。子代理只能自创一整套定点
约定（长度 mm / 时间 µs / 恢复系数千分数）。换个会话重做几乎必然给出不同的单位表，
两份 `.intent` 无法 diff——这直接损害「代码演进后重跑并 diff」这条设计意图。

### 10.5 §10.4 暴露的四个工具缺陷

跑真实项目的收获主要不在基线数字，而在这几个此前无人触发的缺陷。三个已修，
只剩 `function` 未编码。

**（1）SMT 管道静默不健全 —— 已修。**

`smt.rs::run_z3` 用 `Solver::from_string` 且丢弃返回值。当发射的 SMT 里有 Z3 无法
解析的形式（实测起因：`function` 声明从不被编码，调用点成了未声明符号），Z3 会
**丢掉那条 assert 继续解析**，剩下一个约束更弱的 solver。于是：

- 有证明目标时 → `sat` → 报 `❌ FAILED`，`detail` 为空串、无反例、无错误码；
- 无证明目标时 → 短路成 `✅ verified`，唯一那次 Z3 查询（反空洞检查）已经烂掉。

即**任何编码器 bug 都降级成一句自信的错误结论**，而非失败。这是验证器最不该有的
故障模式。修法是 `load_solver`：对比「写出去的 assert 数」与 `get_assertions().len()`，
不一致就拒绝作答（`VerifyResult::Error` / 新增 `SatOutcome::Error`）。`verify_vc` 的
反空洞路径原先只匹配 `Unsat`，`Error` 会落回 `Verified`，一并堵上。

守卫上线后立刻在仓库自己的 `examples/basics/sorting.intent` 上抓到第二例（写 6 条、
收 0 条）——它同样用 `function`。该文件此前就是红的，只是给不出原因。

留下的取舍：数断言只能说「有东西没进去」，说不出是哪个符号。要精确定位得引
`z3-sys` 调 `Z3_get_error_code`。当前实现优先保证「绝不作答」这条性质，定位精度
让位。

**（2）`function` 声明从不被编码 —— 未修。**

`vcgen.rs` / `smt.rs` 里 `Declaration::Function` 零引用，`define-fun` 三处命中全在
model **解析**代码里。语法接受、类型检查通过、SPEC §7 有文档，但它对验证毫无作用。
现在至少会被（1）拦成 error 而不是假结论。两条出路：真编码成 `define-fun`，或在
typecheck 阶段直接拒绝并提示「请内联」。后者便宜且诚实。

**（3）无 prime 的 `safety` 不变量恒真 —— 已修。**

`vcgen` 对每条 goal 同时 `assumes.push(unprime_expr(e))` 与 `goals.push(e.clone())`。
表达式不含 prime 时两者是同一个式子，SMT 里成为：

```
(assert (>= a_b 0))
(assert (not (>= a_b 0)))
```

无条件 UNSAT，与后置状态无关，于是恒「verified」。最小复现：`safety NonNeg { invariant
a.b >= 0 }` 配一个 `ensure a.b' == 0 - 1` 的 intent，通过；同样约束写成 intent 级
`invariant a.b' >= 0` 则正确失败并给出反例 `a.b = 0`。

影响面：**仓库里所有无 prime 的 safety 都什么也没证**，含 `examples/requirements/
ticket.intent` 的 9 条。

**定下的语义**：`invariant` 意为「任何状态下都成立」，故前置形态作假设、后置形态
作证明目标。整条表达式不含 prime 时，其中的**状态字段**在用作目标时自动补 prime；
含 prime 则原样使用（`a.balance' >= a.balance` 是刻意关联前后态，补 prime 会把它压成
恒真式——即从另一侧掉回同一个坑）。裸标识符是标量入参或枚举变体，不是状态，永不补。
写入 SPEC §3。

**修复中发现的连带问题**：safety 的参数是**按名字匹配的自由符号**，不是全称量词
（`ticket.intent` 第 179 行的注释表明作者早知此事）。目标恒真时这无所谓；一旦目标
变成 `c.openTicketCount'`，那些不持有 `c: Customer` 的 intent 就在为一个自己够不到、
frame 也不会约束的符号背锅——首次改完 `ticket.intent` 64 条 intent 全红，`physics-sim`
的 `BuildTransform(b: Body)` 因一条关于 `s.bodyCount` 的 safety 而失败。故补上作用域
规则：**一条 safety 只附加给参数（名字与类型）能对上的 intent**。代价是同类型状态
用别的参数名时会漏网，已在 SPEC §4 记为已知限制并建议统一命名。

**修复后的全仓影响面**：只产生两条新发现，且都是真的。

| 条目 | 判定 |
|------|------|
| `access-control.intent :: LegacyGrantAccessV1` | **符合预期**。`@asis` 遗留意图，注释明写「老代码只检查认证，不检查 banned 状态——这是一条已知漏洞」，文件顶部说 `--include-asis` 的用途正是发现这个。修复前该示例的演示意图一直没兑现。 |
| `ticket.intent :: ResolveTicket` | **真 bug，已修**。`modifies` 含 `t.resolution, t.rmaNumber, t.refundNumber, t.replacementOrderId` 却只 `ensure t.status' == Resolved`，四个字段后置无约束；require 校验的全是前置值，于是 RefundOnly 工单可带 `refundNumber' = 0` 结案——正是该 intent `@doc` 承诺要防的事。其 example 把 resolution/单号放在 `given` 里，证实它们是入参；`modifies` 收窄为 `t.status`。 |

其余文件（`billing` 3 块 safety、`auth`、两个 `transfer`）保持原状态。这是一次典型的
「把恒真的断言变成真断言，就在旗舰示例里逮到一个真缺陷」。

**（4）`example` 拒绝负数字面量（E0009）—— 已修。**

`t.x: -5` 报「must be literals」，`5` 正常。对物理域是硬伤：重力、下落速度、
`gravity.y = -9.8` 全是负数，导致该域最该被 example 钉住的一条事实（F-PHY-BEH-018
半隐式欧拉的**顺序**语义）没有任何机器守卫——变异测试证实把它改成显式欧拉，
`check` 依然全绿。

根因在词法层：数字的正则是 `[0-9]+`，`-5` 只能以前缀负号的形式到达 AST，成为
`UnaryOp(Neg, IntLit(5))`，而 E0009 的白名单只认四种字面量节点。下游其实早就
支持负值——`smt.rs` 会把负 `IntLit` 正确发射成 `(- 5)`，`py_value` 也认得 `-9`。
故修在解析器：前缀负号遇到整数字面量时折回 `IntLit(-5)`，白名单不动，「必须是
字面量」这句话从此为真；非字面量的 `-p.x` 仍是运算符。

**闭环验证**：给 `physics-sim.intent` 补上此前写不出的那条 example（半径 100mm
的球从 1m 静止下落一帧，`newVelY = -164`、`posY' = 997`），基线全绿；把
`pos_integrated_def` 里的 `newVelY` 改成 `b.velY`（显式欧拉）后，该 example
立即 `INCONSISTENT`，退出码 0→1。**intent 本身仍 verified**——显式欧拉的子句集
自身自洽，抓住这次语义改动的完全是 example。这恰好演示了 example 补上的是
`require`/`ensure` 无法自查的那一层，也就是 §6.8 所说的「防形式化偏差」。

一处诚实标注：该 example 的数值由本文件的定点模型推出，不是探针实测。定点化
（mm/µs/向下取整）与代码的 f32 有已声明的量化偏差，故它守的是模型内部的积分
顺序，不足以证明模型与代码逐位一致——后者仍需探针复核，已在文件内注明。

## 11. 未决 / 跟进

- **三环未闭合（X3）**：本次实战未跑 `implement-testspec`，
  `.intent` → binding → 可执行验收这最后一环未被验证，testspec 派生是否与
  Java 控制器边界对齐仍是未知数。在三环全绿之前，本 RFC 门槛设计对下游的
  实际效果无法确认。
- **X1**：「可视化不应作为用户主动要求的独立步骤」已由 D1 + D9 覆盖，
  无需单独决策。
- ~~**待索取**：两版 `.intent`（脱敏），用于 §5.2 的 eval 夹具~~ —— 已从会话编辑
  记录回放取得，结果见 §10.2。
- ~~**P2 前置之二**：在第二个真实项目上跑一遍 P1 全流程，取得 telemetry 基线~~
  —— 已完成，见 §10.4。**P2 的两项启动前置均已解除。**
- **§10.5 遗留的工具缺陷**：只剩 `function` 未编码（(2)）。修法有两条岔路——
  按原计划在 typecheck 拒绝并提示内联（便宜，但删掉 SPEC §7 的成文特性），或
  真正编码成 `define-fun`（`isSorted` 那类非递归纯函数直接可映射，递归另说）。
  ~~无 prime 的 safety 恒真~~ —— 已修，见 §10.5(3)。
  ~~`example` 拒负数~~ —— 已修并做了变异测试闭环，见 §10.5(4)。
- **safety 的全称量化**：作用域按参数名匹配是名字绑定的固有限制，同类型状态
  换个参数名即漏网。真正的解法是给 safety 引入全称量词，属语言层改动，未排期。
- **技能待补**：浮点被建模代码的定点化规范（§10.4）；无测试套件时用只读探针
  产 example 候选的用法（extract-facts 已允许探针，未说明此用途）。
- **源项目侧待办**（不属本仓库）：SUS-001 裁决后写入 `.intent` 并列子句由
  V0020 暴露、补 1–2 组产线真实 SN/productId 到 `example` 块。

---

## 附录 A：反馈条目 → 决策映射

| 反馈条目 | 归宿 | 批次 |
|----------|------|------|
| E1 选域默认建议 | 明确不做（§8） | — |
| E2 无测试时索要真实值 | D10 + D9 第一条 | P1 |
| E3 批量确认跳过裁决 | D7 | P1 |
| E4 文件路径约定 | D8 | P1 |
| W1 交付标准错位 | D1 + D9 | P1 |
| W2 建模粒度（Bool 标志位） | D4（机器抓不到，见 D3 说明） | P1 |
| W3 goal 骨架后置 | D4 | P1 |
| W4 缺少 Bootstrap intent | D2（`--strict` 下拦截）+ D4 | P1 |
| W5 checklist 未强制 | D9 | P1 |
| W6 BEH→intent 映射不完整 | D6 | P1 |
| V1 二进制未分发 | D1 消解其门槛诉求；分发本身见 D12 | P1 / P2 |
| V2 只展示 dominant enum | D3 | P1 |
| V3 coverage「Covered: 0」 | D14 | P3 |
| V4 flowchart 缺无迁移 intent | D15 | P3 |
| V5 viz 输出未版本化 | D12 | P2 |
| C1 E0007 摩擦 | D16 | P3 |
| C2 E0006 摩擦 | D16 | P3 |
| C3 无 facts→intent 辅助 | D6 | P1 |
| §4.3-2 check 输出未挂 goal 列表 | D1 + D2 | P1 |
| X1 多轮补救 | 由 D1 + D9 覆盖 | — |
| X2 易批量 confirmed | D7 | P1 |
| X3 三环未闭合 | §11 跟进项 | — |
| 附录：技能安装路径文档差异 | D17 | P3 |

## 附录 B：复审推翻的初版结论

初版 RFC（同日）经第一性原理复审后，以下五处被推翻或降级，记录于此以免重复论证：

1. **「无 creation 边 = 默认 error」→ 降为 `--strict` 下的 error。**
   初版与 D5「不要逼人发明状态」自相矛盾：它逼出来的是假的 Bootstrap intent，
   与 D5 拒绝的假 phase 迁移是同一类错误。见 P-3。

2. **「未声明 `@lifecycle` 时回退启发式并 warning」→ 改为静默跳过。**
   7/8 的 examples 没有状态 enum，而 transfer / sorting / auth 本来就不该有
   生命周期。对正确的建模报警违反 P-2。

3. **`@lifecycle` 的理由被更正。** 初版称它「同时堵住 V2 和 W2 两个洞」，
   这是编造的因果——用 Bool 标志位建模的人不会去写这个声明，抓不到 W2。
   其真正价值是为状态机检查划定无歧义的适用边界，从而消除假阳性。

4. **`facts_schema: 1` 版本契约 → 降入 P2，代之以 D6 的解析计数自证。**
   schema 尚未稳定就先版本化属 YAGNI；「解析到 N 条」这个数字更便宜也更直接
   地覆盖了「宽松解析静默漏项」这一顾虑。

5. **「DoD ≤ 8 条」→ 改为由 P-4 推导。** 8 是拍的数字。原则应是
   「DoD 只写机器验不了的」，条数随之得出（现为 4 条，其中 1 条带适用条件）。

另有两处补齐：**`intent trace` 的适用条件**（仅存量逆向路径，正向建模无
facts.md）在初版 DoD 中缺失，会形成一条永远过不了的门槛；
**telemetry 与 eval 章节**（§5）初版完全缺失，违反 `AGENT.md` 的项目约定。
