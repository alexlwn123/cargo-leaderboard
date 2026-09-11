const definitions = {
  largest_build: {
    title: "Biggest builds",
    description: "Total artifact footprint after a successful build.",
    heading: "Build footprint",
    secondary: "Build time",
    empty: "The heavyweight title is open.",
    hint: "Build your Rust project with the CLI and claim the first spot.",
  },
  largest_clean: {
    title: "Biggest cleans",
    description: "Bytes reclaimed by a successful cargo clean.",
    heading: "Space reclaimed",
    secondary: "Clean time",
    empty: "A clean slate. Literally.",
    hint: "Reclaim some disk space with the CLI and start this board.",
  },
  longest_single_build: {
    title: "Longest waits",
    description: "Wall-clock time for a single successful cargo build.",
    heading: "Build time",
    secondary: "Footprint",
    empty: "No waiting champions. Yet.",
    hint: "Your next successful build could be the one to beat.",
  },
};
const $ = (selector) => document.querySelector(selector);
const initialMetric = new URL(location.href).searchParams.get("metric");
let metric = Object.hasOwn(definitions, initialMetric)
  ? initialMetric
  : "largest_build";
let requestId = 0;
let abort;
function node(tag, className, text) {
  const el = document.createElement(tag);
  if (className) el.className = className;
  if (text !== undefined) el.textContent = text;
  return el;
}
export function bytes(value) {
  if (value === null || value === undefined) return { value: "Unknown", unit: "" };
  const units = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];
  const index =
    value > 0 ? Math.min(5, Math.floor(Math.log(value) / Math.log(1024))) : 0;
  return {
    value: (value / 1024 ** index).toLocaleString("en-US", {
      maximumFractionDigits: index ? 2 : 0,
    }),
    unit: units[index],
  };
}
export function duration(ms) {
  if (ms < 1000) return { value: String(ms), unit: "ms" };
  if (ms < 60_000) return { value: (ms / 1000).toFixed(1), unit: "s" };
  if (ms < 3_600_000)
    return {
      value: `${Math.floor(ms / 60_000)}m ${Math.floor((ms % 60_000) / 1000)}`,
      unit: "s",
    };
  return {
    value: `${Math.floor(ms / 3_600_000)}h ${Math.floor((ms % 3_600_000) / 60_000)}`,
    unit: "m",
  };
}
function score(value) {
  const el = node("span", "score", value.value + " ");
  el.append(node("small", "", value.unit));
  return el;
}
function state(symbol, title, message, action = false) {
  const container = $("#board-state");
  container.hidden = false;
  container.replaceChildren(
    node("span", "state-symbol", symbol),
    node("h2", "", title),
    node("p", "", message),
  );
  if (action) {
    const link = node("a", "button", "Add your project");
    link.href = "#submit";
    container.append(link);
  }
}
function render(entries) {
  const body = $("#entries");
  body.replaceChildren();
  const timeMetric = metric === "longest_single_build";
  const max = entries.length
    ? timeMetric
      ? entries[0].duration_ms
      : entries[0].bytes
    : 1;
  entries.forEach((entry, i) => {
    const row = node("tr");
    row.append(
      node(
        "td",
        `rank${i === 0 ? " first" : ""}`,
        String(i + 1).padStart(2, "0"),
      ),
    );
    const project = node("td");
    project.append(
      node("span", "project-name", entry.repo_slug),
      node("span", "builder", `by ${entry.nickname}`),
    );
    row.append(project);
    const primary = node("td");
    primary.append(
      score(timeMetric ? duration(entry.duration_ms) : bytes(entry.bytes)),
    );
    const bar = node("span", "bar");
    bar.setAttribute("aria-hidden", "true");
    const fill = node("span");
    fill.style.width = `${Math.max(1, ((timeMetric ? entry.duration_ms : entry.bytes) / max) * 100)}%`;
    bar.append(fill);
    primary.append(bar);
    primary.append(
      node(
        "span",
        "metadata",
        entry.file_count == null ? "File count unknown" : `${Number(entry.file_count).toLocaleString()} files`,
      ),
    );
    row.append(primary);
    const secondary = node("td", "secondary");
    const formatted = timeMetric
      ? bytes(entry.bytes)
      : duration(entry.duration_ms);
    secondary.append(
      node("span", "", `${formatted.value} ${formatted.unit}`),
      node("span", "metadata", `${entry.profile} / ${entry.platform}`),
    );
    row.append(secondary);
    const date = node("td", "recorded");
    const time = node(
      "time",
      "",
      new Date(entry.finished_at).toLocaleDateString("en-US", {
        month: "short",
        day: "numeric",
        year: "numeric",
        timeZone: "UTC",
      }),
    );
    time.dateTime = entry.finished_at;
    date.append(time);
    row.append(date);
    // Keep the context accessible on narrow screens where auxiliary columns are hidden.
    project.title = `${entry.profile}; ${entry.platform}; recorded ${entry.finished_at}`;
    body.append(row);
  });
  $("#board-state").hidden = entries.length > 0;
  if (!entries.length)
    state("0", definitions[metric].empty, definitions[metric].hint, true);
  $("#entry-count").textContent =
    `${entries.length === 100 ? "Top 100" : entries.length} ${entries.length === 1 ? "entry" : "entries"}`;
}
async function load() {
  const id = ++requestId;
  abort?.abort();
  abort = new AbortController();
  const definition = definitions[metric];
  document
    .querySelectorAll("[data-metric]")
    .forEach((button) =>
      button.setAttribute(
        "aria-pressed",
        String(button.dataset.metric === metric),
      ),
    );
  $("#metric-description").textContent = definition.description;
  $("#value-heading").textContent = definition.heading;
  $("#secondary-heading").textContent = definition.secondary;
  $("#table-caption").textContent = `${definition.title}, ranked highest first`;
  $("#refresh").disabled = true;
  $("#entries").replaceChildren();
  $("#entry-count").textContent = "Loading…";
  state(
    "…",
    "Weighing the competition.",
    "Fetching the latest community entries.",
  );
  const timeout = setTimeout(() => abort.abort(), 15_000);
  try {
    const response = await fetch(`/v1/leaderboard?metric=${metric}&limit=100`, {
      signal: abort.signal,
    });
    if (!response.ok) throw new Error("Could not load the leaderboard.");
    const payload = await response.json();
    if (id !== requestId) return;
    render(payload.entries);
    return payload;
  } catch {
    if (id !== requestId) return;
    $("#entry-count").textContent = "Unavailable";
    state(
      "!",
      "The board is taking a breather.",
      "Couldn’t load the rankings. Use the refresh button to try again.",
    );
  } finally {
    clearTimeout(timeout);
    if (id === requestId) $("#refresh").disabled = false;
  }
}
document.querySelectorAll("[data-metric]").forEach((button) =>
  button.addEventListener("click", () => {
    metric = button.dataset.metric;
    const url = new URL(location.href);
    url.searchParams.set("metric", metric);
    history.pushState(null, "", url);
    load();
  }),
);
window.addEventListener("popstate", () => {
  const value = new URL(location.href).searchParams.get("metric");
  metric = Object.hasOwn(definitions, value) ? value : "largest_build";
  load();
});
$("#refresh").addEventListener("click", load);
document.querySelectorAll("[data-copy]").forEach((button) =>
  button.addEventListener("click", async () => {
    const target = document.getElementById(button.dataset.copy);
    try {
      await navigator.clipboard.writeText(target.textContent);
      $("#copy-status").textContent = button.dataset.copy === "agent-code"
        ? "Copied. Paste it into your coding agent."
        : "Copied. Paste it into your terminal.";
    } catch {
      $("#copy-status").textContent =
        "Copy unavailable. Select the text and copy it manually.";
    }
  }),
);
const installCommands = {
  unix: "curl -fsSL https://cargo-leaderboard.vercel.app/install.sh -o /tmp/cargo-leaderboard-install.sh &&\nsh /tmp/cargo-leaderboard-install.sh",
  windows: '& {\n  Invoke-WebRequest https://cargo-leaderboard.vercel.app/install.ps1 -OutFile "$env:TEMP\\cargo-leaderboard-install.ps1" -ErrorAction Stop\n  powershell -ExecutionPolicy Bypass -File "$env:TEMP\\cargo-leaderboard-install.ps1"\n}',
  source: "cargo install --git https://github.com/alexlwn123/cargo-leaderboard --tag v0.2.2 --locked",
};
document.querySelectorAll("[data-install]").forEach((button) =>
  button.addEventListener("click", () => {
    document.querySelectorAll("[data-install]").forEach((item) =>
      item.setAttribute("aria-pressed", String(item === button)),
    );
    $("#install-code").textContent = installCommands[button.dataset.install];
    $("#install-method").textContent = button.dataset.install === "source"
      ? "Compile locally. Requires Git and a current stable "
      : "Prebuilt binary, checksum verified. Requires ";
    $("#copy-status").textContent = "";
  }),
);
// Both setup paths use the current board, including self-hosted installations.
$("#agent-code").textContent =
  "Set up Cargo Leaderboard in this Rust project using " + location.origin +
  "/agents.md, then run a build and verify the submission." +
  (location.origin === "https://cargo-leaderboard.vercel.app" ? "" :
    " Use " + location.origin + " as the leaderboard server.");
$("#config-code").textContent = "cargo leaderboard setup" +
  (location.origin === "https://cargo-leaderboard.vercel.app" ? "" :
    " --api-url " + JSON.stringify(location.origin));
load();

// Progressive enhancement: agents can select the same boards as a person can.
if (document.modelContext?.registerTool) {
  try {
    Promise.resolve(
      document.modelContext.registerTool({
        name: "show_leaderboard",
        title: "Show a Cargo leaderboard",
        description:
          "Select a ranking metric, refresh the visible board and return its public entries.",
        inputSchema: {
          type: "object",
          properties: {
            metric: { type: "string", enum: Object.keys(definitions) },
          },
          required: ["metric"],
          additionalProperties: false,
        },
        annotations: { readOnlyHint: false, untrustedContentHint: true },
        async execute(input) {
          if (!input || !Object.hasOwn(definitions, input.metric))
            throw new Error("Unsupported metric");
          metric = input.metric;
          const url = new URL(location.href);
          url.searchParams.set("metric", metric);
          history.pushState(null, "", url);
          const result = await load();
          if (!result)
            throw new Error("Leaderboard unavailable; use refresh to retry.");
          return result;
        },
      }),
    ).catch(() => {});
  } catch {
    /* Browsers without an enabled registry still have the full UI. */
  }
}
