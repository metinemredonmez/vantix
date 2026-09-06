import { Body, Controller, Post } from "@nestjs/common";
import { AuthService } from "./auth.service";

/**
 * v0 auth. register açık (seed/geliştirme için); prod'da admin-only + davet akışına alınmalı.
 * login JWT döner: { sub, tenantId, role }.
 */
@Controller("auth")
export class AuthController {
  constructor(private readonly auth: AuthService) {}

  @Post("register")
  register(@Body() b: { email?: string; password?: string; tenantId?: string; role?: string }) {
    if (!b?.email || !b?.password || !b?.tenantId) {
      return { error: "email, password, tenantId gerekli" };
    }
    return this.auth.register(b.email, b.password, b.tenantId, b.role);
  }

  @Post("login")
  login(@Body() b: { email?: string; password?: string }) {
    return this.auth.login(b?.email ?? "", b?.password ?? "");
  }
}
