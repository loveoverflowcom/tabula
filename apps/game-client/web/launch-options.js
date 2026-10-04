/* A bounded configuration bridge, not an authority or match-state channel. */
(function (root) {
  "use strict";
  const themes = ["system", "light", "dark", "hc-light", "hc-dark"];
  const clocks = ["untimed", "fischer", "bronstein"];
  const allowed = new Set(["game", "mode", "clock", "initial-ms", "increment-ms", "delay-ms", "theme", "motion", "locale", "chess.clock", "chess.initial-ms", "chess.increment-ms", "chess.delay-ms"]); // xtask-allow-game-id: direct Phase 2 standalone game-client leaf wiring; not platform dispatch.
  function integer(value, fallback, min, max, name) {
    if (value === null || value === undefined) return fallback;
    if (!/^\d+$/.test(String(value))) throw new Error(`${name}: enter a whole number`);
    const parsed = Number(value);
    if (!Number.isSafeInteger(parsed) || parsed < min || parsed > max) throw new Error(`${name}: expected ${min}–${max}`);
    return parsed;
  }
  function parse(search) {
    if (search.length > 4096) throw new Error("Launch configuration is too long");
    const query = new URLSearchParams(search);
    for (const [key] of query) {
      if (!allowed.has(key)) throw new Error(`Unsupported launch option: ${key}`);
      if (query.getAll(key).length !== 1) throw new Error(`Repeated launch option: ${key}`);
    }
    function field(key) {
      if (query.has(key) && query.has(`chess.${key}`)) throw new Error(`Conflicting launch option: ${key}`); // xtask-allow-game-id: direct Phase 2 standalone game-client leaf wiring; not platform dispatch.
      return query.get(key) ?? query.get(`chess.${key}`); // xtask-allow-game-id: direct Phase 2 standalone game-client leaf wiring; not platform dispatch.
    }
    if (query.has("game") && query.get("game") !== "chess") throw new Error("This standalone host supports Chess only"); // xtask-allow-game-id: direct Phase 2 standalone game-client leaf wiring; not platform dispatch.
    if (query.has("mode") && query.get("mode") !== "hot-seat") throw new Error("Only local hot-seat play is available");
    const clock = field("clock") ?? "fischer";
    const theme = query.get("theme") ?? "system";
    const motion = query.get("motion") ?? "system";
    const locale = query.get("locale") ?? "vi";
    if (!clocks.includes(clock)) throw new Error("Unknown clock control");
    if (!themes.includes(theme)) throw new Error("Unknown theme");
    if (!["system", "reduced"].includes(motion)) throw new Error("Unknown motion preference");
    if (!["vi", "en"].includes(locale)) throw new Error("Unknown language");
    return Object.freeze({clock, initialMs: integer(field("initial-ms"), 300000, 1000, 10800000, "Starting time"), incrementMs: integer(field("increment-ms"), 2000, 0, 60000, "Increment"), delayMs: integer(field("delay-ms"), 2000, 0, 60000, "Delay"), theme, motion, locale});
  }
  function resolve(config, media) {
    const dark = media("(prefers-color-scheme: dark)").matches;
    const contrast = media("(prefers-contrast: more)").matches || media("(forced-colors: active)").matches;
    return {...config, resolvedTheme: config.theme === "system" ? (contrast ? (dark ? "hc-dark" : "hc-light") : (dark ? "dark" : "light")) : config.theme, reducedMotion: config.motion === "reduced" || media("(prefers-reduced-motion: reduce)").matches};
  }
  function query(config) {
    const result = new URLSearchParams({game:"chess", mode:"hot-seat", clock:config.clock, theme:config.theme, motion:config.motion, locale:config.locale}); // xtask-allow-game-id: direct Phase 2 standalone game-client leaf wiring; not platform dispatch.
    if (config.clock !== "untimed") {
      result.set("initial-ms", String(config.initialMs));
      result.set(config.clock === "fischer" ? "increment-ms" : "delay-ms", String(config.clock === "fischer" ? config.incrementMs : config.delayMs));
    }
    return result.toString();
  }
  function argumentsFor(config) {
    const result = ["--game", "chess", "--skip-setup", "--clock", config.clock, "--theme", config.resolvedTheme]; // xtask-allow-game-id: direct Phase 2 standalone game-client leaf wiring; not platform dispatch.
    if (config.clock !== "untimed") result.push("--initial-ms", String(config.initialMs), config.clock === "fischer" ? "--increment-ms" : "--delay-ms", String(config.clock === "fischer" ? config.incrementMs : config.delayMs));
    if (config.reducedMotion) result.push("--reduced-motion");
    const text = result.join("\n");
    if (result.length > 64 || new TextEncoder().encode(text).length > 4096) throw new Error("Launch argument budget exceeded");
    return text;
  }
  const api = Object.freeze({parse, resolve, query, argumentsFor, integer});
  root.TabulaLaunch = api;
  if (typeof module !== "undefined" && module.exports) module.exports = api;
})(typeof globalThis !== "undefined" ? globalThis : window);
