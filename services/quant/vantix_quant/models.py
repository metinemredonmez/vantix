"""Quant servis veri modelleri (pydantic). NATS quant.job.* / quant.result.* gövdeleri.

Para/oran değerleri hesap içinde float; dışarıya (emir → exec-core) çıkarken Decimal string'e
çevrilir (exec contracts). Bu modeller quant'ın kendi alanıdır (fon evreni, skor, tahsis).
"""
from __future__ import annotations

from typing import Literal, Optional

from pydantic import BaseModel, Field

RiskTolerance = Literal["conservative", "balanced", "aggressive"]


class RiskProfile(BaseModel):
    """Yatırımcı profili. LLM serbest metinden çıkarabilir; alanlar deterministik motorun girdisi."""
    tolerance: RiskTolerance = "balanced"
    horizon_years: float = Field(default=5, ge=0)
    max_drawdown_pct: Optional[float] = Field(default=None, ge=0, le=100)
    # Yıllık CVaR (%95) bütçesi — tahsisin aşmaması gereken kuyruk risk tavanı (%).
    cvar_budget_pct: Optional[float] = Field(default=None, ge=0)


class FundScore(BaseModel):
    code: str
    ann_return_pct: float
    ann_vol_pct: float
    sharpe: float
    max_drawdown_pct: float
    sortino: float
    score: float  # birleşik skor (yüksek = iyi)


class Allocation(BaseModel):
    weights: dict[str, float]  # fon kodu → ağırlık (toplam ~1.0)
    ann_vol_pct: float
    cvar95_pct: float  # portföyün tarihsel %95 CVaR'ı (pozitif = kayıp)
    method: str = "HRP"
    breaches: list[str] = Field(default_factory=list)  # aşılan bütçeler


class StressResult(BaseModel):
    scenario: str
    pnl_pct: float


class DoctorReport(BaseModel):
    ann_vol_pct: float
    cvar95_pct: float
    max_drawdown_pct: float
    concentration_hhi: float  # 0..1 (1 = tek fonda)
    top_holdings: list[tuple[str, float]]
    stress: list[StressResult]
    flags: list[str]


# --- NATS job/result zarfları (subjects.md: quant.job.<type> / quant.result.<type>) ---
class Job(BaseModel):
    job_id: str
    type: Literal["score", "propose", "doctor"]
    params: dict


class Result(BaseModel):
    job_id: str
    type: str
    result: dict
