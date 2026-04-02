# 从意图到执行：4 层桥接架构

## 问题

intent-lang 验证了"意图逻辑自洽"，但 `ensure light.on' == false` 只是一个逻辑命题——灯不会自己灭。从验证通过到物理世界执行，中间存在鸿沟。

---

## 4 层架构

```
┌──────────────────────────────────────────────────────────┐
│ Layer 1: Intent（意图层）                                  │
│   声明目标状态，SMT 验证安全性                               │
│                                                           │
│   intent GoodNight(home: Home) {                         │
│     ensure forall l: Light :: l.on' == false             │
│     ensure home.frontDoor.locked' == true                │
│   }                                                      │
│                      │ desired state                     │
└──────────────────────┼───────────────────────────────────┘
                       ▼
┌──────────────────────────────────────────────────────────┐
│ Layer 2: Planner（规划层）                                 │
│   从"目标状态"推导"动作序列"                                 │
│                                                           │
│   当前: { light1: on, door: unlocked, door: open }       │
│   目标: { light1: off, door: locked }                    │
│                                                           │
│   计划: Close(door) → Lock(door) ∥ TurnOff(light1)      │
│                      │ action plan                       │
└──────────────────────┼───────────────────────────────────┘
                       ▼
┌──────────────────────────────────────────────────────────┐
│ Layer 3: Executor（执行层）                                │
│   将抽象动作映射到具体设备协议                                │
│                                                           │
│   TurnOff(light1) → MQTT "zigbee2mqtt/light1/set" OFF   │
│   Lock(door) → HTTP POST http://192.168.1.50/api/lock    │
│                      │ actual commands                   │
└──────────────────────┼───────────────────────────────────┘
                       ▼
┌──────────────────────────────────────────────────────────┐
│ Layer 4: Verifier（验证层）                                │
│   读取真实状态，对比 intent 的 ensure 条件                   │
│                                                           │
│   light1.on == false  ✅                                  │
│   door.locked == true ✅                                  │
│   → 意图达成 ✅                                           │
└──────────────────────────────────────────────────────────┘
```

---

## Layer 2: Planner 详细设计

### 设备能力声明（action）

每个设备通过 `action` 声明自己能做什么：

```intent
action TurnOff(light: Light) {
  require light.on == true
  effect light.on' == false
  timeout 5s
  on failure retry(3) then alert("灯关闭失败: {light.id}")
}

action Lock(door: Door) {
  require door.open == false      // 门必须先关上
  effect door.locked' == true
  timeout 10s
  on failure alert("门锁失败！安全风险！")
}

action Close(door: Door) {
  require door.open == true
  effect door.open' == false
}
```

### 规划过程

Planner 的工作类似经典 AI 规划（STRIPS/PDDL）：

```
输入:
  当前状态: { light1.on = true, door.open = true, door.locked = false }
  目标状态: { light1.on = false, door.locked = true }

推理:
  目标 door.locked = true
    → 需要 Lock(door)
    → Lock 的 require: door.open == false
    → 当前 door.open == true，不满足
    → 需要先 Close(door)

输出:
  并行组 1: TurnOff(light1), Close(door)   // 无依赖，可并行
  并行组 2: Lock(door)                     // 依赖 Close(door)
```

**关键特性**：执行计划本身也可以被 SMT 验证——确认这组 action 的 effect 组合起来确实满足 intent 的 ensure。

### 与现有方案的差异

| | 传统规则引擎 | intent-lang Planner |
|---|---|---|
| 用户写 | 具体动作序列 | 目标状态 |
| 加新设备 | 修改所有相关规则 | 只添加 action 声明 |
| 依赖处理 | 手动编排顺序 | 自动推导依赖 |
| 正确性 | 运行时才知道 | 计划阶段就验证 |

---

## Layer 3: Executor 详细设计

### 设备绑定

将抽象类型映射到具体设备：

```toml
# devices.toml
[[devices]]
id = "light1"
type = "Light"
room = "living"
protocol = "mqtt"
topic = "zigbee2mqtt/0x00158d0001a2b3c4/set"
commands = { turnOn = '{"state":"ON"}', turnOff = '{"state":"OFF"}' }
state_topic = "zigbee2mqtt/0x00158d0001a2b3c4"

[[devices]]
id = "front_door"
type = "Door"
protocol = "http"
endpoint = "http://192.168.1.50/api"
commands = { lock = "POST /lock", unlock = "POST /unlock" }
state_endpoint = "GET /status"
```

### 执行策略

- **并行执行**：无依赖的动作并行发送
- **超时重试**：每个 action 可配置 timeout 和 retry 次数
- **失败处理**：retry → 补偿 → 告警 的降级链
- **部分成功**：记录已完成的动作，支持回滚

---

## Layer 4: Runtime Verifier 详细设计

### 从 intent 自动生成运行时断言

```bash
$ intent export GoodNight --format rust-assert
```

生成：

```rust
fn verify_good_night(home: &HomeState) -> VerifyResult {
    let mut failures = vec![];

    for light in &home.lights {
        if light.on {
            failures.push(format!("{} is still on", light.id));
        }
    }

    if !home.front_door.locked {
        failures.push("front door is not locked".into());
    }

    if failures.is_empty() {
        VerifyResult::Satisfied
    } else {
        VerifyResult::Violated(failures)
    }
}
```

### 验证失败处理

```
验证失败
   │
   ├─ 可重试？→ 重新执行失败的 action → 再次验证
   │
   ├─ 部分满足？→ 报告哪些 ensure 满足/不满足
   │
   └─ 完全失败？→ 执行补偿动作 → 恢复到安全状态 → 告警
```

---

## 完整执行时序示例

```
14:00:00  用户: "晚安"
14:00:00  NLU → 识别意图: GoodNight
14:00:00  Layer 1: SMT 预检查 GoodNight vs HomeSafety → ✅
14:00:01  Layer 2: Planner
            当前: {light1: on, light2: on, door: closed, unlocked}
            目标: {lights: off, door: locked}
            计划: [TurnOff(light1) ∥ TurnOff(light2), Lock(door)]
            SMT 验证计划 → ✅
14:00:01  Layer 3: Executor
            MQTT → light1 OFF ... ✅ (0.3s)
            MQTT → light2 OFF ... ✅ (0.5s)
            HTTP → door LOCK  ... ✅ (1.2s)
14:00:03  Layer 4: Verifier
            light1.on == false ✅
            light2.on == false ✅
            door.locked == true ✅
            意图完全达成 ✅
14:00:03  回复: "晚安，灯已关闭，门已锁好 🌙"
```

---

## 跨领域通用性

这 4 层架构不限于智能家居：

| 层 | 智能家居 | 软件开发 | 金融 |
|---|---------|---------|------|
| Intent | ensure light.off | ensure order.paid | ensure balance >= 0 |
| Planner | 推导设备动作 | 推导 API 调用链 | 推导交易步骤 |
| Executor | MQTT/HTTP | REST/gRPC | SWIFT/FIX 协议 |
| Verifier | 读传感器 | 读数据库 | 读对账系统 |
