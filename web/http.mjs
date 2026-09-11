import { createHmac } from "node:crypto";
import { HttpError } from "./contract.mjs";
export function send(res, status, data) {
  res.statusCode = status;
  res.setHeader("Content-Type", "application/json; charset=utf-8");
  res.setHeader("Cache-Control", "no-store");
  res.end(JSON.stringify(data));
}
export function handleError(res, error) {
  if (!(error instanceof HttpError))
    console.error("Leaderboard request failed:", error.message);
  send(res, error instanceof HttpError ? error.status : 503, {
    error:
      error instanceof HttpError
        ? error.message
        : "Leaderboard temporarily unavailable. Please try again.",
  });
}
export async function readEvent(req) {
  if (
    !(req.headers["content-type"] || "")
      .toLowerCase()
      .startsWith("application/json")
  )
    throw new HttpError(415, "Use application/json.");
  if (Number(req.headers["content-length"]) > 16_384)
    throw new HttpError(413, "Event is too large.");
  let raw;
  try {
    // Vercel parses JSON lazily when this property is read.
    raw = req.body;
  } catch (error) {
    if (error?.statusCode === 400 || error instanceof SyntaxError) {
      throw new HttpError(400, "Invalid JSON.");
    }
    throw error;
  }
  if (raw === undefined) {
    const chunks = [];
    let bytes = 0;
    for await (const chunk of req) {
      bytes += Buffer.byteLength(chunk);
      if (bytes > 16_384) throw new HttpError(413, "Event is too large.");
      chunks.push(Buffer.from(chunk));
    }
    raw = Buffer.concat(chunks).toString("utf8");
  }
  if (
    Buffer.byteLength(typeof raw === "string" ? raw : JSON.stringify(raw)) >
    16_384
  )
    throw new HttpError(413, "Event is too large.");
  try {
    return typeof raw === "string" ? JSON.parse(raw) : raw;
  } catch {
    throw new HttpError(400, "Invalid JSON.");
  }
}
export function clientHash(req) {
  // Vercel overwrites x-vercel-forwarded-for. Never trust a user-supplied x-forwarded-for.
  const ip = process.env.VERCEL
    ? req.headers["x-vercel-forwarded-for"]
    : req.socket?.remoteAddress;
  if (process.env.VERCEL && !process.env.RATE_LIMIT_SALT)
    throw new Error("RATE_LIMIT_SALT is required on Vercel");
  return createHmac(
    "sha256",
    process.env.RATE_LIMIT_SALT || "local-development",
  )
    .update(String(ip || "unknown"))
    .digest("hex");
}
