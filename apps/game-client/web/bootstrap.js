/* Macroquad keeps ownership of the canvas, input and frame loop (ADR-011). */
(function () {
  "use strict";
  const byId = (id) => document.getElementById(id);
  let ready = false;
  let failed = false;
  let leaving = false;
  let startupTimer;
  let boardAcknowledged = false;
  let config;
  let setupUrl = "index.html";
  const vi = {loading:"Đang mở bàn cờ…",download:"Đang tải chương trình WebAssembly",starting:"Đang khởi tạo bàn cờ",back:"Về thiết lập",retry:"Thử lại",error:"Không thể mở bàn cờ",restart:"Tải lại sẽ bắt đầu một ván mới. Ván cục bộ không được lưu",leaveTitle:"Rời ván cờ?",leaveDetail:"Ván cục bộ này không được lưu. Bạn có thể ở lại hoặc quay về thiết lập để bắt đầu ván mới",stay:"Ở lại",leave:"Rời ván cờ",help:"Trợ giúp bàn phím",helpTitle:"Điều khiển bàn cờ",helpDetail:"Chạm hoặc nhấp quân rồi ô đích. Dùng phím mũi tên để đổi ô, Enter để chọn, Escape để hủy chọn hoặc phong cấp. Tab chuyển từ ô cuối tới các nút trong ván; Shift+Tab đến nút rời ván. Trình đọc màn hình đầy đủ cho bàn cờ chưa có",closeHelp:"Về bàn cờ"};
  const en = {loading:"Opening your board…",download:"Downloading WebAssembly game",starting:"Starting the Chess board",back:"Back to setup",retry:"Try again",error:"Could not open the board",restart:"Reloading starts a new game. Local games are not saved",leaveTitle:"Leave this game?",leaveDetail:"This local game is not saved. Stay here or return to setup to start a new game",stay:"Stay",leave:"Leave game",help:"Keyboard help",helpTitle:"Board controls",helpDetail:"Tap or click a piece, then its destination. Use arrow keys to move focus, Enter to select, and Escape to cancel selection or promotion. Tab moves from the last square to in-game controls; Shift+Tab reaches the leave button. A complete screen-reader board is not available",closeHelp:"Back to board"}; // xtask-allow-game-id: direct Phase 2 standalone game-client leaf wiring; not platform dispatch.
  let text = vi;
  byId("glcanvas").tabIndex = -1;
  byId("glcanvas").setAttribute("aria-hidden", "true");
  byId("cancel-load").focus();
  function fail(error) {
    if (failed || leaving) return;
    failed = true;
    ready = false;
    clearTimeout(startupTimer);
    // A failed runtime is only restarted through an explicit document reload.
    if (typeof animation_frame_timeout !== "undefined") cancelAnimationFrame(animation_frame_timeout);
    window.blocking_event_loop = true;
    byId("loader").hidden = true;
    byId("runtime-error").hidden = false;
    byId("glcanvas").tabIndex = -1;
    byId("glcanvas").setAttribute("aria-hidden", "true");
    byId("error-detail").textContent = error instanceof Error ? error.message : String(error);
    byId("error-back").focus();
    console.error("Standalone Chess:", error); // xtask-allow-game-id: direct Phase 2 standalone game-client leaf wiring; not platform dispatch.
  }
  function forwardCanvasFocus(focused) {
    if (!ready || failed || leaving || typeof wasm_exports?.focus !== "function") return;
    try {
      wasm_exports.focus(Boolean(focused && document.hasFocus() && document.visibilityState === "visible"));
    } catch (error) { fail(error); }
  }
  byId("glcanvas").addEventListener("blur", () => forwardCanvasFocus(false));
  byId("glcanvas").addEventListener("focus", () => forwardCanvasFocus(true));
  function applyLanguage() {
    const ids = {"loading-title":"loading","loading-status":"download","cancel-load":"back","error-title":"error","restart-notice":"restart","error-back":"back","retry":"retry","leave":"back","help":"help","leave-title":"leaveTitle","leave-detail":"leaveDetail","stay":"stay","confirm-leave":"leave","help-title":"helpTitle","help-detail":"helpDetail","close-help":"closeHelp"};
    for (const [id, key] of Object.entries(ids)) byId(id).textContent = text[key];
    byId("keyboard-help").textContent = text.helpDetail;
    byId("glcanvas").setAttribute("aria-label", text.helpTitle);
    document.documentElement.lang = config.locale;
  }
  byId("retry").addEventListener("click", () => { leaving = true; location.reload(); });
  byId("leave").addEventListener("click", () => { byId("leave-dialog").showModal(); byId("stay").focus(); });
  byId("stay").addEventListener("click", () => byId("leave-dialog").close());
  byId("confirm-leave").addEventListener("click", () => { leaving = true; location.assign(setupUrl); });
  byId("help").addEventListener("click", () => byId("help-dialog").showModal());
  byId("close-help").addEventListener("click", () => byId("help-dialog").close());
  for (const id of ["leave-dialog", "help-dialog"]) byId(id).addEventListener("close", () => { if (ready) byId("glcanvas").focus(); });
  // Ordinary Tab is presenter input (including its in-game HUD). The pinned
  // bundle prevents default Tab; Shift+Tab is the explicit accessible host exit.
  byId("glcanvas").addEventListener("keydown", (event) => {
    if (event.code === "Tab" && event.shiftKey) {
      event.preventDefault();
      event.stopImmediatePropagation();
      byId("leave").focus();
    }
  }, true);
  byId("glcanvas").addEventListener("webglcontextlost", (event) => { event.preventDefault(); fail(new Error(config?.locale === "en" ? "The graphics context was lost. Return to setup or restart a new game." : "Đã mất kết nối đồ họa. Về thiết lập hoặc tải lại để bắt đầu ván mới.")); });
  window.addEventListener("error", (event) => fail(event.error ?? new Error(event.message)));
  window.addEventListener("unhandledrejection", (event) => fail(event.reason));
  window.addEventListener("beforeunload", (event) => { if (ready && !leaving) { event.preventDefault(); event.returnValue = ""; } });
  async function download(url) {
    const response = await fetch(url);
    if (!response.ok) throw new Error(`Game download failed: HTTP ${response.status}`);
    const advertised = Number(response.headers.get("Content-Length"));
    const exactLength = advertised > 0 && !response.headers.get("Content-Encoding");
    if (advertised > 64 * 1024 * 1024) throw new Error("Game artifact exceeds the 64 MiB host limit");
    const progress = byId("load-progress");
    if (exactLength) progress.max = advertised;
    if (!response.body) return new Uint8Array(await response.arrayBuffer());
    const reader = response.body.getReader();
    const chunks = [];
    let received = 0;
    try {
      for (;;) {
        const {done, value} = await reader.read();
        if (done) break;
        received += value.byteLength;
        if (received > 64 * 1024 * 1024) throw new Error("Game artifact exceeds the 64 MiB host limit");
        chunks.push(value);
        if (exactLength && received <= advertised) progress.value = received;
        else progress.removeAttribute("value");
        byId("loading-status").textContent = `${text.download} · ${(received / 1024).toFixed(0)} KiB`;
      }
    } catch (error) { await reader.cancel(); throw error; }
    if (received === 0) throw new Error("The game artifact is empty");
    const result = new Uint8Array(received);
    let offset = 0;
    for (const chunk of chunks) { result.set(chunk, offset); offset += chunk.byteLength; }
    return result;
  }
  async function start() {
    config = TabulaLaunch.resolve(TabulaLaunch.parse(location.search), matchMedia);
    text = config.locale === "en" ? en : vi;
    applyLanguage();
    document.documentElement.dataset.theme = config.resolvedTheme;
    setupUrl = `index.html?${TabulaLaunch.query(config)}`;
    byId("cancel-load").href = setupUrl;
    byId("error-back").href = setupUrl;
    const launchBytes = new TextEncoder().encode(TabulaLaunch.argumentsFor(config));
    const bytes = await download("tabula-game-client.wasm");
    if (bytes.byteLength > 64 * 1024 * 1024 || bytes.byteLength === 0) throw new Error("Invalid game artifact size");
    byId("loading-status").textContent = text.starting;
    byId("load-progress").removeAttribute("value");
    // This is the existing Miniquad file-loading API. Rust uses the safe
    // macroquad::file::load_file("tabula-launch.txt") only on wasm32.
    miniquad_add_plugin({register_plugin(imports) {
      const originalLoad = imports.env.fs_load_file;
      imports.env.fs_load_file = function (pointer, length) {
        const name = UTF8ToString(pointer, length);
        if (name !== "tabula-launch.txt" && name !== "tabula-ready.txt") return originalLoad(pointer, length);
        const id = FS.unique_id++;
        FS.loaded_files[id] = name === "tabula-launch.txt" ? launchBytes.slice() : new TextEncoder().encode("ready");
        setTimeout(() => {
          if (failed || leaving) return;
          try {
            wasm_exports.file_loaded(id);
            if (name === "tabula-ready.txt") boardAcknowledged = true;
          }
          catch (error) { fail(error); }
        }, 0);
        return id;
      };
    }});
    register_plugins(plugins);
    const compiled = await WebAssembly.compile(bytes);
    for (const entry of WebAssembly.Module.imports(compiled)) {
      if (!importObject[entry.module] || importObject[entry.module][entry.name] === undefined) throw new Error(`Bootstrap import unavailable: ${entry.module}.${entry.name}`);
    }
    const instance = await WebAssembly.instantiate(compiled, importObject);
    wasm_memory = instance.exports.memory;
    wasm_exports = instance.exports;
    if (!(wasm_memory instanceof WebAssembly.Memory) || typeof wasm_exports.main !== "function" || wasm_exports.crate_version() !== version) throw new Error("The game and pinned Miniquad bootstrap are incompatible");
    init_plugins(plugins);
    // Fetch/compile/main are not board readiness. Reveal after an actual
    // successful runtime frame, which includes Rust startup and embedded art.
    const originalFrame = wasm_exports.frame;
    if (typeof originalFrame !== "function") throw new Error("The game frame export is unavailable");
    const originalAnimation = animation;
    animation = function () {
      if (failed || leaving) return;
      try {
        originalAnimation();
        if (!ready && boardAcknowledged) {
          ready = true;
          clearTimeout(startupTimer);
          byId("loader").hidden = true;
          byId("glcanvas").tabIndex = 0;
          byId("glcanvas").removeAttribute("aria-hidden");
          byId("glcanvas").focus();
          forwardCanvasFocus(true);
        }
      } catch (error) { fail(error); }
    };
    startupTimer = setTimeout(() => fail(new Error(config.locale === "en" ? "The board did not start. Return to setup or try again." : "Bàn cờ chưa khởi động được. Về thiết lập hoặc thử lại.")), 30000);
    wasm_exports.main();
  }
  start().catch(fail);
})();
