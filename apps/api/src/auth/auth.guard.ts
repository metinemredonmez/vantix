import { CanActivate, ExecutionContext, Injectable, UnauthorizedException } from "@nestjs/common";
import { JwtClaims, verifyJwt } from "./crypto";

const SECRET = process.env.JWT_SECRET ?? "change-me";

/** Authorization: Bearer <jwt> doğrular, req.user = claims yapar. */
@Injectable()
export class AuthGuard implements CanActivate {
  canActivate(ctx: ExecutionContext): boolean {
    const req = ctx.switchToHttp().getRequest<{ headers: Record<string, string>; user?: JwtClaims }>();
    const header = req.headers["authorization"] ?? "";
    if (!header.startsWith("Bearer ")) throw new UnauthorizedException("token gerekli");
    const claims = verifyJwt(header.slice(7), SECRET);
    if (!claims) throw new UnauthorizedException("geçersiz/süresi dolmuş token");
    req.user = claims;
    return true;
  }
}
