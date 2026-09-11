import { validateEvent } from "../web/contract.mjs";
import { submit } from "../web/database.mjs";
import { send, handleError, readEvent, clientHash } from "../web/http.mjs";
import { requireCli, limit } from "../web/auth.mjs";

export function createEventsHandler({ authenticate = requireCli, consume = limit, save = submit } = {}) {
  return async function handler(req, res) {
    if (req.method !== "POST") {
      res.setHeader("Allow", "POST");
      return send(res, 405, { error: "Use POST." });
    }
    try {
      const user = await authenticate(req);
      await consume(`submit-user:${user.github_id}`, 30);
      await consume(`submit-ip:${clientHash(req)}`, 120);
      const input = await readEvent(req);
      // Identity is always server-derived, including for old clients that send a nickname.
      const event = validateEvent({ ...input, nickname: user.github_login });
      const result = await save(event, user);
      send(res, result === "duplicate" ? 200 : 201, { event_id: event.event_id, status: result, github_login: user.github_login });
    } catch (error) {
      if (error.status === 429) res.setHeader("Retry-After", "3600");
      handleError(res, error);
    }
  };
}
export default createEventsHandler();
