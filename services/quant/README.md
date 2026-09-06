# quant — Python (FastAPI + worker)
uv init; uv add fastapi uvicorn polars numpy scipy cvxpy riskfolio-lib langgraph nats-py psycopg
Görevler: TEFAS/KAP ingestion, factor engine, HRP+CVaR optimizer, backtest, stress test, LangGraph agent'lar, KAP RAG.
Tüm ağır işler NATS job olarak (quant.job.*), sonuç Postgres'e, bildirim WS ile.
