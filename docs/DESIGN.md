# intent-lang 语言设计文档

## 1. 设计理念

**核心主张**：用户声明"意图是什么"（What），系统自动验证"是否逻辑自洽"（Formally Provable），无需编写实现或证明。

**设计选择**：

| 决策 | 选择 | 理由 |
|------|------|------|
| 意图表达 | 结构化声明 + LLM 辅助 | 结构化保证确定性，LLM 降低门槛 |
| 验证方式 | SMT 自动验证 (Z3) | 无需手写证明，自动化程度最高 |
| 实现语言 | Rust | 性能好、WASM 支持、Z3 bindings 成熟 |

**与其他工具的定位对比**：

| 特性 | intent-lang | Dafny | TLA+ | Lean 4 | Alloy |
|------|-------------|-------|------|--------|-------|
| 只写条件，不写实现 | ✅ | ❌ | ⚠️ | ❌ | ✅ |
| SMT 自动验证 | ✅ | ✅ | ❌ | ❌ | ❌ |
| LLM 辅助生成 | ✅ | ❌ | ❌ | ❌ | ❌ |
| 领域插件 | ✅ | ❌ | ❌ | ⚠️ | ❌ |
| Primed 变量 | ✅ | ❌ | ✅ | ❌ | ❌ |
| 反例生成 | ✅ | ✅ | ✅ | ❌ | ✅ |
| 学习曲线 | 低 | 中 | 高 | 高 | 中 |

---

## 2. 语法规范

### 2.1 顶层声明

```ebnf
program     ::= declaration*
declaration ::= import_decl | type_decl | enum_decl | function_decl
              | intent_decl | safety_decl | theorem_decl | axiom_decl
```

### 2.2 类型定义

```intent
type Account {
  balance: Int
  owner: String
  active: Bool
}

enum Role { Admin, Editor, Viewer }

type Pair<A, B> {
  first: A
  second: B
}
```

```ebnf
type_decl   ::= "type" IDENT type_params? "{" field_list "}"
type_params ::= "<" IDENT ("," IDENT)* ">"
field_list  ::= (IDENT ":" type_expr)*
type_expr   ::= IDENT type_args? | builtin_type
type_args   ::= "<" type_expr ("," type_expr)* ">"
builtin_type::= "Int" | "Bool" | "String" | "Seq" "<" type_expr ">"
              | "Set" "<" type_expr ">"

enum_decl   ::= "enum" IDENT "{" IDENT ("," IDENT)* "}"
```

### 2.3 意图声明

```intent
intent TransferSafe(sender: Account, receiver: Account, amount: Int) {
  require amount > 0
  require sender.balance >= amount
  ensure sender.balance' == sender.balance - amount
  ensure receiver.balance' == receiver.balance + amount
  invariant sender.balance' >= 0
}
```

```ebnf
intent_decl ::= annotation* "intent" IDENT "(" param_list ")" "{" clause* "}"
clause      ::= "require" expr
              | "ensure" expr
              | "invariant" expr
param_list  ::= param ("," param)*
param       ::= IDENT ":" type_expr
```

**语义**：
- `require P` — 前置条件，调用意图前必须满足
- `ensure Q` — 后置条件，意图执行后必须满足
- `invariant I` — 不变量，执行前后都必须满足
- 验证条件：`(∧ require_i) ∧ (∧ invariant_j) → (∧ ensure_k) ∧ (∧ invariant_j')`

### 2.4 Primed 变量

`x'` 表示变量 `x` 在意图执行后的新值。

```intent
ensure sender.balance' == sender.balance - amount
//     ^^^^^^^^^^^^^^^^    ^^^^^^^^^^^^^^
//     新值（执行后）         旧值（执行前）
```

**规则**：
- Primed 变量只能出现在 `ensure` 和 `invariant` 中
- `x'` 的类型与 `x` 相同
- 嵌套字段支持：`account.balance'`

### 2.5 定理

```intent
theorem TransferPreservesTotal {
  forall s: Account, r: Account, a: Int ::
    TransferSafe(s, r, a) ==>
      s.balance' + r.balance' == s.balance + r.balance
}
```

```ebnf
theorem_decl ::= "theorem" IDENT "{" expr "}"
```

### 2.6 安全规则

全局不变量，所有 intent 都必须满足：

```intent
safety HomeSafety(home: Home) {
  invariant !home.occupied ==> home.frontDoor.locked
  invariant home.frontDoor.open ==> !home.frontDoor.locked
}
```

```ebnf
safety_decl ::= "safety" IDENT "(" param_list ")" "{" invariant_clause* "}"
invariant_clause ::= "invariant" IDENT? ":" ? expr
```

**语义**：safety 中的 invariant 会自动合并到同作用域内所有 intent 的验证条件中。

### 2.7 公理

向 SMT solver 注入领域知识：

```intent
axiom temp_monotonic {
  forall t: Thermostat ::
    t.mode == Heat && t.target > t.temperature ==>
      t.temperature' > t.temperature
}
```

```ebnf
axiom_decl ::= "axiom" IDENT "{" expr "}"
```

**语义**：公理被无条件假设为真，注入 SMT 查询。**注意**：错误的公理会导致 unsound 结果。

### 2.8 纯函数

```intent
function max(a: Int, b: Int) -> Int {
  if a >= b then a else b
}
```

```ebnf
function_decl ::= "function" IDENT "(" param_list ")" "->" type_expr "{" expr "}"
```

### 2.9 导入

```intent
import smarthome
import finance.currency
```

```ebnf
import_decl ::= "import" module_path
module_path ::= IDENT ("." IDENT)*
```

### 2.10 注解

```intent
@source("PRD-2024-Q1", section: "3.2.1")
@priority(P0)
intent PaymentSafe(...) { ... }
```

```ebnf
annotation  ::= "@" IDENT "(" annotation_args? ")"
annotation_args ::= annotation_arg ("," annotation_arg)*
annotation_arg  ::= expr | IDENT ":" expr
```

### 2.11 表达式

```ebnf
expr ::= literal
       | IDENT                          -- 变量引用
       | IDENT "'"                      -- primed 变量
       | expr "." IDENT                 -- 字段访问
       | expr "." IDENT "'"            -- primed 字段
       | "!" expr                       -- 逻辑非
       | "-" expr                       -- 负号
       | expr binop expr                -- 二元运算
       | "if" expr "then" expr "else" expr
       | "forall" typed_vars "::" expr  -- 全称量词
       | "exists" typed_vars "::" expr  -- 存在量词
       | IDENT "(" expr_list ")"        -- 函数/intent 调用
       | expr "[" expr "]"             -- 索引访问
       | "(" expr ")"                  -- 括号

binop ::= "==" | "!=" | "<" | "<=" | ">" | ">="
        | "+" | "-" | "*" | "/" | "%"
        | "&&" | "||" | "==>"

typed_vars ::= typed_var ("," typed_var)*
typed_var  ::= IDENT ":" type_expr
```

**优先级（低→高）**：

| 优先级 | 运算符 | 结合性 |
|--------|--------|--------|
| 1 | `==>` | 右结合 |
| 2 | `\|\|` | 左结合 |
| 3 | `&&` | 左结合 |
| 4 | `==`, `!=` | 无结合 |
| 5 | `<`, `<=`, `>`, `>=` | 无结合 |
| 6 | `+`, `-` | 左结合 |
| 7 | `*`, `/`, `%` | 左结合 |
| 8 | `!`, `-` (一元) | 前缀 |
| 9 | `.`, `'`, `[]` | 后缀 |

---

## 3. 多层意图体系

### 3.1 从 PRD 到代码的意图分层

```
L0  用户故事    "作为买家，我想安全地完成支付"         ← 自然语言
     │
     ▼  (LLM 翻译)
L1  业务意图    intent PaymentSafe { ... }            ← 业务规则级
     │
     ▼  (refines)
L2  系统意图    intent APIContract { ... }            ← 接口/协议级
     │
     ▼  (refines)
L3  组件意图    intent DeductBalance { ... }          ← 函数/模块级
```

### 3.2 精化关系（Refinement）

低层意图必须**满足**高层意图的所有条件：

```intent
intent PaymentSafe(buyer: User, order: Order) {
  require buyer.balance >= order.total
  ensure buyer.balance' == buyer.balance - order.total
  ensure order.status' == Paid
}

intent DeductBalance(account: Account, amount: Int) {
  require amount > 0
  require account.balance >= amount
  ensure account.balance' == account.balance - amount
}

refines PaymentSafe.deduction by DeductBalance
```

**验证**：SMT 检查 `DeductBalance` 的 ensure 是否蕴含 `PaymentSafe` 对应部分的 ensure。

### 3.3 时序意图（Future Extension）

```intent
workflow OrderLifecycle(order: Order) {
  step Pay {
    require order.status == Created
    ensure order.status' == Paid
  }
  step Ship {
    after Pay
    require order.status == Paid
    ensure order.status' == Shipped
  }
  step Confirm {
    after Ship
    ensure order.status' == Completed
  }
  invariant no_skip: Shipped ==> was(Paid)
}
```

### 3.4 意图组合（Future Extension）

```intent
intent PlaceOrder = compose {
  ValidateStock && DeductBalance && CreateShipment
}
```

---

## 4. 架构设计

```
用户自然语言 / .intent 源码
        │
        ▼
  ┌─────────────┐
  │  LLM 翻译层  │  ← 自然语言 → intent-lang（可选）
  └──────┬──────┘
         │
         ▼
  ┌─────────────┐
  │   Lexer      │  ← logos
  └──────┬──────┘
         │ Token stream
         ▼
  ┌─────────────┐
  │   Parser     │  ← 递归下降 + Pratt parsing
  └──────┬──────┘
         │ AST
         ▼
  ┌─────────────┐
  │  Plugin      │  ← 加载领域插件的类型/safety/axiom
  │  Loader      │
  └──────┬──────┘
         │ Enriched AST
         ▼
  ┌─────────────┐
  │  Type Check  │  ← 类型推导 + 约束检查
  └──────┬──────┘
         │ Typed AST
         ▼
  ┌─────────────┐
  │  VCGen       │  ← 生成验证条件
  │              │     intent: require ∧ invariant → ensure ∧ invariant'
  │              │     theorem: 直接编码
  │              │     safety: 合并到所有 intent
  └──────┬──────┘
         │ Verification Conditions
         ▼
  ┌─────────────┐
  │  SMT Encode  │  ← VC → SMT-LIB2
  │              │     axiom → (assert ...)
  │              │     VC → (assert (not ...))
  └──────┬──────┘
         │ SMT-LIB2 queries
         ▼
  ┌─────────────┐
  │   Z3 Solver  │
  └──────┬──────┘
         │
         ├── unsat → ✅ Verified
         ├── sat   → ❌ Counterexample
         └── unknown → ⚠️ Timeout / Undecidable
```

---

## 5. SMT 编码策略

### 5.1 类型映射

| intent-lang | SMT-LIB2 |
|-------------|----------|
| `Int` | `Int` |
| `Bool` | `Bool` |
| `String` | `String` |
| `struct` | `(declare-datatype ...)` |
| `enum` | `(declare-datatype ...)` 枚举 |
| `Seq<T>` | `(Seq T)` 或 `(Array Int T)` |

### 5.2 Intent 验证编码

对于 `intent I { require R1, R2; ensure E1, E2; invariant V1 }`：

```smt2
; 声明变量
(declare-const sender Account)
(declare-const sender_prime Account)  ; primed

; 假设前置条件和不变量
(assert R1)
(assert R2)
(assert V1)         ; invariant on old state

; 否定后置条件和不变量（反证法）
(assert (not (and E1 E2 V1_prime)))

(check-sat)
; unsat → 验证通过（不可能违反）
; sat   → 找到反例
```

### 5.3 Theorem 验证编码

```smt2
; 直接否定定理体
(assert (not (forall ((s Account) (r Account) (a Int))
  (=> (TransferSafe s r a)
      (= (+ s.balance_prime r.balance_prime)
         (+ s.balance r.balance))))))

(check-sat)
```

---

## 6. 错误报告

采用 rustc 风格的诊断信息：

```
error[E0001]: type mismatch
  --> examples/transfer.intent:12:11
   |
12 |   require amount > "zero"
   |                     ^^^^^^ expected Int, found String
   |

error[V0001]: verification failed
  --> examples/transfer.intent:49:3
   |
49 |   ensure sender.balance' == sender.balance - amount - 1
   |   ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ cannot be verified
   |
   = counterexample:
       sender.balance = 100
       receiver.balance = 50
       amount = 10
     expected: sender.balance' + receiver.balance' == sender.balance + receiver.balance
     got:      (100 - 10 - 1) + (50 + 10) = 149 ≠ 160
```
