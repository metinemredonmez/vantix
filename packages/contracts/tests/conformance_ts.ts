// TS (zod) conformance: golden payload'lar üretilen zod şemalarıyla doğrulanıyor mu?
// Çalıştır: pnpm --filter @vantix/contracts test:ts
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";
import * as C from "../../shared/src/generated/contracts.ts";

const here = dirname(fileURLToPath(import.meta.url));
const golden = resolve(here, "../golden");
const index = JSON.parse(readFileSync(resolve(golden, "index.json"), "utf8")) as {
  samples: { file: string; def: string }[];
};

function subset(sub: unknown, sup: unknown, path: string): void {
  if (sub && typeof sub === "object" && !Array.isArray(sub)) {
    for (const [k, v] of Object.entries(sub)) {
      const sv = (sup as Record<string, unknown>)?.[k];
      if (sv === undefined) throw new Error(`${path}: '${k}' alanı zod çıktısında yok`);
      subset(v, sv, `${path}.${k}`);
    }
  } else if (Array.isArray(sub)) {
    const a = sup as unknown[];
    if (!Array.isArray(a) || a.length !== sub.length) throw new Error(`${path}: dizi uyuşmuyor`);
    sub.forEach((x, i) => subset(x, a[i], `${path}[${i}]`));
  } else if (String(sub) !== String(sup)) {
    throw new Error(`${path}: ${String(sub)} != ${String(sup)}`);
  }
}

let fails = 0;
for (const s of index.samples) {
  const schema = (C as Record<string, { parse: (x: unknown) => unknown }>)[s.def];
  if (!schema) {
    console.log(`FAIL ${s.file} -> ${s.def}: şema yok`);
    fails++;
    continue;
  }
  const raw = JSON.parse(readFileSync(resolve(golden, s.file), "utf8"));
  try {
    const parsed = schema.parse(raw);
    subset(raw, parsed, s.file); // golden alanları korunuyor mu (default eklenenler serbest)
    console.log(`ok   ${s.file.padEnd(32)} -> ${s.def}`);
  } catch (e) {
    console.log(`FAIL ${s.file.padEnd(32)} -> ${s.def}: ${(e as Error).message}`);
    fails++;
  }
}
if (fails) {
  console.log(`\n${fails} conformance hatası`);
  process.exit(1);
}
console.log(`\n${index.samples.length} golden zod ile doğrulandı ✅`);
