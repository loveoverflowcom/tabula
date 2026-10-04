/* Macroquad keeps ownership of the canvas, input and frame loop (ADR-011). */
(function () {
  "use strict";
  // One bootstrap per document, even if a host accidentally includes it twice.
  // Retrying/remounting always creates a fresh document and local match.
  if (window.__tabulaLocalGameHost) return;
  const host = {};
  window.__tabulaLocalGameHost = host;
  const byId = (id) => document.getElementById(id);
  const controller = new AbortController();
  const pendingTimers = new Set();
  const loadedFileIds = new Set();
  const artifactLimit = 64 * 1024 * 1024;
  let live = true;
  let instanceExports = null;
  const current = () => live && window.__tabulaLocalGameHost === host;
  let ready = false;
  let failed = false;
  let leaving = false;
  let startupTimer;
  let boardAcknowledged = false;
  let config;
  const integratedEntry = /^\/play\/local\/(?:index\.html)?$/.test(location.pathname ?? "");
  let setupUrl = integratedEntry ? "/games" : location.pathname?.startsWith("/play/local/") ? "standalone.html" : "index.html";
  let returnToTabula = integratedEntry;
  const vi = {loading:"Đang mở bàn cờ…",download:"Đang tải chương trình WebAssembly",starting:"Đang khởi tạo bàn cờ",back:"Về thiết lập",retry:"Thử lại",error:"Không thể mở bàn cờ",restart:"Tải lại sẽ bắt đầu một ván mới. Ván cục bộ không được lưu",leaveTitle:"Rời ván cờ?",leaveDetail:"Ván cục bộ này không được lưu. Bạn có thể ở lại hoặc quay về thiết lập để bắt đầu ván mới",stay:"Ở lại",leave:"Rời ván cờ",help:"Trợ giúp bàn phím",helpTitle:"Điều khiển bàn cờ",helpDetail:"Chạm hoặc nhấp quân rồi ô đích. Dùng phím mũi tên để đổi ô, Enter để chọn, Escape để hủy chọn hoặc phong cấp. Tab chuyển từ ô cuối tới các nút trong ván; Shift+Tab đến nút rời ván. Trình đọc màn hình đầy đủ cho bàn cờ chưa có. Đồng hồ tiếp tục chạy khi mở hộp thoại hoặc chuyển tab; tải lại bắt đầu ván mới",closeHelp:"Về bàn cờ",tabulaBack:"Về Tabula",tabulaLeaveDetail:"Ván cục bộ này không được lưu. Về Tabula để thiết lập ván mới. Đồng hồ tiếp tục chạy khi mở hộp thoại hoặc chuyển tab"};
  const en = {loading:"Opening your board…",download:"Downloading WebAssembly game",starting:"Starting the Chess board",back:"Back to setup",retry:"Try again",error:"Could not open the board",restart:"Reloading starts a new game. Local games are not saved",leaveTitle:"Leave this game?",leaveDetail:"This local game is not saved. Stay here or return to setup to start a new game",stay:"Stay",leave:"Leave game",help:"Keyboard help",helpTitle:"Board controls",helpDetail:"Tap or click a piece, then its destination. Use arrow keys to move focus, Enter to select, and Escape to cancel selection or promotion. Tab moves from the last square to in-game controls; Shift+Tab reaches the leave button. A complete screen-reader board is not available. Clocks keep running while dialogs are open or the tab is hidden; reloading starts a new game",closeHelp:"Back to board",tabulaBack:"Return to Tabula",tabulaLeaveDetail:"This local game is not saved. Return to Tabula to set up a new game. Clocks keep running while dialogs are open or the tab is hidden"}; // xtask-allow-game-id: direct Phase 2 standalone game-client leaf wiring; not platform dispatch.
  let text = vi;
  byId("glcanvas").tabIndex = -1;
  byId("glcanvas").setAttribute("aria-hidden", "true");
  byId("cancel-load").focus();
  function stopRuntime() {
    if (!live) return;
    // Cancellation clears held input before retiring exports. Rust still owns
    // local clock policy; this is document disposal, never a game pause.
    try { if (typeof wasm_exports?.focus === "function") wasm_exports.focus(false); } catch (_) {}
    live = false;
    controller.abort();
    clearTimeout(startupTimer);
    for (const timer of pendingTimers) clearTimeout(timer);
    pendingTimers.clear();
    for (const id of loadedFileIds) delete FS.loaded_files[id];
    loadedFileIds.clear();
    if (typeof animation_frame_timeout !== "undefined") cancelAnimationFrame(animation_frame_timeout);
    window.blocking_event_loop = true;
    instanceExports = null;
    wasm_memory = null;
    // Pinned DOM callbacks can still be queued during pagehide/BFCache. Their
    // admitted export wrappers become no-ops and retain no instance functions.
    if (wasm_exports) for (const name of Object.keys(wasm_exports)) if (typeof wasm_exports[name] !== "function") wasm_exports[name] = null;
  }
  function leaveDocument(url, reload = false) {
    if (leaving) return;
    leaving = true;
    stopRuntime();
    try {
      if (reload) location.reload();
      else location.assign(url);
    } catch (error) {
      // A blocked navigation cannot resurrect a retired local match. Expose
      // recovery again so Return/Retry can make another fresh-document attempt.
      leaving = false;
      failed = false;
      fail(error);
    }
  }
  function setReturnLinks() {
    for (const id of ["cancel-load", "error-back"]) byId(id).href = setupUrl;
    if (returnToTabula) {
      for (const id of ["cancel-load", "error-back", "leave", "confirm-leave"]) byId(id).textContent = text.tabulaBack;
      byId("leave-detail").textContent = text.tabulaLeaveDetail;
    }
  }
  function fail(error) {
    if (failed || leaving) return;
    failed = true;
    ready = false;
    // A failed runtime is only restarted through an explicit document reload.
    stopRuntime();
    for (const id of ["leave-dialog", "help-dialog"]) if (byId(id).open) byId(id).close();
    byId("loader").hidden = true;
    byId("runtime-error").hidden = false;
    byId("glcanvas").tabIndex = -1;
    byId("glcanvas").setAttribute("aria-hidden", "true");
    byId("error-detail").textContent = error instanceof Error ? error.message : String(error);
    byId("error-back").focus();
    console.error("Standalone Chess:", error); // xtask-allow-game-id: direct Phase 2 standalone game-client leaf wiring; not platform dispatch.
  }
  function forwardCanvasFocus(focused) {
    if (!current() || !ready || failed || leaving || typeof wasm_exports?.focus !== "function") return;
    try {
      wasm_exports.focus(Boolean(focused && document.hasFocus() && document.visibilityState === "visible"));
    } catch (error) { fail(error); }
  }
  byId("glcanvas").addEventListener("blur", () => forwardCanvasFocus(false));
  byId("glcanvas").addEventListener("focus", () => forwardCanvasFocus(true));
  function applyLanguage(locale) {
    const ids = {"loading-title":"loading","loading-status":"download","cancel-load":"back","error-title":"error","restart-notice":"restart","error-back":"back","retry":"retry","leave":"back","help":"help","leave-title":"leaveTitle","leave-detail":"leaveDetail","stay":"stay","confirm-leave":"leave","help-title":"helpTitle","help-detail":"helpDetail","close-help":"closeHelp"};
    for (const [id, key] of Object.entries(ids)) byId(id).textContent = text[key];
    byId("keyboard-help").textContent = text.helpDetail;
    byId("glcanvas").setAttribute("aria-label", text.helpTitle);
    document.documentElement.lang = locale;
    setReturnLinks();
  }
  byId("retry").addEventListener("click", () => leaveDocument(null, true));
  for (const id of ["cancel-load", "error-back"]) byId(id).addEventListener("click", (event) => { event.preventDefault(); leaveDocument(setupUrl); });
  byId("leave").addEventListener("click", () => {
    if (!ready || failed) { leaveDocument(setupUrl); return; }
    forwardCanvasFocus(false);
    byId("leave-dialog").showModal();
    byId("stay").focus();
  });
  byId("stay").addEventListener("click", () => byId("leave-dialog").close());
  byId("confirm-leave").addEventListener("click", () => leaveDocument(setupUrl));
  byId("help").addEventListener("click", () => { if (!current() || !ready) return; forwardCanvasFocus(false); byId("help-dialog").showModal(); });
  byId("close-help").addEventListener("click", () => byId("help-dialog").close());
  for (const id of ["leave-dialog", "help-dialog"]) byId(id).addEventListener("close", () => { if (current() && ready && !leaving) { byId("glcanvas").focus(); forwardCanvasFocus(true); } });
  // Ordinary Tab is presenter input (including its in-game HUD). The pinned
  // bundle prevents default Tab; Shift+Tab is the explicit accessible host exit.
  byId("glcanvas").addEventListener("keydown", (event) => {
    if (event.code === "Tab" && event.shiftKey) {
      event.preventDefault();
      event.stopImmediatePropagation();
      forwardCanvasFocus(false);
      byId("leave").focus();
    }
  }, true);
  byId("glcanvas").addEventListener("webglcontextlost", (event) => { event.preventDefault(); fail(new Error(config?.locale === "en" ? "The graphics context was lost. Return to setup or restart a new game." : "Đã mất kết nối đồ họa. Về thiết lập hoặc tải lại để bắt đầu ván mới.")); });
  window.addEventListener("error", (event) => fail(event.error ?? new Error(event.message)));
  window.addEventListener("unhandledrejection", (event) => fail(event.reason));
  window.addEventListener("beforeunload", (event) => { if (ready && !leaving) { event.preventDefault(); event.returnValue = ""; } });
  // pagehide, not beforeunload, establishes that Back/close really happened:
  // cancelling the browser's leave warning must keep the current game alive.
  window.addEventListener("pagehide", () => { leaving = true; stopRuntime(); });
  window.addEventListener("pageshow", (event) => {
    if (event.persisted && !live) { leaving = false; leaveDocument(null, true); }
  });
  async function download(url, reportProgress = false) {
    const response = await fetch(url, {signal:controller.signal, credentials:"same-origin"});
    if (!current()) return null;
    if (!response.ok) throw new Error(`Game download failed: HTTP ${response.status}`);
    const advertised = Number(response.headers.get("Content-Length"));
    const exactLength = advertised > 0 && !response.headers.get("Content-Encoding");
    if (advertised > artifactLimit) throw new Error("Game artifact exceeds the 64 MiB host limit");
    const progress = byId("load-progress");
    if (reportProgress && exactLength) progress.max = advertised;
    if (!response.body) {
      const bytes = new Uint8Array(await response.arrayBuffer());
      if (!current()) return null;
      if (!bytes.byteLength || bytes.byteLength > artifactLimit) throw new Error("Invalid game artifact size");
      return bytes;
    }
    const reader = response.body.getReader();
    const chunks = [];
    let received = 0;
    try {
      for (;;) {
        const {done, value} = await reader.read();
        if (!current()) { await reader.cancel(); return null; }
        if (done) break;
        received += value.byteLength;
        if (received > artifactLimit) throw new Error("Game artifact exceeds the 64 MiB host limit");
        chunks.push(value);
        if (reportProgress) {
          if (exactLength && received <= advertised) progress.value = received;
          else progress.removeAttribute("value");
          byId("loading-status").textContent = `${text.download} · ${(received / 1024).toFixed(0)} KiB`;
        }
      }
    } catch (error) {
      try { await reader.cancel(); } catch (_) {}
      throw error;
    } finally { reader.releaseLock(); }
    if (received === 0) throw new Error("The game artifact is empty");
    const result = new Uint8Array(received);
    let offset = 0;
    for (const chunk of chunks) { result.set(chunk, offset); offset += chunk.byteLength; }
    return result;
  }
  async function start() {
    // Validate navigation independently so a bad gameplay option still has a
    // safe shell return. Malformed navigation falls back to the fixed catalog.
    setReturnLinks();
    if (location.search.length > 4096) throw new Error("Launch configuration is too long");
    const rawQuery = new URLSearchParams(location.search);
    const locale = rawQuery.getAll("locale").length === 1 && rawQuery.get("locale") === "en" ? "en" : "vi";
    text = locale === "en" ? en : vi;
    applyLanguage(locale);
    if (rawQuery.has("source") || rawQuery.has("return_to")) {
      setupUrl = "/games";
      returnToTabula = true;
      setReturnLinks();
    }
    const navigation = TabulaLaunch.navigation(location.search);
    if (integratedEntry && !navigation) throw new Error("Tabula launch metadata is required for this entry");
    if (navigation) { setupUrl = navigation.returnTo; setReturnLinks(); }
    config = TabulaLaunch.resolve(TabulaLaunch.parse(location.search), matchMedia);
    text = config.locale === "en" ? en : vi;
    applyLanguage(config.locale);
    document.documentElement.dataset.theme = config.resolvedTheme;
    if (!returnToTabula) setupUrl = `${setupUrl}?${TabulaLaunch.query(config)}`;
    setReturnLinks();
    const launchBytes = new TextEncoder().encode(TabulaLaunch.argumentsFor(config));
    startupTimer = setTimeout(() => fail(new Error(config.locale === "en" ? "The board did not start. Return to setup or try again." : "Bàn cờ chưa khởi động được. Về thiết lập hoặc thử lại.")), 30000);
    let bytes = await download("tabula-game-client.wasm", true);
    if (!current()) return;
    byId("loading-status").textContent = text.starting;
    byId("load-progress").removeAttribute("value");
    // This is the existing Miniquad file-loading API. Rust uses the safe
    // macroquad::file::load_file("tabula-launch.txt") only on wasm32.
    miniquad_add_plugin({register_plugin(imports) {
      imports.env.fs_load_file = function (pointer, length) {
        if (!current()) return 0;
        const name = UTF8ToString(pointer, length);
        const id = FS.unique_id++;
        loadedFileIds.add(id);
        function deliver(bytes) {
          if (!current()) return;
          FS.loaded_files[id] = bytes;
          try {
            wasm_exports.file_loaded(id);
            if (name === "tabula-ready.txt") boardAcknowledged = true;
          } catch (error) { fail(error); }
        }
        if (name === "tabula-launch.txt" || name === "tabula-ready.txt") {
          const timer = setTimeout(() => {
            pendingTimers.delete(timer);
            deliver(name === "tabula-launch.txt" ? launchBytes.slice() : new TextEncoder().encode("ready"));
          }, 0);
          pendingTimers.add(timer);
        } else {
          // Preserve the pinned fs_load_file/file_loaded API while making asset
          // requests cancelable and stale callbacks inadmissible. No new cache.
          try {
            const asset = new URL(name, location.href);
            if (asset.origin !== location.origin || asset.username || asset.password) throw new Error("Game assets must be same-origin");
            download(asset.href).then(deliver).catch(fail);
          } catch (error) { fail(error); }
        }
        return id;
      };
    }});
    register_plugins(plugins);
    let compiled = await WebAssembly.compile(bytes);
    bytes = null;
    if (!current()) return;
    for (const entry of WebAssembly.Module.imports(compiled)) {
      if (!importObject[entry.module] || importObject[entry.module][entry.name] === undefined) throw new Error(`Bootstrap import unavailable: ${entry.module}.${entry.name}`);
    }
    let instance = await WebAssembly.instantiate(compiled, importObject);
    compiled = null;
    if (!current()) return;
    wasm_memory = instance.exports.memory;
    instanceExports = instance.exports;
    instance = null;
    if (!(wasm_memory instanceof WebAssembly.Memory) || typeof instanceExports.main !== "function" || typeof instanceExports.crate_version !== "function" || instanceExports.crate_version() !== version) throw new Error("The game and pinned Miniquad bootstrap are incompatible");
    // Pinned canvas/visibility/file callbacks use these exports. Retirement
    // removes the instance reference; even queued callbacks cannot re-enter it.
    wasm_exports = {};
    for (const name of Object.keys(instanceExports)) {
      wasm_exports[name] = typeof instanceExports[name] !== "function" ? instanceExports[name] : (...args) => {
        if (!current() || !instanceExports) return;
        if (name === "focus" && args[0]) args[0] = ready && !byId("leave-dialog").open && !byId("help-dialog").open && document.activeElement === byId("glcanvas") && document.hasFocus() && document.visibilityState === "visible";
        return instanceExports[name](...args);
      };
    }
    init_plugins(plugins);
    // Fetch/compile/main are not board readiness. Reveal after an actual
    // successful runtime frame, which includes Rust startup and embedded art.
    if (typeof wasm_exports.frame !== "function") throw new Error("The game frame export is unavailable");
    const originalAnimation = animation;
    animation = function () {
      if (!current() || failed || leaving) return;
      try {
        originalAnimation();
        if (!current() || failed || leaving) return;
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
    wasm_exports.main();
  }
  start().catch(fail);
})();
