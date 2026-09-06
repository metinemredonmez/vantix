#!/usr/bin/env node
// Vantix contracts codegen — TEK KAYNAK: schema/exec.schema.json
// JSON Schema (elle yazılan) → zod (TS) + pydantic (Python). Sıfır bağımlılık (yalnız Node stdlib).
// Rust tarafı elle kalır; drift, golden conformance testleriyle yakalanır (bkz. ../README.md).
//
// Kullanım: node codegen/generate.mjs   (paket kökünden: pnpm --filter @vantix/contracts gen)
import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, "..");
const schema = JSON.parse(readFileSync(resolve(root, "schema/exec.schema.json"), "utf8"));
const defs = schema.$defs;
const SCALAR = new Set(["Decimal", "Uuid"]); // named scalar aliaslar (class/enum üretilmez)

const refName = (n) => (n && n.$ref ? n.$ref.split("/").pop() : null);
const isEnum = (n) => Array.isArray(n.enum);
const isObject = (n) => n.type === "object" && n.properties;
const isOneOf = (n) => Array.isArray(n.oneOf);
const nullableInner = (n) => (n.anyOf ? n.anyOf.find((s) => !(s.type === "null")) : null);
const isNullable = (n) => Array.isArray(n.anyOf) && n.anyOf.some((s) => s.type === "null");

// ----------------------------- zod (TypeScript) -----------------------------
function zScalar(name) {
  if (name === "Decimal") return "z.string().regex(/^-?[0-9]+(\\.[0-9]+)?$/, 'decimal string')";
  if (name === "Uuid") return "z.string().uuid()";
  return null;
}
function zNode(n) {
  if (n.$ref) return refName(n); // başka bir emitted const'a referans
  if (n.const !== undefined) return `z.literal(${JSON.stringify(n.const)})`;
  if (n.allOf) {
    let base = zNode(n.allOf[0]);
    if (n.default !== undefined) base += `.default(${JSON.stringify(n.default)})`;
    return base;
  }
  if (n.anyOf) {
    const inner = nullableInner(n);
    let base = zNode(inner);
    if (isNullable(n)) base += ".nullable()";
    return base;
  }
  if (isEnum(n)) return `z.enum(${JSON.stringify(n.enum)})`;
  switch (n.type) {
    case "integer": {
      let s = "z.number().int()";
      if (typeof n.minimum === "number") s += `.min(${n.minimum})`;
      return s;
    }
    case "number":
      return "z.number()";
    case "boolean":
      return "z.boolean()";
    case "string": {
      let s = "z.string()";
      if (n.format === "uuid") s += ".uuid()";
      if (n.pattern) s += `.regex(/${n.pattern}/)`;
      return s;
    }
    case "array": {
      let s = `z.array(${zNode(n.items)})`;
      if (typeof n.minItems === "number") s += `.min(${n.minItems})`;
      return s;
    }
    default:
      return "z.unknown()";
  }
}
function zProp(name, node, required) {
  let s = zNode(node);
  const hasDefault = node.default !== undefined || (node.allOf && node.default !== undefined);
  if (node.default !== undefined && !node.allOf) s += `.default(${JSON.stringify(node.default)})`;
  if (!required.includes(name) && node.default === undefined) s += ".optional()";
  return `  ${JSON.stringify(name)}: ${s},`;
}
function emitZod() {
  const out = [
    "// OTOMATİK ÜRETİLDİ — elle düzenleme. Kaynak: packages/contracts/schema/exec.schema.json",
    "// Yeniden üret: pnpm --filter @vantix/contracts gen",
    'import { z } from "zod";',
    "",
  ];
  for (const [name, node] of Object.entries(defs)) {
    if (SCALAR.has(name)) {
      out.push(`export const ${name} = ${zScalar(name)};`);
      out.push(`export type ${name} = z.infer<typeof ${name}>;`, "");
      continue;
    }
    if (isEnum(node)) {
      out.push(`export const ${name} = z.enum(${JSON.stringify(node.enum)});`);
      out.push(`export type ${name} = z.infer<typeof ${name}>;`, "");
      continue;
    }
    if (isOneOf(node)) {
      const variants = node.oneOf.map(refName);
      const disc = node.discriminator?.propertyName;
      out.push(
        disc
          ? `export const ${name} = z.discriminatedUnion(${JSON.stringify(disc)}, [${variants.join(", ")}]);`
          : `export const ${name} = z.union([${variants.join(", ")}]);`
      );
      out.push(`export type ${name} = z.infer<typeof ${name}>;`, "");
      continue;
    }
    if (isObject(node)) {
      const req = node.required || [];
      out.push(`export const ${name} = z.object({`);
      for (const [pn, pnode] of Object.entries(node.properties)) out.push(zProp(pn, pnode, req));
      out.push(`}).strict();`);
      out.push(`export type ${name} = z.infer<typeof ${name}>;`, "");
    }
  }
  return out.join("\n");
}

// ----------------------------- pydantic (Python) -----------------------------
function pyNode(n) {
  if (n.$ref) {
    const r = refName(n);
    if (r === "Decimal") return "Decimal";
    if (r === "Uuid") return "UUID";
    return r;
  }
  if (n.const !== undefined) return `Literal[${JSON.stringify(n.const)}]`;
  if (n.allOf) return pyNode(n.allOf[0]);
  if (n.anyOf) return `Optional[${pyNode(nullableInner(n))}]`;
  if (isEnum(n)) return `Literal[${n.enum.map((e) => JSON.stringify(e)).join(", ")}]`;
  switch (n.type) {
    case "integer":
      return "int";
    case "number":
      return "float";
    case "boolean":
      return "bool";
    case "string":
      return "str";
    case "array":
      return `List[${pyNode(n.items)}]`;
    default:
      return "object";
  }
}
function pyDefault(node) {
  if (node.default !== undefined) return JSON.stringify(node.default);
  return null;
}
// Python anahtar kelimeleriyle çakışan alan adları alias'lanır (ör. "from").
const PY_KEYWORDS = new Set([
  "from", "import", "class", "def", "return", "global", "in", "is", "and",
  "or", "not", "if", "else", "elif", "for", "while", "lambda", "None", "True",
  "False", "as", "with", "try", "except", "finally", "pass", "raise", "yield",
]);
function pyProp(name, node, required) {
  const t = pyNode(node);
  const def = pyDefault(node);
  const kw = PY_KEYWORDS.has(name);
  const py = kw ? `${name}_` : name;
  const alias = kw ? `alias=${JSON.stringify(name)}` : null;
  const field = (extra) => {
    const parts = [extra, alias].filter(Boolean);
    return `Field(${parts.join(", ")})`;
  };
  if (def !== null) {
    return kw ? `    ${py}: ${t} = ${field(`default=${def}`)}` : `    ${py}: ${t} = ${def}`;
  }
  if (!required.includes(name)) {
    const opt = t.startsWith("Optional[") ? t : `Optional[${t}]`;
    return kw ? `    ${py}: ${opt} = ${field("default=None")}` : `    ${py}: ${opt} = None`;
  }
  return kw ? `    ${py}: ${t} = ${field()}` : `    ${py}: ${t}`;
}
function emitPydantic() {
  const out = [
    "# OTOMATİK ÜRETİLDİ — elle düzenleme. Kaynak: packages/contracts/schema/exec.schema.json",
    "# Yeniden üret: pnpm --filter @vantix/contracts gen",
    "from __future__ import annotations",
    "from decimal import Decimal",
    "from uuid import UUID",
    "from typing import Annotated, List, Literal, Optional, Union",
    "from pydantic import BaseModel, ConfigDict, Field",
    "",
    "",
  ];
  for (const [name, node] of Object.entries(defs)) {
    if (SCALAR.has(name)) continue; // Decimal/UUID stdlib
    if (isEnum(node)) {
      out.push(`${name} = Literal[${node.enum.map((e) => JSON.stringify(e)).join(", ")}]`, "");
      continue;
    }
    if (isOneOf(node)) {
      const variants = node.oneOf.map(refName);
      const disc = node.discriminator?.propertyName;
      out.push(
        disc
          ? `${name} = Annotated[Union[${variants.join(", ")}], Field(discriminator=${JSON.stringify(disc)})]`
          : `${name} = Union[${variants.join(", ")}]`,
        ""
      );
      continue;
    }
    if (isObject(node)) {
      const req = node.required || [];
      out.push(`class ${name}(BaseModel):`);
      out.push(`    model_config = ConfigDict(extra="forbid", populate_by_name=True)`);
      for (const [pn, pnode] of Object.entries(node.properties)) out.push(pyProp(pn, pnode, req));
      out.push("", "");
    }
  }
  return out.join("\n");
}

// ----------------------------- write -----------------------------
const zodOut = resolve(root, "../shared/src/generated/contracts.ts");
const pyOut = resolve(root, "../../services/quant/vantix_contracts/generated.py");
mkdirSync(dirname(zodOut), { recursive: true });
mkdirSync(dirname(pyOut), { recursive: true });
writeFileSync(zodOut, emitZod());
writeFileSync(pyOut, emitPydantic());
console.log("generated:\n  " + zodOut + "\n  " + pyOut);
