import { validateEvent } from "../web/contract.mjs";
import { submit } from "../web/database.mjs";
import {
  send,
  handleError,
  readEvent,
  authorize,
  clientHash,
} from "../web/http.mjs";
export default async function handler(req, res) {
  if (req.method !== "POST") {
    res.setHeader("Allow", "POST");
    return send(res, 405, { error: "Use POST." });
  }
  try {
    authorize(req);
    const event = validateEvent(await readEvent(req));
    const result = await submit(event, clientHash(req));
    if (result === "limited") {
      res.setHeader("Retry-After", "3600");
      return send(res, 429, {
        error: "Maximum 30 submissions per hour. Try again next hour.",
      });
    }
    send(res, result === "duplicate" ? 200 : 201, {
      event_id: event.event_id,
      status: result,
    });
  } catch (error) {
    handleError(res, error);
  }
}
