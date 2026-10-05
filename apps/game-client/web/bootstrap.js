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
  let live = true;
  let instanceExports = null;
  const current = () => live && window.__tabulaLocalGameHost === host;
  let ready = false;
  let failed = false;
  let leaving = false;
  let startupTimer;
  let boardAcknowledged = false;
  let config;
  let direct = null;
  // Mobile GameHost (ADR-0033). The native host injects an origin-restricted port at
  // document start; its absence means an ordinary browser document with no bridge.
  const hostMode = typeof window.TabulaHostNative === "object" && window.TabulaHostNative !== null && Boolean(window.TabulaHostBridge);
  let bridge = null;
  let hostPreferences = null;
  let suspended = false;
  const integratedEntry = /^\/play\/local\/(?:index\.html)?$/.test(location.pathname ?? "");
  let setupUrl = integratedEntry ? "/games" : location.pathname?.startsWith("/play/local/") ? "standalone.html" : "index.html";
  let returnToTabula = integratedEntry;
  const vi = {loading:"Đang mở bàn cờ…",checking:"Đang mở tài nguyên chương trình",download:"Đang tải chương trình WebAssembly",verifying:"Đang kiểm tra chương trình WebAssembly",cacheCheck:"Đang kiểm tra chương trình WebAssembly đã lưu",cached:"Đang dùng chương trình WebAssembly đã kiểm tra",resources:"Đang mở tài nguyên bàn cờ",cacheSource:"bộ nhớ đệm đã kiểm tra",cacheCheckSource:"kiểm tra bộ nhớ đệm",networkSource:"tải từ mạng",starting:"Đang khởi tạo bàn cờ",back:"Về thiết lập",retry:"Thử lại",error:"Không thể mở bàn cờ",restart:"Tải lại sẽ bắt đầu một ván mới. Ván cục bộ không được lưu",leaveTitle:"Rời ván cờ?",leaveDetail:"Ván cục bộ này không được lưu. Bạn có thể ở lại hoặc quay về thiết lập để bắt đầu ván mới",stay:"Ở lại",leave:"Rời ván cờ",help:"Trợ giúp bàn phím",helpTitle:"Điều khiển bàn cờ",helpDetail:"Chạm hoặc nhấp quân rồi ô đích. Dùng phím mũi tên để đổi ô, Enter để chọn, Escape để hủy chọn hoặc phong cấp. Tab chuyển từ ô cuối tới các nút trong ván; Shift+Tab đến nút rời ván. Trình đọc màn hình đầy đủ cho bàn cờ chưa có. Đồng hồ tiếp tục chạy khi mở hộp thoại hoặc chuyển tab; tải lại bắt đầu ván mới",closeHelp:"Về bàn cờ",tabulaBack:"Về Tabula",tabulaLeaveDetail:"Ván cục bộ này không được lưu. Về Tabula để thiết lập ván mới. Đồng hồ tiếp tục chạy khi mở hộp thoại hoặc chuyển tab"};
  const en = {loading:"Opening your board…",checking:"Resolving game resources",download:"Downloading WebAssembly game",verifying:"Checking WebAssembly game",cacheCheck:"Checking cached WebAssembly game",cached:"Using verified cached WebAssembly game",resources:"Opening board resource",cacheSource:"verified cache",cacheCheckSource:"checking cached bytes",networkSource:"network download",starting:"Starting the Chess board",back:"Back to setup",retry:"Try again",error:"Could not open the board",restart:"Reloading starts a new game. Local games are not saved",leaveTitle:"Leave this game?",leaveDetail:"This local game is not saved. Stay here or return to setup to start a new game",stay:"Stay",leave:"Leave game",help:"Keyboard help",helpTitle:"Board controls",helpDetail:"Tap or click a piece, then its destination. Use arrow keys to move focus, Enter to select, and Escape to cancel selection or promotion. Tab moves from the last square to in-game controls; Shift+Tab reaches the leave button. A complete screen-reader board is not available. Clocks keep running while dialogs are open or the tab is hidden; reloading starts a new game",closeHelp:"Back to board",tabulaBack:"Return to Tabula",tabulaLeaveDetail:"This local game is not saved. Return to Tabula to set up a new game. Clocks keep running while dialogs are open or the tab is hidden"}; // xtask-allow-game-id: direct Phase 2 standalone game-client leaf wiring; not platform dispatch.
  let text = vi;
  byId("glcanvas").tabIndex = -1;
  byId("glcanvas").setAttribute("aria-hidden", "true");
  byId("cancel-load").focus();
  function stopRuntime() {
    if (!live) return;
    // Cancellation clears held input before retiring exports. Rust still owns
    // local clock policy; this is document disposal, never a game pause.
    try { if (typeof wasm_exports !== "undefined" && typeof wasm_exports?.focus === "function") wasm_exports.focus(false); } catch (_) {}
    live = false;
    controller.abort();
    direct?.retire();
    clearTimeout(startupTimer);
    for (const timer of pendingTimers) clearTimeout(timer);
    pendingTimers.clear();
    if (typeof FS !== "undefined" && FS?.loaded_files) for (const id of loadedFileIds) delete FS.loaded_files[id];
    loadedFileIds.clear();
    if (typeof animation_frame_timeout !== "undefined") cancelAnimationFrame(animation_frame_timeout);
    window.blocking_event_loop = true;
    instanceExports = null;
    if (typeof wasm_memory !== "undefined") wasm_memory = null;
    // Pinned DOM callbacks can still be queued during pagehide/BFCache. Their
    // admitted export wrappers become no-ops and retain no instance functions.
    if (typeof wasm_exports !== "undefined" && wasm_exports) for (const name of Object.keys(wasm_exports)) if (typeof wasm_exports[name] !== "function") wasm_exports[name] = null;
  }
  function leaveDocument(url, reload = false) {
    if (leaving) return;
    leaving = true;
    stopRuntime();
    // Inside a mobile host the shell owns navigation: ask it to leave and stay inert.
    // A retry still reloads this document, which performs a fresh handshake.
    if (hostMode && !reload) { bridge?.exit(); return; }
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
  function fail(error, code = "runtime") {
    if (failed || leaving) return;
    failed = true;
    ready = false;
    // A failed runtime is only restarted through an explicit document reload.
    stopRuntime();
    bridge?.failed(code, error instanceof Error ? error.message : String(error));
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
    if (!current() || !ready || failed || leaving || typeof wasm_exports === "undefined" || typeof wasm_exports?.focus !== "function") return;
    try {
      wasm_exports.focus(Boolean(focused && document.hasFocus() && document.visibilityState === "visible"));
    } catch (error) { fail(error); }
  }
  byId("glcanvas").addEventListener("blur", () => forwardCanvasFocus(false));
  byId("glcanvas").addEventListener("focus", () => forwardCanvasFocus(true));
  function applyLanguage(locale) {
    const ids = {"loading-title":"loading","loading-status":"checking","cancel-load":"back","error-title":"error","restart-notice":"restart","error-back":"back","retry":"retry","leave":"back","help":"help","leave-title":"leaveTitle","leave-detail":"leaveDetail","stay":"stay","confirm-leave":"leave","help-title":"helpTitle","help-detail":"helpDetail","close-help":"closeHelp"};
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
  byId("glcanvas").addEventListener("webglcontextlost", (event) => { event.preventDefault(); fail(new Error(config?.locale === "en" ? "The graphics context was lost. Return to setup or restart a new game." : "Đã mất kết nối đồ họa. Về thiết lập hoặc tải lại để bắt đầu ván mới."), "graphics-context"); });
  window.addEventListener("error", (event) => fail(event.error ?? new Error(event.message)));
  window.addEventListener("unhandledrejection", (event) => fail(event.reason));
  window.addEventListener("beforeunload", (event) => { if (ready && !leaving) { event.preventDefault(); event.returnValue = ""; } });
  // pagehide, not beforeunload, establishes that Back/close really happened:
  // cancelling the browser's leave warning must keep the current game alive.
  window.addEventListener("pagehide", () => { leaving = true; stopRuntime(); });
  window.addEventListener("pageshow", (event) => {
    if (event.persisted && !live) { leaving = false; leaveDocument(null, true); }
  });
  // Host lifecycle (ADR-0033). Suspending stops drawing and clears held input only: the
  // local clock keeps wall-clock time exactly as for a hidden browser document (ADR-0030).
  function suspendHost() {
    if (!current() || suspended || leaving) return;
    suspended = true;
    forwardCanvasFocus(false);
  }
  function resumeHost() {
    if (!current() || !suspended) return;
    suspended = false;
    if (!ready || failed || leaving || typeof animation === "undefined" || typeof window.requestAnimationFrame !== "function") return;
    // Cancel the last scheduled frame first so a suspend/resume inside one tick cannot leave two loops.
    if (typeof animation_frame_timeout !== "undefined" && animation_frame_timeout) window.cancelAnimationFrame?.(animation_frame_timeout);
    animation_frame_timeout = window.requestAnimationFrame(animation);
    forwardCanvasFocus(document.activeElement === byId("glcanvas"));
  }
  function disposeHost() {
    leaving = true;
    stopRuntime();
  }
  // The system Back gesture is routed to the page's own leave confirmation while a match
  // is live; a loading or failed document has nothing to protect and leaves at once.
  function requestBack() {
    if (!current() || leaving) return;
    if (!ready || failed) { leaveDocument(setupUrl); return; }
    if (byId("leave-dialog").open) { byId("leave-dialog").close(); return; }
    byId("leave").click();
  }
  function resourceProgress(artifact) {
    return ({source, phase, received, total}) => {
      if (!current()) return;
      const progress = byId("load-progress");
      progress.max = total;
      if (phase === "verify") progress.removeAttribute("value");
      else progress.value = received;
      const label = source === "cache" ? (phase === "cache-hit" ? text.cached : text.cacheCheck) : (phase === "verify" ? text.verifying : text.download);
      byId("loading-status").textContent = artifact ? `${label} · ${(received / 1024).toFixed(0)} KiB` : `${text.resources} · ${source === "cache" ? (phase === "cache-hit" ? text.cacheSource : text.cacheCheckSource) : text.networkSource} · ${(received / 1024).toFixed(0)} KiB`;
    };
  }
  async function start() {
    // Validate navigation independently so a bad gameplay option still has a
    // safe shell return. Malformed navigation falls back to the fixed catalog.
    setReturnLinks();
    if (hostMode) {
      // Nothing starts until the native host has answered with this document's generation,
      // its granted capabilities and its preferences. Silence is a failure, not a default.
      bridge = window.TabulaHostBridge.attach(window.TabulaHostNative, {suspend:suspendHost, resume:resumeHost, dispose:disposeHost, backRequested:requestBack}, {timers:{setTimeout:(callback, delay) => setTimeout(callback, delay), clearTimeout:(timer) => clearTimeout(timer)}});
      try { hostPreferences = (await bridge.handshake()).preferences; }
      catch (error) { fail(error, "bridge"); return; }
      if (!current()) return;
    }
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
    // Host preferences replace only the three preference fields; every game option still
    // comes from the registry-validated launch query.
    const parsed = TabulaLaunch.parse(location.search);
    config = TabulaLaunch.resolve(hostPreferences ? Object.freeze({...parsed, theme:hostPreferences.theme, motion:hostPreferences.motion, locale:hostPreferences.locale}) : parsed, matchMedia);
    text = config.locale === "en" ? en : vi;
    applyLanguage(config.locale);
    if (config.online) {
      document.title = config.locale === "en" ? "Tabula · Online game" : "Tabula · Ván trực tuyến";
      byId("runtime").setAttribute("aria-label", config.locale === "en" ? "Online game" : "Ván trực tuyến");
      document.documentElement.dataset.mode = "online";
      if (hostMode) throw new Error("Online mobile hosting is unavailable");
      if (!window.TabulaDirectTransport) throw new Error("The online transport is unavailable");
      direct = window.TabulaDirectTransport.create({
        matchId:config.matchId, gameId:config.gameId, signal:controller.signal, current,
        onWaiting() { clearTimeout(startupTimer); byId("loading-status").textContent = config.locale === "en" ? "Waiting for the other player to join…" : "Đang đợi người chơi còn lại…"; },
        onStatus(value) {
          document.documentElement.dataset.onlineSeat = String(value.seat);
          document.documentElement.dataset.onlineRevision = String(value.revision);
          document.documentElement.dataset.onlineStatus = value.status;
          document.documentElement.dataset.onlineConnection = value.connection;
          byId("online-status-container").hidden = false;
          for (const [key,label] of [["seat",String(value.seat+1)],["revision",String(value.revision)],["status",value.status],["connection",value.connection]]) byId("online-status-container").querySelector('[data-testid="online-' + key + '"]').textContent = label;
        }
      });
      text = {...text,
        restart:config.locale === "en" ? "Reopening rechecks your session and seat. The server match stays saved." : "Mở lại sẽ kiểm tra phiên và chỗ chơi. Ván được lưu trên máy chủ.",
        tabulaLeaveDetail:config.locale === "en" ? "Return to Tabula. Leaving this page does not resign the server match." : "Về Tabula. Rời trang không đầu hàng ván trên máy chủ.",
        helpDetail:config.locale === "en" ? "Tap a piece then its destination. Arrow keys and Enter select; Escape cancels. The server decides every move. A complete screen reader is unavailable." : "Chạm quân rồi ô đích. Dùng mũi tên và Enter; Escape hủy. Máy chủ quyết định mỗi nước. Chưa có trình đọc màn hình đầy đủ."
      };
      applyLanguage(config.locale);
    }
    document.documentElement.dataset.theme = config.resolvedTheme;
    if (!returnToTabula) setupUrl = `${setupUrl}?${TabulaLaunch.query(config)}`;
    setReturnLinks();
    const launchBytes = new TextEncoder().encode(TabulaLaunch.argumentsFor(config));
    startupTimer = setTimeout(() => fail(new Error(config.locale === "en" ? "The board did not start. Return to setup or try again." : "Bàn cờ chưa khởi động được. Về thiết lập hoặc thử lại."), "timeout"), 30000);
    if (!window.TabulaResources) throw new Error("The verified game resource loader is unavailable");
    const resources = window.TabulaResources.create(window.TabulaResourceManifest, {signal:controller.signal, current});
    let bytes = await resources.load("tabula-game-client.wasm", {onProgress:resourceProgress(true)});
    if (!current()) return;
    byId("loading-status").textContent = text.starting;
    byId("load-progress").removeAttribute("value");
    // This is the existing Miniquad file-loading API. Rust uses the safe
    // macroquad::file::load_file("tabula-launch.txt") only on wasm32.
    miniquad_add_plugin({register_plugin(imports) {
      // Release this host's bookkeeping when the pinned bridge consumes bytes.
      const takeBuffer = imports.env.fs_take_buffer;
      if (typeof takeBuffer === "function") imports.env.fs_take_buffer = function (id, pointer, length) {
        try { return takeBuffer(id, pointer, length); }
        finally { loadedFileIds.delete(id); }
      };
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
        } else if (name.startsWith("tabula-online-")) {
          if (!direct) { fail(new Error("Online play is unavailable")); return id; }
          direct.file(name).then(deliver).catch((error) => {
            if (name === "tabula-online-attach.txt") fail(error);
            else deliver(new TextEncoder().encode("{}"));
          });
        } else {
          // Only explicitly requested manifest aliases cross this boundary.
          // The loader verifies public bytes before Rust's font/file decoding;
          // virtual configuration/readiness files never enter its cache.
          resources.load(name, {onProgress:resourceProgress(false)}).then(deliver).catch(fail);
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
    // successful runtime frame, which includes verified Rust startup resources.
    if (typeof wasm_exports.frame !== "function") throw new Error("The game frame export is unavailable");
    const originalAnimation = animation;
    animation = function () {
      if (!current() || failed || leaving || suspended) return;
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
          if (bridge) {
            bridge.ready(performance.now());
            // A match in progress must not sleep under the player; the host may refuse.
            bridge.service("keep-awake", true);
          }
        }
      } catch (error) { fail(error); }
    };
    wasm_exports.main();
  }
  start().catch(fail);
})();
