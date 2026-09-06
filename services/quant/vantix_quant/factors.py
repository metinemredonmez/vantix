"""Faktör motoru: TEFAS NAV serisinden deterministik fon metrikleri + birleşik skor.

Tüm metrikler saf fonksiyon → test edilebilir. NAV = fiyat serisi (np.ndarray, artan zaman).
periods_per_year: günlük NAV için 252.
"""
from __future__ import annotations

import numpy as np

from .models import FundScore

TRADING_DAYS = 252


def simple_returns(nav: np.ndarray) -> np.ndarray:
    nav = np.asarray(nav, dtype=float)
    if nav.size < 2:
        return np.array([])
    return nav[1:] / nav[:-1] - 1.0


def annualized_return(nav: np.ndarray, ppy: int = TRADING_DAYS) -> float:
    nav = np.asarray(nav, dtype=float)
    if nav.size < 2 or nav[0] <= 0:
        return 0.0
    total = nav[-1] / nav[0]
    years = (nav.size - 1) / ppy
    if years <= 0:
        return 0.0
    return float(total ** (1.0 / years) - 1.0)


def annualized_vol(returns: np.ndarray, ppy: int = TRADING_DAYS) -> float:
    if returns.size < 2:
        return 0.0
    return float(np.std(returns, ddof=1) * np.sqrt(ppy))


def max_drawdown(nav: np.ndarray) -> float:
    """En büyük tepe-dip düşüş (pozitif kesir, 0..1)."""
    nav = np.asarray(nav, dtype=float)
    if nav.size == 0:
        return 0.0
    peak = np.maximum.accumulate(nav)
    dd = (nav - peak) / peak
    return float(-dd.min())


def downside_deviation(returns: np.ndarray, mar: float = 0.0, ppy: int = TRADING_DAYS) -> float:
    if returns.size == 0:
        return 0.0
    downside = np.minimum(returns - mar, 0.0)
    return float(np.sqrt(np.mean(downside ** 2)) * np.sqrt(ppy))


def sharpe(returns: np.ndarray, rf: float = 0.0, ppy: int = TRADING_DAYS) -> float:
    if returns.size < 2:
        return 0.0
    excess = returns - rf / ppy
    sd = np.std(excess, ddof=1)
    if sd == 0:
        return 0.0
    return float(np.mean(excess) / sd * np.sqrt(ppy))


def sortino(returns: np.ndarray, rf: float = 0.0, ppy: int = TRADING_DAYS) -> float:
    if returns.size < 2:
        return 0.0
    dd = downside_deviation(returns, mar=rf / ppy, ppy=ppy)
    if dd == 0:
        return 0.0
    ann_excess = np.mean(returns - rf / ppy) * ppy
    return float(ann_excess / dd)


def score_fund(code: str, nav: np.ndarray, ppy: int = TRADING_DAYS) -> FundScore:
    """Birleşik skor: risk-ayarlı getiri, drawdown cezasıyla. Yüksek = iyi.

    score = 0.6*sharpe + 0.4*sortino - 1.5*max_drawdown  (şeffaf, ağırlıklar ayarlanabilir)
    """
    r = simple_returns(nav)
    mdd = max_drawdown(nav)
    shp = sharpe(r, ppy=ppy)
    srt = sortino(r, ppy=ppy)
    score = 0.6 * shp + 0.4 * srt - 1.5 * mdd
    return FundScore(
        code=code,
        ann_return_pct=annualized_return(nav, ppy) * 100,
        ann_vol_pct=annualized_vol(r, ppy) * 100,
        sharpe=shp,
        max_drawdown_pct=mdd * 100,
        sortino=srt,
        score=score,
    )
