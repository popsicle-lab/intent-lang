# 从 PRD 到验证：intent-lang 在软件开发中的完整链路

## 当前的断裂链

```
PRD                 设计                代码                测试               生产
"余额不足          设计文档             if balance >=       test_transfer()    用户投诉:
 不能转账"          (可能过时)           amount { ... }     (覆盖3个场景)       "余额为0
                                                                              竟然能转账"
     │                 │                    │                   │                  │
     └─────── 没有形式化链接 ──────────────────────────────────────────────────────┘
```

**问题根源**：
1. PRD 说"余额不足"——什么叫不足？等于 0 算不算？
2. 设计文档写了，但代码没完全照着写
3. 测试只覆盖了 happy path
4. 没人验证"代码是否满足 PRD 的所有要求"

---

## intent-lang 补齐的 4 个阶段

### Phase 1: PRD → L1 业务意图

把模糊的自然语言变成精确的形式化声明。

```
PRD 原文:
  "用户转账时，如果余额不足则拒绝转账。
   转账金额必须大于零。系统需要保证资金安全。"
         │
         │ LLM 翻译 + 人工审查
         ▼
intent TransferSafe(sender: Account, receiver: Account, amount: Int) {
  require amount > 0                    ← "金额必须大于零"
  require sender.balance >= amount      ← "余额不足则拒绝"
  ensure sender.balance' == sender.balance - amount
  ensure receiver.balance' == receiver.balance + amount
  invariant sender.balance' >= 0        ← "资金安全"
}
```

**关键价值：形式化过程暴露 PRD 的遗漏和歧义。**

```bash
$ intent check --audit-prd transfer.intent

  ⚠️ PRD 覆盖分析:

  未覆盖的边界情况:
    1. sender == receiver 时是否允许？（PRD 未提及）
    2. amount 是否有上限？（PRD 说"大于零"但没说上限）
    3. receiver 账户被冻结时怎么办？（PRD 未提及）
    4. 并发转账时余额检查有 TOCTOU 风险？（PRD 未提及）

  建议补充:
    + require sender != receiver
    + require amount <= MAX_TRANSFER
    + require receiver.active
```

intent-lang 不替代 PRD，而是**用形式化过程倒逼 PRD 变得精确**。

### Phase 2: L1 → L2 系统意图（API 契约）

业务意图不关心 HTTP/认证，系统意图要关心：

```intent
// L1: 纯业务
intent TransferSafe(sender: Account, receiver: Account, amount: Int) {
  require amount > 0
  require sender.balance >= amount
  ensure sender.balance' == sender.balance - amount
}

// L2: API 层（多了认证、错误码、幂等性）
intent POST_Transfer(req: TransferRequest) -> TransferResponse {
  require req.auth.valid
  require req.idempotency_key.unique

  ensure response.status == 200 ==>
    TransferSafe(account(req.sender_id), account(req.receiver_id), req.amount)

  ensure response.status == 400 ==>
    unchanged(all_accounts)                 // 失败不改状态

  ensure response.status == 401 ==>
    !req.auth.valid
}

// 精化证明
refines TransferSafe by POST_Transfer when response.status == 200
```

```bash
$ intent check --refinement
  ✅ POST_Transfer refines TransferSafe (when status=200)
  ✅ Failure cases are side-effect-free
```

### Phase 3: L2 → L3 组件意图

```intent
intent DeductBalance(account_id: String, amount: Int) {
  require amount > 0
  require db.get(account_id).balance >= amount

  ensure db.get(account_id).balance' ==
         db.get(account_id).balance - amount

  ensure db.get(account_id).version' ==
         db.get(account_id).version + 1     // 乐观锁

  ensure failed ==> unchanged(db)           // 原子性
}
```

### Phase 4: 从意图生成下游产物

#### 4a. 生成测试用例

```bash
$ intent test-gen TransferSafe --format pytest --cases 50
```

```python
# Auto-generated from intent TransferSafe

def test_transfer_normal():
    """require: amount > 0 AND balance >= amount"""
    sender = Account(balance=100)
    result = transfer(sender, receiver, amount=50)
    assert result.sender.balance == 50

def test_transfer_exact_balance():
    """boundary: balance == amount"""
    sender = Account(balance=50)
    result = transfer(sender, receiver, amount=50)
    assert result.sender.balance == 0

def test_transfer_insufficient():
    """violates: require sender.balance >= amount"""
    sender = Account(balance=30)
    with pytest.raises(InsufficientBalance):
        transfer(sender, receiver, amount=50)

def test_transfer_zero_amount():
    """violates: require amount > 0"""
    with pytest.raises(InvalidAmount):
        transfer(sender, receiver, amount=0)

# ... 46 more: boundary, random, adversarial
```

#### 4b. 生成运行时断言

```bash
$ intent export TransferSafe --format rust-assert
```

```rust
fn transfer(sender: &mut Account, receiver: &mut Account, amount: i64) -> Result<()> {
    // --- require ---
    assert!(amount > 0, "require: amount > 0");
    assert!(sender.balance >= amount, "require: sender.balance >= amount");

    let old_sender = sender.balance;
    let old_receiver = receiver.balance;

    // ... your implementation ...

    // --- ensure ---
    assert_eq!(sender.balance, old_sender - amount);
    assert_eq!(receiver.balance, old_receiver + amount);

    // --- invariant ---
    assert!(sender.balance >= 0);

    Ok(())
}
```

#### 4c. 生成 API 契约

```bash
$ intent export POST_Transfer --format openapi
```

```yaml
paths:
  /transfer:
    post:
      x-intent: POST_Transfer
      x-requires: [auth.valid, amount > 0, sender.balance >= amount]
      requestBody:
        content:
          application/json:
            schema:
              required: [sender_id, receiver_id, amount]
              properties:
                amount: { type: integer, minimum: 1 }
      responses:
        200: { description: "Transfer successful" }
        400: { description: "Bad request (no state mutation)" }
        401: { description: "Unauthorized" }
```

#### 4d. CI/CD 集成

```yaml
# .github/workflows/intent-check.yml
name: Intent Verification
on: [push, pull_request]
jobs:
  verify:
    steps:
      - run: intent check specs/              # 验证所有意图
      - run: intent check --refinement specs/  # 验证精化关系
      - run: intent coverage --tests tests/ --intents specs/  # 测试覆盖
      - run: intent audit --prd docs/prd.md --intents specs/  # PRD 覆盖
```

---

## 完整链路图

```
PRD (自然语言)
  │
  │ ① LLM 翻译 + 人工审查
  │   → 暴露 PRD 遗漏/歧义 → 反馈给产品经理
  ▼
L1 业务意图 ──── SMT 验证自洽性 ✅
  │
  │ ② 精化 (refines)
  │   → 验证 API 满足业务意图
  ▼
L2 系统意图 ──── SMT 验证精化 ✅
  │
  │ ③ 精化 (refines)
  │   → 验证组件满足系统意图
  ▼
L3 组件意图 ──── SMT 验证精化 ✅
  │
  │ ④ 生成下游产物
  │
  ├──→ 测试用例 (pytest/jest/go test)
  ├──→ 运行时断言 (Rust/Go/Java)
  ├──→ API 契约 (OpenAPI/gRPC)
  ├──→ 代码骨架 (签名 + pre/post check)
  └──→ CI 验证 (每次提交自动检查)

  每一步都有形式化验证，断裂链变成验证链。
```

---

## 对比：有无 intent-lang

| 阶段 | 没有 intent-lang | 有 intent-lang |
|------|------------------|----------------|
| PRD 评审 | 靠人肉发现遗漏 | **SMT 自动暴露边界情况** |
| 设计评审 | review 文档 | **验证 L2 refines L1** |
| 编码 | 凭理解写代码 | **生成骨架 + 断言** |
| 测试 | 手写测试用例 | **自动生成边界测试** |
| Code Review | 人肉检查逻辑 | **CI 自动验证** |
| 上线后 | 用户投诉 | **运行时断言立即告警** |
| 需求变更 | 全链路人肉检查 | **`intent check` 显示哪些意图被破坏** |
