// Bağımlılıksız kimlik yardımcıları: scrypt parola hash + HS256 JWT (node:crypto).
// Not: prod'da anahtar rotasyonu + vetted kütüphane değerlendirilmeli; primitifler standart.
import { createHmac, randomBytes, scryptSync, timingSafeEqual } from "crypto";

export function hashPassword(pw: string): string {
  const salt = randomBytes(16);
  const dk = scryptSync(pw, salt, 64);
  return `${salt.toString("hex")}:${dk.toString("hex")}`;
}

export function verifyPassword(pw: string, stored: string): boolean {
  const [s, h] = stored.split(":");
  if (!s || !h) return false;
  const dk = scryptSync(pw, Buffer.from(s, "hex"), 64);
  const hb = Buffer.from(h, "hex");
  return hb.length === dk.length && timingSafeEqual(hb, dk);
}

function b64url(buf: Buffer): string {
  return buf.toString("base64").replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}
function b64urlJson(o: unknown): string {
  return b64url(Buffer.from(JSON.stringify(o)));
}

export interface JwtClaims {
  sub: string;
  tenantId: string;
  role: string;
  iat?: number;
  exp?: number;
}

export function signJwt(payload: Omit<JwtClaims, "iat" | "exp">, secret: string, expiresInSec = 3600): string {
  const now = Math.floor(Date.now() / 1000);
  const body = { ...payload, iat: now, exp: now + expiresInSec };
  const data = `${b64urlJson({ alg: "HS256", typ: "JWT" })}.${b64urlJson(body)}`;
  const sig = b64url(createHmac("sha256", secret).update(data).digest());
  return `${data}.${sig}`;
}

export function verifyJwt(token: string, secret: string): JwtClaims | null {
  const parts = token.split(".");
  if (parts.length !== 3) return null;
  const data = `${parts[0]}.${parts[1]}`;
  const expected = b64url(createHmac("sha256", secret).update(data).digest());
  const a = Buffer.from(expected);
  const b = Buffer.from(parts[2]);
  if (a.length !== b.length || !timingSafeEqual(a, b)) return null;
  try {
    const body = JSON.parse(Buffer.from(parts[1], "base64").toString()) as JwtClaims;
    if (body.exp && body.exp < Math.floor(Date.now() / 1000)) return null;
    return body;
  } catch {
    return null;
  }
}
