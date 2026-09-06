import { Injectable, UnauthorizedException } from "@nestjs/common";
import { PrismaService } from "../prisma/prisma.service";
import { hashPassword, signJwt, verifyPassword } from "./crypto";

const SECRET = process.env.JWT_SECRET ?? "change-me";

@Injectable()
export class AuthService {
  constructor(private readonly prisma: PrismaService) {}

  async register(email: string, password: string, tenantId: string, role = "trader") {
    await this.prisma.tenant.upsert({ where: { id: tenantId }, update: {}, create: { id: tenantId, name: tenantId } });
    const user = await this.prisma.user.create({
      data: { email, passwordHash: hashPassword(password), tenantId, role },
    });
    return { id: user.id, email: user.email, tenantId: user.tenantId, role: user.role };
  }

  async login(email: string, password: string) {
    const u = await this.prisma.user.findUnique({ where: { email } });
    if (!u || !verifyPassword(password, u.passwordHash)) {
      throw new UnauthorizedException("geçersiz e-posta veya parola");
    }
    const token = signJwt({ sub: u.id, tenantId: u.tenantId, role: u.role }, SECRET);
    return { token, tenantId: u.tenantId, role: u.role };
  }
}
