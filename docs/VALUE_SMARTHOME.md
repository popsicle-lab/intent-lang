# intent-lang 价值分析：智能家居场景

## 现状痛点

以 Home Assistant 为代表的智能家居自动化现状：

```yaml
# 典型的 HA 自动化：15 行 YAML，关注具体设备和时序
automation:
  - alias: "离家模式"
    trigger:
      - platform: state
        entity_id: person.curtis
        from: "home"
        to: "not_home"
    action:
      - service: light.turn_off
        target: { area_id: living_room }
      - service: light.turn_off
        target: { area_id: bedroom }
      - service: climate.set_hvac_mode
        data: { hvac_mode: "off" }
      - service: lock.lock
        target: { entity_id: lock.front_door }
```

### 5 个根本问题

| 问题 | 描述 | 后果 |
|------|------|------|
| **规则冲突** | 规则 A 说开灯，规则 B 说关灯——谁赢？ | 运行时撞了才知道 |
| **安全漏洞** | "离家锁门"覆盖了所有情况吗？ | 忘了"窗户开着时门锁不了"这种边缘情况 |
| **不可解释** | "为什么灯突然亮了？" | 翻 50 条自动化规则慢慢找 |
| **无法验证** | 新加一条规则会不会破坏已有规则？ | 只能人肉 review |
| **调试困难** | 出了 bug 只能看日志猜 | 没有结构化的因果链 |

---

## intent-lang 的 5 个核心价值

### ① 安全性保证（最核心）

智能家居的 bug 有**物理后果**——门没锁、暖气干烧、水管没关。

```intent
// 用户写了这条新规则
intent PartyMode(home: Home) {
  ensure forall l: Light :: l.on' == true && l.brightness' == 100
  ensure home.frontDoor.locked' == false   // 方便朋友进来
}
```

```bash
$ intent check party.intent

  ❌ PartyMode violates HomeSafety:

     safety HomeSafety:
       invariant !home.occupied ==> home.frontDoor.locked

     If PartyMode executes and then everyone leaves,
     the door remains unlocked.

     Counterexample:
       home.occupied = true → PartyMode executes → home.occupied becomes false
       but home.frontDoor.locked = false (set by PartyMode)
       violates: !occupied ==> locked

     Suggestion: add `require home.occupied` or ensure door relocks on leave
```

**没有 intent-lang**：规则直接生效，某天出门忘关 PartyMode，门整晚没锁。
**有了 intent-lang**：部署前就拦住了。

### ② 可解释性（AI Explainability）

```bash
$ intent why "为什么灯没关？"

  分析当前状态:
    living_room.light1: ON

  活跃意图:
    GoodNight: ensure light.on' == false        → 要求关灯
    SecurityAlert: ensure light1.on' == true    → 要求开灯

  冲突解决:
    SecurityAlert 优先级 > GoodNight
    原因: SecurityAlert 是 safety 规则，自动优先

  触发链:
    22:45 动作传感器触发 → SecurityAlert 激活 → 覆盖 GoodNight

  结论: 灯亮是因为安全警报，不是 bug。
        如果要关灯，请先确认安全警报是否误触发。
```

**对比现状**：Home Assistant 日志只告诉你 "light.turn_on was called by automation X"，但不告诉你：
- 为什么这个 automation 优先于另一个？
- 这是预期行为还是 bug？
- 因果链是什么？

intent-lang 能做到可解释，因为每个决策都有**形式化的因果链**：

```
用户问: 为什么 X 发生了？
  → X 是 intent A 的 ensure 条件触发的
    → intent A 激活是因为其 require 条件满足
      → require 条件满足是因为传感器数据变化
  → intent A 与 intent B 冲突时，A 优先因为它是 safety 规则
```

### ③ 冲突检测（部署前静态分析）

```intent
// 规则 1: 节能 — 没人的房间关灯
intent EnergySaver(room: Room) {
  require !room.hasMotion
  ensure forall l: Light :: l.room == room ==> l.on' == false
}

// 规则 2: 安全 — 走廊常亮
intent HallwayLight(hall: Room) {
  require hall.name == "hallway"
  ensure exists l: Light :: l.room == hall && l.on' == true
}
```

```bash
$ intent check --conflicts

  ⚠️ Conflict: EnergySaver vs HallwayLight

     When: hallway has no motion
       EnergySaver: all lights OFF
       HallwayLight: at least one light ON
     Cannot both be satisfied.

     Suggestions:
     1. @priority(HallwayLight > EnergySaver)
     2. EnergySaver: add `require room.name != "hallway"`
     3. Merge into single intent with conditional logic
```

**现状**：几十条自动化规则，没人知道它们之间有没有冲突。通常撞了才发现。

### ④ 声明式简洁性

```yaml
# Home Assistant: 15 行，关注具体设备和时序
automation:
  - alias: "离家模式"
    trigger: ...
    action:
      - service: light.turn_off
        target: { area_id: living_room }
      - service: light.turn_off
        target: { area_id: bedroom }
      - service: climate.set_hvac_mode ...
      - service: lock.lock ...
      - delay: "00:00:05"
      - condition: ...
```

```intent
// intent-lang: 5 行，只关注目标状态
intent LeaveHome(home: Home) {
  require home.occupied
  ensure home.occupied' == false
  ensure forall l: Light :: l.on' == false
  ensure home.thermostat.mode' == Off
  ensure home.frontDoor.locked' == true
}
// 具体怎么做 → Planner 自动推导
// 设备协议 → Executor 层处理
// 加新设备 → 不需要改 intent
```

### ⑤ 可组合、可演进

```intent
// 自由组合，系统自动验证安全性
intent MovieNight = ArriveHome && DimLights && CloseCurtains

// 加新设备？只声明 action，不修改任何 intent
action TurnOnProjector(proj: Projector) {
  effect proj.on' == true
}
// Planner 自动纳入 MovieNight 的执行计划
```

---

## 总结对比

|  | 传统规则引擎 (HA/HomeKit) | intent-lang |
|---|---|---|
| 用户写什么 | **怎么做** (How) | **要什么** (What) |
| 安全检查 | 无（运行时撞） | 部署前静态验证 |
| 冲突检测 | 无 | SMT 自动分析 |
| 可解释性 | 翻日志猜 | 形式化因果链 |
| 加新设备 | 改所有相关规则 | 只改绑定，intent 不变 |
| 验证覆盖 | 手动测试 | SMT 穷举 |
| 调试体验 | grep 日志 | `intent why` 结构化回答 |

## 一句话定位

> intent-lang 在智能家居的价值不是"让灯能关"——Home Assistant 已经做到了。
> 而是**让你确信"灯该关的时候一定会关，不该关的时候一定不会关"，并且能解释为什么。**
