import { parseQuery } from "../web/contract.mjs";
import { leaderboard } from "../web/database.mjs";
import { send, handleError } from "../web/http.mjs";
export default async function handler(req, res) {
  if (req.method !== "GET") {
    res.setHeader("Allow", "GET");
    return send(res, 405, { error: "Use GET." });
  }
  try {
    const { metric, limit, page } = parseQuery(req.url);
    const payload = await leaderboard(metric, limit, undefined, page);
    res.statusCode = 200;
    res.setHeader("Content-Type", "application/json; charset=utf-8");
    res.setHeader(
      "Cache-Control",
      "public, max-age=0, s-maxage=15, stale-while-revalidate=30",
    );
    res.end(JSON.stringify(payload));
  } catch (error) {
    handleError(res, error);
  }
}
