# 智能家居平台架构对比

## 各平台执行链路

### Amazon Alexa

```
"Alexa, good night"
       │
       ▼
┌─────────────┐
│ ASR 语音识别  │  云端 Alexa Voice Service
└──────┬──────┘
       │
       ▼
┌─────────────┐
│ NLU 意图识别  │  Alexa Skills Kit
│              │  识别为 Intent + Slots
│              │  { intent: "GoodNight", slots: {} }
└──────┬──────┘
       │
       ▼
┌─────────────┐
│ Routine 引擎 │  顺序执行 action list
│              │
│  action 1: light.turn_off(living_room)
│  action 2: light.turn_off(bedroom)
│  action 3: thermostat.set(22)
│  action 4: lock.lock(front_door)
└──────┬──────┘
       │
       ▼
┌─────────────┐
│ Smart Home   │  标准化指令 (Directives):
│ Skill API    │  TurnOnRequest / TurnOffRequest
│              │  SetTargetTemperatureRequest
│              │  LockRequest
└──────┬──────┘
       │ HTTPS → 厂商云
       ▼
┌─────────────┐
│ 设备厂商云    │  Philips Hue / August / etc.
└──────┬──────┘
       │ Zigbee / WiFi / BLE
       ▼
     💡🔒🌡️
```

**特点**：
- "Intent" 是 NLU 概念（槽位填充），不是逻辑意图
- Routine = 顺序 action list
- 全云端链路
- 无冲突检测、无安全验证、无可解释性

### 米家 (Xiaomi Mi Home)

```
"小爱同学，晚安"
       │
       ▼
┌─────────────┐
│ 小爱 NLU     │  小米云端，支持快捷指令绑定
└──────┬──────┘
       │
       ▼
┌───────────────┐
│ 智能场景引擎    │
│               │
│ 手动场景:      │  语音/点击触发 → 执行 action list
│ 自动场景:      │  if 条件(与/或) then 动作
│               │
│ 支持设备联动（A 状态变化触发 B 动作）
│ 不支持量词、不变量、冲突检测
└──────┬───────┘
       │
       ▼
┌─────────────┐
│ MIoT 协议     │  设备建模:
│              │  服务(siid) → 属性(piid) / 动作(aiid)
│              │  例: 灯(siid=2).开关(piid=1).亮度(piid=2)
│              │  指令: set_properties [{siid:2,piid:1,value:false}]
└──────┬──────┘
       │ 米家网关（本地 Zigbee/BLE）或 WiFi 直连
       ▼
     💡🔒🌡️
```

**特点**：
- MIoT 协议对设备建模较结构化
- 场景引擎是 if-then 规则
- 本地网关执行（延迟低）
- 有设备联动，但是命令式编排

### Yandex Alice

```
"Алиса, спокойной ночи"
       │
       ▼
┌─────────────┐
│ Alice NLU    │  Yandex 云端
└──────┬──────┘
       │
       ▼
┌─────────────┐
│ Quasar 平台   │  场景 = 触发条件 + 动作列表
│              │  标准化 Capability 模型:
│              │    on_off / range / color_setting
└──────┬──────┘
       │ HTTPS → 设备厂商 adapter
       ▼
     💡🔒🌡️
```

**特点**：设备能力标准化较好，但仍是 action list 模式。

### Apple HomeKit

```
"Hey Siri, good night"
       │
       ▼
┌─────────────┐
│ Siri NLU     │  本地 + 云端混合
└──────┬──────┘
       │
       ▼
┌─────────────┐
│ Scene 引擎    │  Scene = 目标状态集合 ← 最接近 intent！
│              │    light: off
│              │    thermostat: 22
│              │    door: locked
│              │  Automation = trigger → scene
└──────┬──────┘
       │ HomeKit Accessory Protocol (HAP)
       ▼
     💡🔒🌡️
```

**特点**：Scene 是目标状态（不是动作列表），最接近 intent-lang 的 ensure。但没有 require、invariant、冲突检测。

---

## 横向对比

| 维度 | Alexa | 米家 | Alice | HomeKit | **intent-lang** |
|------|-------|------|-------|---------|-----------------|
| 用户写什么 | action list | if-then + actions | action list | 目标状态 (scene) | **ensure 条件** |
| "意图"含义 | NLU 槽位 | 场景名称 | 场景名称 | Scene 名称 | **形式化命题** |
| 执行方式 | 顺序执行 | 顺序执行 | 顺序执行 | 目标状态设置 | **Planner 推导** |
| 知道目标状态 | ❌ | ❌ | ❌ | ⚠️ 部分 | ✅ |
| 安全验证 | ❌ | ❌ | ❌ | ❌ | ✅ SMT |
| 冲突检测 | ❌ | ⚠️ 重复触发警告 | ❌ | ❌ | ✅ |
| 可解释性 | 日志 | 执行记录 | 日志 | 日志 | ✅ 因果链 |
| 加新设备 | 改 Routine | 改场景 | 改场景 | 改 Scene | 只加 bind |
| 执行后验证 | ❌ | ❌ | ❌ | ❌ | ✅ |

---

## 核心差异：命令式 vs 声明式

所有现有平台（HomeKit 除外）都是命令式的：

```
命令式: "晚安" → 依次执行: 关灯1, 关灯2, 锁门, 调空调
                          ← 固定操作序列，用户必须写清每一步

声明式: "晚安" → 目标: 灯全灭, 门锁上, 温度 22°C
                       ← 目标状态，Planner 自动推导怎么达到
```

| 场景 | 命令式 | 声明式 (intent-lang) |
|------|-------|---------------------|
| 新加一盏灯 | 手动加 "关灯3" 动作 | 不需要改，`forall l: Light` 自动涵盖 |
| 灯已经关了 | 发冗余指令 | Planner 跳过 |
| 门开着要先关再锁 | 手动编排顺序 | 自动推导依赖 |
| 两个场景冲突 | 运行时才发现 | 部署前发现 |

---

## 值得借鉴的点

| 平台 | 借鉴点 | intent-lang 如何吸收 |
|------|--------|---------------------|
| **米家 MIoT** | 设备建模（服务/属性/动作） | 插件 type/action 参考 siid/piid |
| **Alexa** | Directive 标准化 | action 库对齐 Alexa Smart Home API |
| **Alice** | Capability 模型 | type 字段参考 capability 分类 |
| **HomeKit** | Scene = 目标状态 | ensure 就是增强版 Scene |
| **Home Assistant** | 本地执行 + 丰富集成 | Executor 作为 HA integration |

---

## NLU 技术概览

NLU (Natural Language Understanding) 是语音助手理解用户说了什么的模块。

### 技术演进

| 代际 | 技术 | 特点 |
|------|------|------|
| 第一代 (2014-2020) | 规则 + 传统 ML (SVM/LSTM/CRF) | 快、确定、只能理解预定义意图 |
| 第二代 (2020-2024) | 大模型 (GPT/LLaMA) | 理解模糊意图，但慢、贵、不确定 |
| 第三代 (当前) | 混合架构 | 简单指令走传统 NLU，复杂对话走 LLM |

### 各平台 NLU 实现

| 平台 | 技术栈 |
|------|-------|
| Alexa | 自研 NLU + Alexa LLM (2023) |
| 米家/小爱 | 自研 NLU + MiLM 大模型 (2023) |
| Alice | Yandex NLU + YandexGPT |
| Siri | Apple NLU + Apple Intelligence |
| Google Home | Google NLU + Gemini |

**所有平台都是传统 NLU 兜底 + 大模型增强**，因为智能家居要求：
- **快**（500ms 内响应）
- **确定**（"关灯"必须 100% 正确）
- **离线**（网断了基础功能要能用）

### 与 intent-lang 的关系

```
语音 → NLU (大模型/传统ML)  ← 不是 intent-lang 的事
         │
         │ 结构化意图名称
         ▼
    intent-lang              ← 从这里开始
         │
         ├─ 验证安全性
         ├─ Planner 规划
         └─ Executor 执行
```

intent-lang 不做 NLU，它接收 NLU 的结构化输出，负责验证 → 规划 → 执行 → 确认。
