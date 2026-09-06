import { test } from "node:test";
import assert from "node:assert/strict";
import { marketable } from "./match.ts";

test("BUY limit fills when last <= limit", () => {
  assert.equal(marketable("BUY", 125, 125), true);
  assert.equal(marketable("BUY", 125, 124), true);
  assert.equal(marketable("BUY", 125, 126), false);
});

test("SELL limit fills when last >= limit", () => {
  assert.equal(marketable("SELL", 130, 130), true);
  assert.equal(marketable("SELL", 130, 131), true);
  assert.equal(marketable("SELL", 130, 129), false);
});
