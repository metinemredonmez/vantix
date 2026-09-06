# OTOMATİK ÜRETİLDİ — elle düzenleme. Kaynak: packages/contracts/schema/exec.schema.json
# Yeniden üret: pnpm --filter @vantix/contracts gen
from __future__ import annotations
from decimal import Decimal
from uuid import UUID
from typing import Annotated, List, Literal, Optional, Union
from pydantic import BaseModel, ConfigDict, Field


Side = Literal["BUY", "SELL"]

NativeType = Literal["LIMIT", "MARKET", "MARKET_TO_LIMIT"]

TimeInForce = Literal["DAY", "IOC", "FOK", "GTD"]

TriggerOn = Literal["FILLED", "PARTIAL"]

CondField = Literal["LAST", "BID", "ASK"]

CondOp = Literal[">", ">=", "<", "<="]

Status = Literal["DRAFT", "PENDING_TRIGGER", "PENDING_RISK", "SENDING", "WORKING", "PARTIALLY_FILLED", "FILLED", "CANCELLING", "CANCELLED", "REJECTED", "EXPIRED", "ERROR"]

class Condition(BaseModel):
    model_config = ConfigDict(extra="forbid", populate_by_name=True)
    symbol: str
    field: CondField
    op: CondOp
    value: Decimal


class Leg(BaseModel):
    model_config = ConfigDict(extra="forbid", populate_by_name=True)
    symbol: str
    side: Side
    qty: Decimal
    price: Optional[Decimal] = None
    native_type: NativeType = "LIMIT"
    tif: TimeInForce = "DAY"
    trigger_price: Optional[Decimal] = None
    trail_amount: Optional[Decimal] = None
    trail_percent: Optional[Decimal] = None
    condition: Optional[Condition] = None
    expire_at_ms: Optional[int] = None


class Edge(BaseModel):
    model_config = ConfigDict(extra="forbid", populate_by_name=True)
    parent: int
    child: int
    on: TriggerOn


class OrderTree(BaseModel):
    model_config = ConfigDict(extra="forbid", populate_by_name=True)
    id: Optional[UUID] = None
    tenant_id: str
    account_id: str
    broker_id: str
    legs: List[Leg]
    tree: List[Edge] = []
    oco_groups: List[List[int]] = []
    client_ref: Optional[str] = None


class CancelRequest(BaseModel):
    model_config = ConfigDict(extra="forbid", populate_by_name=True)
    order_id: UUID
    leg: Optional[int] = None


class BrokerAck(BaseModel):
    model_config = ConfigDict(extra="forbid", populate_by_name=True)
    ems_order_id: str
    broker_order_id: str


class BrokerReject(BaseModel):
    model_config = ConfigDict(extra="forbid", populate_by_name=True)
    ems_order_id: str
    reason: str


class BrokerCancelled(BaseModel):
    model_config = ConfigDict(extra="forbid", populate_by_name=True)
    ems_order_id: str


class ReconcileRequest(BaseModel):
    model_config = ConfigDict(extra="forbid", populate_by_name=True)
    open_broker_ids: List[str]


class ModifyRequest(BaseModel):
    model_config = ConfigDict(extra="forbid", populate_by_name=True)
    order_id: UUID
    leg: int
    price: Optional[Decimal] = None
    qty: Optional[Decimal] = None


class KillRequest(BaseModel):
    model_config = ConfigDict(extra="forbid", populate_by_name=True)
    tenant_id: Optional[str] = None
    account_id: Optional[str] = None
    active: bool


class Fill(BaseModel):
    model_config = ConfigDict(extra="forbid", populate_by_name=True)
    ems_order_id: str
    broker_order_id: str
    qty: Decimal
    price: Decimal
    remaining: Decimal
    ts_ms: int


class Tick(BaseModel):
    model_config = ConfigDict(extra="forbid", populate_by_name=True)
    symbol: str
    last: Decimal
    bid: Optional[Decimal] = None
    ask: Optional[Decimal] = None
    ts_ms: int


class NativeOrder(BaseModel):
    model_config = ConfigDict(extra="forbid", populate_by_name=True)
    ems_order_id: str
    tenant_id: str
    account_id: str
    broker_id: str
    symbol: str
    side: Side
    qty: Decimal
    price: Optional[Decimal] = None
    native_type: NativeType
    tif: TimeInForce


class StateChanged(BaseModel):
    model_config = ConfigDict(extra="forbid", populate_by_name=True)
    type: Literal["state_changed"]
    order_id: UUID
    leg: int
    from_: Status = Field(alias="from")
    to: Status
    reason: Optional[str]


class SendToBroker(BaseModel):
    model_config = ConfigDict(extra="forbid", populate_by_name=True)
    type: Literal["send_to_broker"]
    ems_order_id: str
    tenant_id: str
    account_id: str
    broker_id: str
    symbol: str
    side: Side
    qty: Decimal
    price: Optional[Decimal] = None
    native_type: NativeType
    tif: TimeInForce


class CancelAtBroker(BaseModel):
    model_config = ConfigDict(extra="forbid", populate_by_name=True)
    type: Literal["cancel_at_broker"]
    ems_order_id: str
    broker_order_id: str


class ModifyAtBroker(BaseModel):
    model_config = ConfigDict(extra="forbid", populate_by_name=True)
    type: Literal["modify_at_broker"]
    ems_order_id: str
    broker_order_id: str
    price: Optional[Decimal] = None
    qty: Optional[Decimal] = None


class RiskViolation(BaseModel):
    model_config = ConfigDict(extra="forbid", populate_by_name=True)
    type: Literal["risk_violation"]
    order_id: UUID
    leg: int
    rule: str
    detail: str


class ReconcileMismatch(BaseModel):
    model_config = ConfigDict(extra="forbid", populate_by_name=True)
    type: Literal["reconcile_mismatch"]
    ems_order_id: Optional[str] = None
    broker_order_id: str
    kind: str


class Rejected(BaseModel):
    model_config = ConfigDict(extra="forbid", populate_by_name=True)
    type: Literal["rejected"]
    order_id: UUID
    reason: str


Event = Annotated[Union[StateChanged, SendToBroker, CancelAtBroker, ModifyAtBroker, RiskViolation, Rejected, ReconcileMismatch], Field(discriminator="type")]
