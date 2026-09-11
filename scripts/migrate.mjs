import { readFile } from "node:fs/promises";
import { neon } from "@neondatabase/serverless";
if (!process.env.DATABASE_URL)
  throw new Error("DATABASE_URL is required. Run vercel env pull first.");
const sql = neon(process.env.DATABASE_URL);
const schema = await readFile(
  new URL("../migrations/001_events.sql", import.meta.url),
  "utf8",
);
await sql.transaction(
  schema
    .split(";")
    .map((s) => s.trim())
    .filter(Boolean)
    .map((s) => sql.query(s)),
);
console.log("Leaderboard schema ready.");
