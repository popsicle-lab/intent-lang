# intent-lang 领域插件系统

## 概述

intent-lang 采用**核心 + 插件**架构：核心语言提供领域无关的验证能力，插件提供领域特定的类型、安全规则和公理。

```
┌──────────────────────────────────────────────────────┐
│                  intent-lang core                     │
│                                                       │
│  语法引擎: type, enum, intent, require, ensure,       │
│           invariant, theorem, safety, function        │
│  类型系统: Int, Bool, String, Seq<T>, Set<T>          │
│  验证引擎: VCGen + SMT encoding + Z3                  │
│  LLM 层:  自然语言 → intent 代码                      │
│                                                       │
│  ↑ 这一层永远不变                                      │
└────────────┬────────────────────────────┬─────────────┘
             │ Plugin API                  │
    ┌────────▼─────────┐        ┌─────────▼──────────┐
    │ @domain("finance")│       │ @domain("smarthome")│
    └──────────────────┘        └────────────────────┘
```

---

## 插件的 4 层结构

每个领域插件包含 4 层，每层服务于不同的引擎模块：

| 层 | 内容 | 作用于 | 示例 |
|---|---|---|---|
| **类型层** | 领域数据结构 | Parser + 类型系统 | `Device`, `Account` |
| **安全层** | 全局不变量 | VCGen（自动合并到所有 intent） | `PhysicalConstraints` |
| **公理层** | 领域事实 | SMT（注入为前置假设） | `temp_monotonic` |
| **函数层** | 便捷辅助函数 | 用户代码 | `allLightsOff()` |

---

## 插件示例：智能家居

```intent
// ===== 文件: plugins/smarthome/plugin.intent =====

@plugin("smarthome")
@version("0.1.0")

// -------- 第 1 层：领域类型 --------

type Device {
  id: String
  on: Bool
  room: Room
}

type Light extends Device {
  brightness: Int     // 0-100
  color: Color
}

type Thermostat extends Device {
  temperature: Int
  mode: ThermoMode
  target: Int
}

type Sensor {
  type: SensorType
  value: Int
  room: Room
}

type Room {
  name: String
  devices: Seq<Device>
  sensors: Seq<Sensor>
}

enum ThermoMode { Heat, Cool, Auto, Off }
enum Color { Warm, Cool, Daylight, Custom }
enum SensorType { Motion, Temperature, Humidity, Light }

// -------- 第 2 层：安全规则 --------
// 自动附加到所有使用此插件的 intent

safety PhysicalConstraints {
  invariant forall t: Thermostat :: t.target >= 5 && t.target <= 40
  invariant forall l: Light :: l.brightness >= 0 && l.brightness <= 100
  invariant forall l: Light :: !l.on ==> l.brightness == 0
}

safety EmergencyOverride {
  invariant smokeDetected ==> forall d: Device :: !d.on'
}

// -------- 第 3 层：领域公理 --------
// 告诉 SMT solver 的领域知识

axiom temp_monotonic {
  forall t: Thermostat ::
    t.mode == Heat && t.target > t.temperature ==>
      t.temperature' > t.temperature
}

axiom device_mutex {
  forall r: Room, h: Thermostat, c: Thermostat ::
    h.room == r && c.room == r ==>
      !(h.mode == Heat && c.mode == Cool)
}

// -------- 第 4 层：便捷函数 --------

function allLightsOff(rooms: Seq<Room>) -> Bool {
  forall r: Room, l: Light :: l.room == r ==> !l.on
}

function roomTemp(room: Room) -> Int {
  // 抽象函数，由运行时绑定
}
```

### 用户使用

```intent
import smarthome

intent MovieMode(living: Room) {
  ensure forall l: Light :: l.room == living ==> l.brightness' == 20
  ensure forall d: Device :: d.room == living && d.type == Curtain ==> !d.open'
}
// 验证时自动检查 PhysicalConstraints + EmergencyOverride
```

---

## 插件示例：金融

```intent
@plugin("finance")
@version("0.1.0")

// 类型
type Currency { code: String, decimals: Int }
type Money { amount: Int, currency: Currency }
type Account { id: String, balance: Money, owner: String, frozen: Bool }
type Ledger { entries: Seq<LedgerEntry> }
type LedgerEntry { debit: Account, credit: Account, amount: Money, timestamp: Int }

// 安全规则
safety DoubleEntryBookkeeping {
  invariant forall e: LedgerEntry ::
    e.debit.balance' == e.debit.balance - e.amount.amount &&
    e.credit.balance' == e.credit.balance + e.amount.amount
}

safety AMLCompliance {
  // 单笔交易限额
  invariant forall e: LedgerEntry :: e.amount.amount <= 1000000
}

safety NoOverdraft {
  invariant forall a: Account :: a.balance.amount >= 0
}

// 公理
axiom currency_conversion {
  forall a: Money, b: Money, rate: Int ::
    a.currency != b.currency ==>
      convert(a, b.currency).amount == a.amount * rate / 100
}

// 函数
function netBalance(account: Account, ledger: Ledger) -> Int {
  // 抽象函数
}
```

---

## 插件示例：医疗

```intent
@plugin("healthcare")
@version("0.1.0")

// 类型
type Patient { id: String, weight: Int, age: Int, allergies: Set<String> }
type Medication { name: String, maxDailyDose: Int, contraindications: Set<String> }
type Prescription { patient: Patient, medication: Medication, dose: Int, frequency: Int }

// 安全规则
safety DrugSafety {
  // 不能超过最大日剂量
  invariant forall p: Prescription ::
    p.dose * p.frequency <= p.medication.maxDailyDose

  // 不能给过敏药物
  invariant forall p: Prescription ::
    !(p.medication.name in p.patient.allergies)

  // 不能开禁忌药物组合
  invariant forall p1: Prescription, p2: Prescription ::
    p1.patient == p2.patient ==>
      !(p1.medication.name in p2.medication.contraindications)
}

// 公理
axiom dosage_weight {
  forall p: Patient, m: Medication ::
    safeDose(m, p) <= m.maxDailyDose * p.weight / 70
}
```

---

## 插件示例：访问控制

```intent
@plugin("access-control")
@version("0.1.0")

type Principal { id: String, roles: Set<Role> }
type Resource { id: String, owner: String, classification: Level }
type Permission { action: Action, resource: Resource }

enum Role { Admin, Manager, Developer, Viewer }
enum Action { Read, Write, Delete, Admin }
enum Level { Public, Internal, Confidential, Restricted }

safety LeastPrivilege {
  invariant forall p: Principal, perm: Permission ::
    granted(p, perm) ==> necessaryFor(p, perm)
}

safety SeparationOfDuty {
  invariant forall p: Principal ::
    !(Developer in p.roles && Admin in p.roles)
}

axiom role_hierarchy {
  forall p: Principal ::
    Admin in p.roles ==> Manager in p.roles
}
```

---

## 更多领域（规划中）

```
plugins/
├── finance/           金融 — 交易、合规、复式记账
├── smarthome/         智能家居 — 设备控制、安全联动
├── healthcare/        医疗 — 用药安全、剂量约束
├── access-control/    权限 — RBAC/ABAC 策略
├── automotive/        自动驾驶 — 安全距离、碰撞避免
├── supply-chain/      供应链 — 库存、物流约束
└── compliance/        合规 — GDPR、SOX 等法规规则
```

---

## 插件开发规范

### 目录结构

```
plugins/my-domain/
├── plugin.intent       # 主文件：类型 + 安全规则 + 公理 + 函数
├── plugin.toml         # 元数据（名称、版本、依赖）
├── examples/           # 使用示例
│   └── basic.intent
└── tests/              # 插件自身的验证测试
    └── safety_tests.intent
```

### plugin.toml

```toml
[plugin]
name = "smarthome"
version = "0.1.0"
description = "Smart home device control and safety rules"
authors = ["intent-lang team"]

[dependencies]
# 可依赖其他插件
# access-control = "0.1.0"
```

### 安全注意事项

**公理层是危险的**。错误的公理会让 SMT solver 接受本应失败的验证：

```intent
// ⚠️ 危险：这个公理是错的，会导致所有验证都通过
axiom wrong {
  false ==> true   // 这恒为真，没问题
}

// ⚠️ 更危险：
axiom unsound {
  forall x: Int :: x > 0 && x < 0   // 矛盾！SMT 下一切可证
}
```

建议：
1. 公理必须经过领域专家审核
2. 提供 `intent check --audit-axioms` 命令检查公理一致性
3. 插件发布需要签名 + review

---

## 核心引擎如何处理插件

| 阶段 | 处理 |
|------|------|
| **解析** | `import smarthome` → 加载 `plugins/smarthome/plugin.intent`，解析其中的类型/函数声明加入符号表 |
| **类型检查** | 插件类型与用户代码统一检查 |
| **VCGen** | 插件 `safety` 中的 invariant 自动合并到当前文件所有 intent 的验证条件 |
| **SMT 编码** | 插件 `axiom` 作为 `(assert ...)` 注入每个 SMT 查询的开头 |
| **报告** | 如果 safety 被违反，报告中会标注来源插件 |
