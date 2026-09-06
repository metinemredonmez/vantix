import "reflect-metadata";
import { NestFactory } from "@nestjs/core";
import { FastifyAdapter, NestFastifyApplication } from "@nestjs/platform-fastify";
import { WsAdapter } from "@nestjs/platform-ws";
import { AppModule } from "./app.module";

async function bootstrap() {
  const app = await NestFactory.create<NestFastifyApplication>(AppModule, new FastifyAdapter());
  app.enableCors();
  app.useWebSocketAdapter(new WsAdapter(app)); // ws://<host>/ws/orders
  await app.listen(Number(process.env.PORT ?? 4000), "0.0.0.0");
  console.log(`Vantix API listening on :${process.env.PORT ?? 4000} (ws: /ws/orders)`);
}
bootstrap();
