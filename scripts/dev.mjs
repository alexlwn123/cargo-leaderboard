import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import leaderboard from "../api/leaderboard.mjs";
import auth from "../api/auth.mjs";
import events from "../api/build-events.mjs";
const files = new Map([
  ["/", ["index.html", "text/html"]],
  ["/account.html", ["account.html", "text/html"]],
  ["/account.js", ["account.js", "text/javascript"]],
  ["/styles.css", ["styles.css", "text/css"]],
  ["/app.js", ["app.js", "text/javascript"]],
  ["/agents.md", ["agents.md", "text/plain"]],
  ["/llms.txt", ["llms.txt", "text/plain"]],
  ["/robots.txt", ["robots.txt", "text/plain"]],
  ["/sitemap.xml", ["sitemap.xml", "application/xml"]],
  ["/install.sh", ["install.sh", "text/plain"]],
  ["/install.ps1", ["install.ps1", "text/plain"]],
  ["/favicon.svg", ["favicon.svg", "image/svg+xml"]],
]);
const server = createServer(async (req, res) => {
  const path = new URL(req.url, "http://localhost").pathname;
  if (path === "/v1/leaderboard" || path === "/api/leaderboard")
    return leaderboard(req, res);
  if (path === "/v1/build-events" || path === "/api/build-events")
    return events(req, res);
  if (path.startsWith("/auth/") || path === "/api/auth") return auth(req, res);
  const file = files.get(path);
  if (!file) {
    res.writeHead(404);
    return res.end("Not found");
  }
  try {
    res.setHeader("Content-Type", file[1] + "; charset=utf-8");
    res.end(await readFile(new URL("../public/" + file[0], import.meta.url)));
  } catch {
    res.writeHead(500);
    res.end("Unable to read page");
  }
});
server.listen(Number(process.env.PORT || 3000), "127.0.0.1", () =>
  console.log(`Local: http://127.0.0.1:${server.address().port}`),
);
