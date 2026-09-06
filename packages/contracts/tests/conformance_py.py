"""Python (pydantic) conformance: golden payload'lar üretilen modellerle doğrulanıyor mu?
Çalıştır: uv run --with pydantic python packages/contracts/tests/conformance_py.py
"""
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "services" / "quant"))

from vantix_contracts import generated as m  # noqa: E402

GOLDEN = ROOT / "packages" / "contracts" / "golden"


def subset(sub, sup, path=""):
    """golden'daki her alan, dump edilmiş çıktıda aynı değerle var mı."""
    if isinstance(sub, dict):
        assert isinstance(sup, dict), f"{path}: obje bekleniyordu"
        for k, v in sub.items():
            assert k in sup, f"{path}: '{k}' alanı çıktıda yok"
            subset(v, sup[k], f"{path}.{k}")
    elif isinstance(sub, list):
        assert isinstance(sup, list) and len(sub) == len(sup), f"{path}: dizi uyuşmuyor"
        for i, (a, b) in enumerate(zip(sub, sup)):
            subset(a, b, f"{path}[{i}]")
    else:
        # Decimal/UUID json moduna string döner; golden zaten string → str karşılaştır
        assert str(sub) == str(sup), f"{path}: {sub!r} != {sup!r}"


def main() -> int:
    index = json.loads((GOLDEN / "index.json").read_text())
    fails = []
    for s in index["samples"]:
        model = getattr(m, s["def"])
        raw = json.loads((GOLDEN / s["file"]).read_text())
        try:
            obj = model.model_validate(raw)
            dumped = obj.model_dump(mode="json", by_alias=True)
            subset(raw, dumped, s["file"])
            print(f"ok   {s['file']:32} -> {s['def']}")
        except Exception as e:  # noqa: BLE001
            fails.append((s["file"], s["def"], repr(e)))
            print(f"FAIL {s['file']:32} -> {s['def']}: {e}")
    if fails:
        print(f"\n{len(fails)} conformance hatası")
        return 1
    print(f"\n{len(index['samples'])} golden pydantic ile doğrulandı ✅")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
