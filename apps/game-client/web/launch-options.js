/* A bounded configuration bridge, not an authority or match-state channel. */
(function (root) {
  "use strict";
  const themes = ["system", "light", "dark", "hc-light", "hc-dark"];
  const clocks = ["untimed", "fischer", "bronstein"];
  const allowed = new Set(["game", "mode", "clock", "initial-ms", "increment-ms", "delay-ms", "theme", "motion", "locale", "chess.clock", "chess.initial-ms", "chess.increment-ms", "chess.delay-ms", "source", "return_to", "seats", "initial_ms", "increment_ms", "delay_ms", "match_id"]); // xtask-allow-game-id: direct Phase 2 standalone game-client leaf wiring; not platform dispatch.
  function integer(value, fallback, min, max, name) {
    if (value === null || value === undefined) return fallback;
    if (!/^\d+$/.test(String(value))) throw new Error(`${name}: enter a whole number`);
    const parsed = Number(value);
    if (!Number.isSafeInteger(parsed) || parsed < min || parsed > max) throw new Error(`${name}: expected ${min}–${max}`);
    return parsed;
  }
  // The bounded local leaf supports one registry identity; navigation metadata
  // is never passed through to Rust or accepted as an arbitrary redirect.
  const registryGame = "com.tabula.chess"; // xtask-allow-game-id: direct Phase 2 standalone game-client leaf wiring; not platform dispatch.
  const tabulaReturn = `/games/${registryGame}?setup=1`;
  function navigation(search) {
    if (search.length > 4096) throw new Error("Launch configuration is too long");
    const query = new URLSearchParams(search);
    if (!query.has("source") && !query.has("return_to")) return null;
    if (query.getAll("source").length !== 1 || query.getAll("return_to").length !== 1 || query.get("source") !== "tabula" || query.get("return_to") !== tabulaReturn) throw new Error("Unsupported Tabula return destination");
    return Object.freeze({source:"tabula", returnTo:tabulaReturn});
  }
  function parse(search) {
    if (search.length > 4096) throw new Error("Launch configuration is too long");
    const query = new URLSearchParams(search);
    for (const [key] of query) {
      if (!allowed.has(key)) throw new Error(`Unsupported launch option: ${key}`);
      if (query.getAll(key).length !== 1) throw new Error(`Repeated launch option: ${key}`);
    }
    if (query.get("game") === "werewolf") { // xtask-allow-game-id: ADR-0035 opt-in standalone leaf.
      const keys = new Set(["game","mode","seats","theme","motion","locale"]);
      for (const [key] of query) if (!keys.has(key)) throw new Error(`Unsupported simulator option: ${key}`);
      if (query.has("mode") && query.get("mode") !== "simulator") throw new Error("Only isolated-seat local simulation is available");
      const theme = query.get("theme") ?? "dark", motion = query.get("motion") ?? "system", locale = query.get("locale") ?? "vi";
      if (!themes.includes(theme) || !["system","reduced"].includes(motion) || !["vi","en"].includes(locale)) throw new Error("Invalid simulator preference");
      return Object.freeze({game:"werewolf", mode:"simulator", seats:integer(query.get("seats"),12,6,20,"Seats"), theme,motion,locale}); // xtask-allow-game-id: ADR-0035 opt-in standalone leaf.
    }
    const handoff = navigation(search);
    const online = handoff && query.get("mode") === "network";
    const matchId = query.get("match_id");
    if (online) {
      if (!/^[0-9a-f]{32}$/.test(matchId ?? "") || /^0+$/.test(matchId)) throw new Error("Invalid public match identifier");
      for (const key of ["clock", "initial_ms", "increment_ms", "delay_ms"]) if (query.has(key)) throw new Error("Online configuration belongs to the server");
    } else if (query.has("match_id")) throw new Error("Match identifiers require direct online play");
    if (handoff) {
      for (const key of online ? ["game", "mode", "seats", "locale"] : ["game", "mode", "seats", "clock", "locale"]) if (!query.has(key)) throw new Error(`Missing Tabula launch option: ${key}`);
      if (query.get("game") !== registryGame || !online && query.get("mode") !== "local" || query.get("seats") !== "2") throw new Error("Only two-player local Chess is available"); // xtask-allow-game-id: direct Phase 2 standalone game-client leaf wiring; not platform dispatch.
      for (const key of ["initial-ms", "increment-ms", "delay-ms", "chess.clock", "chess.initial-ms", "chess.increment-ms", "chess.delay-ms"]) if (query.has(key)) throw new Error(`Noncanonical Tabula launch option: ${key}`); // xtask-allow-game-id: direct Phase 2 standalone game-client leaf wiring; not platform dispatch.
      const control = query.get("clock");
      const required = control === "fischer" ? ["initial_ms", "increment_ms"] : control === "bronstein" ? ["initial_ms", "delay_ms"] : [];
      for (const key of ["initial_ms", "increment_ms", "delay_ms"]) {
        if (required.includes(key) !== query.has(key)) throw new Error(`Missing or inapplicable Tabula clock option: ${key}`);
      }
    } else {
      for (const key of ["seats", "initial_ms", "increment_ms", "delay_ms"]) if (query.has(key)) throw new Error(`Unsupported standalone launch option: ${key}`);
      if (query.has("game") && query.get("game") !== "chess") throw new Error("This standalone host supports Chess only"); // xtask-allow-game-id: direct Phase 2 standalone game-client leaf wiring; not platform dispatch.
      if (query.has("mode") && query.get("mode") !== "hot-seat") throw new Error("Only local hot-seat play is available");
    }
    function field(key) {
      if (handoff) return query.get(key.replaceAll("-", "_"));
      if (query.has(key) && query.has(`chess.${key}`)) throw new Error(`Conflicting launch option: ${key}`); // xtask-allow-game-id: direct Phase 2 standalone game-client leaf wiring; not platform dispatch.
      return query.get(key) ?? query.get(`chess.${key}`); // xtask-allow-game-id: direct Phase 2 standalone game-client leaf wiring; not platform dispatch.
    }
    const clock = online ? "untimed" : field("clock") ?? "fischer";
    const theme = query.get("theme") ?? "system";
    const motion = query.get("motion") ?? "system";
    const locale = query.get("locale") ?? "vi";
    if (!clocks.includes(clock)) throw new Error("Unknown clock control");
    if (!themes.includes(theme)) throw new Error("Unknown theme");
    if (!["system", "reduced"].includes(motion)) throw new Error("Unknown motion preference");
    if (!["vi", "en"].includes(locale)) throw new Error("Unknown language");
    return Object.freeze({...handoff, online:Boolean(online), matchId:online ? matchId : null, gameId:registryGame, clock, initialMs: integer(field("initial-ms"), 300000, 1000, 10800000, "Starting time"), incrementMs: integer(field("increment-ms"), 2000, 0, 60000, "Increment"), delayMs: integer(field("delay-ms"), 2000, 0, 60000, "Delay"), theme, motion, locale});
  }
  function resolve(config, media) {
    const dark = media("(prefers-color-scheme: dark)").matches;
    const contrast = media("(prefers-contrast: more)").matches || media("(forced-colors: active)").matches;
    return {...config, resolvedTheme: config.theme === "system" ? (contrast ? (dark ? "hc-dark" : "hc-light") : (dark ? "dark" : "light")) : config.theme, reducedMotion: config.motion === "reduced" || media("(prefers-reduced-motion: reduce)").matches};
  }
  function query(config) {
    if (config.game === "werewolf") return new URLSearchParams({game:config.game,mode:"simulator",seats:String(config.seats),theme:config.theme,motion:config.motion,locale:config.locale}).toString(); // xtask-allow-game-id: ADR-0035 opt-in standalone leaf.

    const result = new URLSearchParams({game:"chess", mode:"hot-seat", clock:config.clock, theme:config.theme, motion:config.motion, locale:config.locale}); // xtask-allow-game-id: direct Phase 2 standalone game-client leaf wiring; not platform dispatch.
    if (config.source === "tabula") {
      result.set("game", registryGame);
      result.set("mode", "local");
      result.set("seats", "2");
      result.set("source", "tabula");
      result.set("return_to", config.returnTo);
    }
    if (config.clock !== "untimed") {
      result.set(config.source === "tabula" ? "initial_ms" : "initial-ms", String(config.initialMs));
      const adjustment = config.clock === "fischer" ? "increment-ms" : "delay-ms";
      result.set(config.source === "tabula" ? adjustment.replaceAll("-", "_") : adjustment, String(config.clock === "fischer" ? config.incrementMs : config.delayMs));
    }
    return result.toString();
  }
  function argumentsFor(config) {
    if (config.game === "werewolf") return ["--seats",String(config.seats),"--theme",config.resolvedTheme].join("\n"); // xtask-allow-game-id: ADR-0035 opt-in standalone leaf.

    const result = ["--game", "chess", "--skip-setup", "--clock", config.clock, "--theme", config.resolvedTheme]; // xtask-allow-game-id: direct Phase 2 standalone game-client leaf wiring; not platform dispatch.
    if (config.clock !== "untimed") result.push("--initial-ms", String(config.initialMs), config.clock === "fischer" ? "--increment-ms" : "--delay-ms", String(config.clock === "fischer" ? config.incrementMs : config.delayMs));
    if (config.online) result.push("--online-match", config.matchId);
    if (config.reducedMotion) result.push("--reduced-motion");
    const text = result.join("\n");
    if (result.length > 64 || new TextEncoder().encode(text).length > 4096) throw new Error("Launch argument budget exceeded");
    return text;
  }
  const api = Object.freeze({parse, navigation, resolve, query, argumentsFor, integer});
  root.TabulaLaunch = api;
  if (typeof module !== "undefined" && module.exports) module.exports = api;
})(typeof globalThis !== "undefined" ? globalThis : window);
