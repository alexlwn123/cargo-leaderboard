import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import leaderboard from "../api/leaderboard.mjs";
import events from "../api/build-events.mjs";
const files = new Map([
  ["/", ["index.html", "text/html"]],
  ["/styles.css", ["styles.css", "text/css"]],
  ["/app.js", ["app.js", "text/javascript"]],
  ["/favicon.svg", ["favicon.svg", "image/svg+xml"]],
]);
const server = createServer(async (req, res) => {
  const path = new URL(req.url, "http://localhost").pathname;
  if (path === "/v1/leaderboard" || path === "/api/leaderboard")
    return leaderboard(req, res);
  if (path === "/v1/build-events" || path === "/api/build-events")
    return events(req, res);
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
