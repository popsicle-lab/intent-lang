"""被测系统：工单系统的内存参考实现（对应 examples/requirements/ticket.intent）。

与 bank_demo 同构：纯内存、无副作用外依赖，供 `intent accept` 管线验收。
枚举以字符串表示（与 codegen 的 py_value 一致：枚举变体渲染为字符串字面量）。

业务规则对应 .intent 的 `require ... else reject`：违反任何前置条件都抛
TicketError 且不改动任何状态（D3：拒绝 ⇒ 状态不变）。ensure 对应 mutate。
"""


class TicketError(Exception):
    """操作被拒绝（require ... else reject 的实现侧形态）。"""


class Customer:
    def __init__(self, id=0, openTicketCount=0, urgentTicketCount=0):
        self.id = id
        self.openTicketCount = openTicketCount
        self.urgentTicketCount = urgentTicketCount


class Order:
    def __init__(
        self,
        id=0,
        ownerId=0,
        daysSinceDelivery=0,
        hasActiveAfterSalesForSku=False,
        nonReturnable=False,
    ):
        self.id = id
        self.ownerId = ownerId
        self.daysSinceDelivery = daysSinceDelivery
        self.hasActiveAfterSalesForSku = hasActiveAfterSalesForSku
        self.nonReturnable = nonReturnable


class Ticket:
    def __init__(
        self,
        id=0,
        customerId=0,
        ticketType="Logistics",
        status="Pending",
        priority="Normal",
        orderId=0,
        needsManualReview=False,
        resolution="Answered",
        rmaNumber=0,
        refundNumber=0,
        replacementOrderId=0,
        assignedAgentId=0,
        skillGroup="AfterSales",
    ):
        self.id = id
        self.customerId = customerId
        self.ticketType = ticketType
        self.status = status
        self.priority = priority
        self.orderId = orderId
        self.needsManualReview = needsManualReview
        self.resolution = resolution
        self.rmaNumber = rmaNumber
        self.refundNumber = refundNumber
        self.replacementOrderId = replacementOrderId
        self.assignedAgentId = assignedAgentId
        self.skillGroup = skillGroup


class Agent:
    def __init__(self, id=0, role="Customer", skillGroup="AfterSales", activeTicketCount=0):
        self.id = id
        self.role = role
        self.skillGroup = skillGroup
        self.activeTicketCount = activeTicketCount


class Rating:
    def __init__(self, ticketId=0, stars=0, submitted=False):
        self.ticketId = ticketId
        self.stars = stars
        self.submitted = submitted


def _require(cond):
    if not cond:
        raise TicketError("requirement violated")


# ── 能力组 1：客户自助售后闭环 ──────────────────────────────


def create_ticket(c, t, o):
    _require(c.openTicketCount < 5)
    _require(t.orderId == 0 or o.ownerId == c.id)
    _require(t.orderId == 0 or not o.hasActiveAfterSalesForSku)
    _require(t.ticketType != "Return" or t.orderId == 0 or o.daysSinceDelivery <= 7)
    _require(t.ticketType != "Quality" or t.orderId == 0 or o.daysSinceDelivery <= 15)
    _require(
        t.orderId == 0
        or not o.nonReturnable
        or t.ticketType not in ("Return", "RefundOnly", "Quality")
    )

    t.status = "Pending"
    t.priority = "Normal"
    t.needsManualReview = False
    t.skillGroup = "PreSales" if t.ticketType == "PreSale" else "AfterSales"
    c.openTicketCount = c.openTicketCount + 1


def create_ticket_soft_review(c, t, o):
    _require(c.openTicketCount < 5)
    _require(t.ticketType == "RefundOnly")
    _require(t.orderId > 0)
    _require(o.ownerId == c.id)
    _require(o.daysSinceDelivery > 1)

    t.status = "Pending"
    t.needsManualReview = True
    t.priority = "Normal"
    t.skillGroup = "AfterSales"
    c.openTicketCount = c.openTicketCount + 1


# ── 能力组 2：客服流转与协作 ────────────────────────────────


def assign_ticket(t, a):
    _require(t.status == "Pending")
    _require(a.role == "Agent")
    _require(t.skillGroup == a.skillGroup)

    t.status = "InProgress"
    t.assignedAgentId = a.id
    a.activeTicketCount = a.activeTicketCount + 1


def agent_public_reply(t, a):
    _require(t.assignedAgentId == a.id)
    _require(a.role == "Agent" or a.role == "Supervisor")
    _require(t.status != "Closed" and t.status != "Cancelled")

    if t.status == "Pending":
        t.status = "InProgress"
    # 否则保持原状态


def resolve_ticket(t, a):
    _require(t.assignedAgentId == a.id or a.role == "Supervisor")
    _require(t.status == "InProgress" or t.status == "AwaitingCustomer")

    _require(t.resolution != "ReturnInitiated" or t.rmaNumber > 0)
    _require(t.resolution != "RefundInitiated" or t.refundNumber > 0)
    _require(t.resolution != "Replacement" or t.replacementOrderId > 0)

    _require(
        t.ticketType != "Return"
        or t.resolution in ("ReturnInitiated", "Unsupported")
    )
    _require(
        t.ticketType != "RefundOnly"
        or t.resolution in ("RefundInitiated", "Unsupported")
    )
    _require(
        t.ticketType != "Quality"
        or t.resolution in ("RefundInitiated", "Replacement", "Unsupported")
    )
    _require(
        t.ticketType != "Logistics"
        or t.resolution in ("Answered", "Unsupported")
    )
    _require(
        t.ticketType != "PreSale"
        or t.resolution in ("Answered", "Unsupported")
    )

    t.status = "Resolved"


def customer_confirm_resolved(t, c):
    _require(t.customerId == c.id)
    _require(t.status == "Resolved")

    t.status = "Closed"
    c.openTicketCount = c.openTicketCount - 1


def customer_reject_resolved(t, c):
    _require(t.customerId == c.id)
    _require(t.status == "Resolved")

    t.status = "InProgress"


def cancel_ticket(t, c):
    _require(t.customerId == c.id)
    _require(t.status == "Pending" or t.status == "InProgress")

    t.status = "Cancelled"
    c.openTicketCount = c.openTicketCount - 1


def customer_request_urgent(t, c):
    _require(t.customerId == c.id)
    _require(t.status in ("Pending", "InProgress", "AwaitingCustomer"))
    _require(c.urgentTicketCount < 1)

    t.priority = "Urgent"
    c.urgentTicketCount = c.urgentTicketCount + 1


def supervisor_set_critical(t, s):
    _require(s.role == "Supervisor")
    _require(t.status != "Closed" and t.status != "Cancelled")

    t.priority = "Critical"


def auto_close_resolved(t, c):
    _require(t.customerId == c.id)
    _require(t.status == "Resolved")

    t.status = "Closed"
    c.openTicketCount = c.openTicketCount - 1


def set_awaiting_customer(t, a):
    _require(t.assignedAgentId == a.id)
    _require(t.status == "InProgress")

    t.status = "AwaitingCustomer"


def customer_reply_during_wait(t, c):
    _require(t.customerId == c.id)
    _require(t.status == "AwaitingCustomer")

    t.status = "InProgress"


def transfer_ticket(t, source, target):
    """转派：需求名 `from`（Python 保留字），此处形参取名 source。

    注意：acceptance codegen 用需求形参名直接生成局部变量，`from` 会产出
    非法 Python，故本 op 未在 binding 中映射（详见 bind.toml 注释）。
    """
    _require(t.assignedAgentId == source.id)
    _require(t.status == "InProgress" or t.status == "AwaitingCustomer")

    t.status = "Pending"
    t.assignedAgentId = 0
    t.skillGroup = target
    source.activeTicketCount = source.activeTicketCount - 1


# ── 能力组 3：满意度反馈 ────────────────────────────────────


def submit_rating(t, r):
    _require(t.status == "Closed")
    _require(r.stars >= 1 and r.stars <= 5)
    _require(not r.submitted)

    r.submitted = True
    # r.stars 保持不变
