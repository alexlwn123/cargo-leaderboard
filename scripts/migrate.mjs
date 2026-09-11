import { readFile, readdir } from "node:fs/promises";
import { neon } from "@neondatabase/serverless";
import { postgres } from "../web/local-database.mjs";
if (!process.env.DATABASE_URL)
  throw new Error("DATABASE_URL is required. Run vercel env pull first.");
const connection = process.env.DATABASE_URL_UNPOOLED || process.env.DATABASE_URL;
const sql = ["localhost", "127.0.0.1"].includes(new URL(connection).hostname) ? postgres(connection) : neon(connection);
const directory = new URL("../migrations/", import.meta.url);
for (const name of (await readdir(directory)).filter(name => name.endsWith('.sql')).sort()) {
  const schema = await readFile(new URL(name, directory), 'utf8');
  await sql.transaction(schema.split(';').map(s => s.trim()).filter(Boolean).map(s => sql.query(s)));
  console.log(`Applied ${name}`);
}
console.log("Leaderboard schema ready.");

await sql.end?.();
