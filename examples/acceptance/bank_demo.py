"""被测系统：约 50 行的内存银行 demo（RFC M-A1 第 6 步）。

自举验收标准：设置环境变量 BANK_DEMO_BUGGY=1 会注入"多扣 1"的 bug
（对应 examples/basics/transfer.intent 的 TransferBuggy），
`intent accept run` 的报告必须把失败归属到 TransferSafe/debit 这条
具体的 ensure 子句上，且退出码非零。
"""

import os


class TransferError(Exception):
    """转账被拒绝（require ... else reject 的实现侧形态）。"""


class Account:
    def __init__(self, owner: str, balance: int, active: bool = True):
        self.owner = owner
        self.balance = balance
        self.active = active

    def __repr__(self) -> str:
        return f"Account({self.owner!r}, balance={self.balance}, active={self.active})"


def transfer(sender: Account, receiver: Account, amount: int) -> None:
    """把 amount 从 sender 转到 receiver。

    业务规则（对应 .intent 的 require ... else reject）：
    金额必须为正、余额必须充足、双方账户必须激活；
    违反任何一条都抛 TransferError 且不动任何状态。
    """
    if amount <= 0:
        raise TransferError(f"amount must be positive, got {amount}")
    if sender.balance < amount:
        raise TransferError(
            f"insufficient funds: balance={sender.balance}, amount={amount}"
        )
    if not (sender.active and receiver.active):
        raise TransferError("both accounts must be active")

    debit = amount
    if os.environ.get("BANK_DEMO_BUGGY") == "1":
        debit = amount + 1  # 埋的 bug：多扣 1

    sender.balance -= debit
    receiver.balance += amount
