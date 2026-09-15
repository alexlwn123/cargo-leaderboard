import { readFile, access } from "node:fs/promises";
import { spawnSync } from "node:child_process";
const html = await readFile("public/index.html", "utf8");
for (const path of [
  "public/index.html",
  "public/styles.css",
  "public/app.js",
  "public/account.html",
  "public/account.js",
  "public/favicon.svg",
  "public/install.sh",
  "public/install.ps1",
  "public/agents.md",
  "public/llms.txt",
  "public/robots.txt",
  "public/sitemap.xml",
])
  await access(path);
for (const file of [
  "public/app.js",
  "api/leaderboard.mjs",
  "api/build-events.mjs",
  "web/contract.mjs",
  "web/database.mjs",
  "web/http.mjs",
  "web/auth.mjs",
  "api/auth.mjs",
  "public/account.js",
]) {
  const result = spawnSync(process.execPath, ["--check", file], {
    stdio: "inherit",
  });
  if (result.status !== 0) process.exit(1);
}
if (
  !html.includes("<title>Cargo Leaderboard") ||
  !html.includes('id="leaderboard"')
)
  throw new Error("Missing site metadata or leaderboard");
console.log("Static site and API syntax verified.");
