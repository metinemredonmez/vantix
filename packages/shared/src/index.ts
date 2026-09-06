// @vantix/shared
//
// TEK KAYNAK sözleşme: `contracts` namespace'i, packages/contracts/schema/exec.schema.json'dan
// ÜRETİLİR (zod). Yeni kod bunu kullanmalı:  import { contracts } from "@vantix/shared";
//                                           contracts.OrderTree.parse(payload)
export * as contracts from "./generated/contracts";

// DEPRECATED — bist-ems'ten taşınan geçici Nest in-memory motorunun kullandığı eski tipler.
// exec-core NATS'a bağlanınca (Sıradaki işler #3) silinecek; yeni kodda KULLANMA.
// Ayrıntı: docs/adr/0003-contracts-single-source.md
export * from "./order";
export * from "./events";
