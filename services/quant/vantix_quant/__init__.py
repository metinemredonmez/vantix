"""Vantix Quant/AI servisi.

Deterministik çekirdek (factors, optimize, doctor) kararı verir; LLM (explain) yalnız
profil çıkarımı ve "neden?" açıklaması yapar — hesap yapmaz (CLAUDE.md kuralı).
"""
from . import doctor, factors, models, optimize  # noqa: F401
