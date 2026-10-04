(function () {
  "use strict";
  const byId = (id) => document.getElementById(id);
  const strings = {
    vi:{skip:"Đến thiết lập ván",game:"Cờ vua",local:"Chơi cục bộ",language:"Ngôn ngữ",breadcrumb:"Cờ vua / Ván mới",meta:"2 người · Bàn cờ 8 × 8 · Chiến thuật",setup:"Thiết lập ván",subtitle:"Một ván cờ thật, trên cùng thiết bị",mode:"Cách chơi",hotSeat:"Cùng thiết bị",hotSeatHint:"2 người trên một máy",ai:"Bot cục bộ",aiHint:"Chưa có trong bản này",online:"Trực tuyến",onlineHint:"Chưa có dịch vụ mạng",players:"Người chơi",white:"Người chơi 1 · Trắng",black:"Người chơi 2 · Đen",playersHint:"Luân phiên trên cùng thiết bị",clock:"Đồng hồ",untimed:"Không tính giờ",initial:"Thời gian ban đầu (phút)",increment:"Giờ cộng (giây)",delay:"Giờ hoãn (giây)",preferences:"Giao diện và chuyển động",theme:"Giao diện",system:"Theo hệ thống",light:"Sáng",dark:"Tối",hcLight:"Tương phản cao · sáng",hcDark:"Tương phản cao · tối",motion:"Chuyển động",reduced:"Giảm chuyển động",start:"Bắt đầu ván cờ",localNotice:"Ván cục bộ không được lưu. Bàn cờ dùng Macroquad; hỗ trợ trình đọc màn hình cho toàn bộ ván chưa có"},
    en:{skip:"Skip to game setup",game:"Chess",local:"Local play",language:"Language",breadcrumb:"Chess / New game",meta:"2 players · 8 × 8 board · Strategy",setup:"Set up your game",subtitle:"A real game of Chess, on one device",mode:"How to play",hotSeat:"Same device",hotSeatHint:"2 players on one device",ai:"Local bot",aiHint:"Unavailable in this build",online:"Online",onlineHint:"Network service unavailable",players:"Players",white:"Player 1 · White",black:"Player 2 · Black",playersHint:"Take turns on the same device",clock:"Clock",untimed:"No clock",initial:"Starting time (minutes)",increment:"Increment (seconds)",delay:"Delay (seconds)",preferences:"Appearance and motion",theme:"Theme",system:"Follow system",light:"Light",dark:"Dark",hcLight:"High contrast · light",hcDark:"High contrast · dark",motion:"Motion",reduced:"Reduced motion",start:"Start game",localNotice:"Local games are not saved. The board uses Macroquad; a complete screen-reader board is not available"} // xtask-allow-game-id: direct Phase 2 standalone game-client leaf wiring; not platform dispatch.
  };
  let config;
  let launching = false;
  try { config = TabulaLaunch.parse(location.search); }
  catch (error) { config = TabulaLaunch.parse(""); byId("setup-error").textContent = error.message; byId("setup-error").hidden = false; }
  byId("locale").value = config.locale;
  byId("clock").value = config.clock;
  byId("theme").value = config.theme;
  byId("motion").value = config.motion;
  byId("initial").value = String(config.initialMs / 60000);
  byId("initial").min = String(1 / 60);
  byId("initial").step = "any";
  byId("adjustment").value = String((config.clock === "bronstein" ? config.delayMs : config.incrementMs) / 1000);
  byId("adjustment").step = "any";
  function milliseconds(input, scale, min, max, name) {
    const value = Number(input.value) * scale;
    if (input.value.trim() === "" || !Number.isFinite(value) || Math.abs(Math.round(value) - value) > 0.0001) throw new Error(`${name}: enter a valid time`);
    return TabulaLaunch.integer(String(Math.round(value)), undefined, min, max, name);
  }
  function read() {
    const clock = byId("clock").value;
    const timed = clock !== "untimed";
    const initialMs = timed ? milliseconds(byId("initial"), 60000, 1000, 10800000, "Starting time") : config.initialMs;
    const adjustment = timed ? milliseconds(byId("adjustment"), 1000, 0, 60000, "Increment / delay") : 0;
    return {...config,clock,initialMs,incrementMs:clock === "fischer" ? adjustment : config.incrementMs,delayMs:clock === "bronstein" ? adjustment : config.delayMs,theme:byId("theme").value,motion:byId("motion").value,locale:byId("locale").value};
  }
  function refresh() {
    const locale = byId("locale").value;
    const text = strings[locale];
    document.documentElement.lang = locale;
    document.title = `Tabula · ${text.game}`;
    document.querySelectorAll("[data-i18n]").forEach((element) => { element.textContent = text[element.dataset.i18n]; });
    byId("locale").setAttribute("aria-label", text.language);
    const clock = byId("clock").value;
    const timed = clock !== "untimed";
    byId("clock-fields").hidden = !timed;
    byId("initial").disabled = !timed;
    byId("adjustment").disabled = !timed;
    byId("adjustment-label").textContent = text[clock === "bronstein" ? "delay" : "increment"];
    const amount = byId("adjustment").value;
    byId("clock-hint").textContent = !timed ? (locale === "vi" ? "Không giới hạn thời gian mỗi nước đi" : "Take as long as you need for each move") : clock === "bronstein" ? (locale === "vi" ? `Hoàn lại tối đa ${amount} giây đã dùng mỗi nước đi` : `Refund up to ${amount} seconds spent on each move`) : (locale === "vi" ? `Cộng ${amount} giây sau mỗi nước đi` : `Add ${amount} seconds after each move`);
    byId("summary").textContent = `${locale === "vi" ? "Ván chơi cục bộ · 2 người" : "Local game · 2 players"} · ${timed ? `${clock === "bronstein" ? "Bronstein" : "Fischer"} ${byId("initial").value} ${locale === "vi" ? "phút" : "min"} ${clock === "bronstein" ? "/" : "+"} ${amount} ${locale === "vi" ? "giây" : "sec"}` : text.untimed}`;
    document.documentElement.dataset.theme = TabulaLaunch.resolve({theme:byId("theme").value,motion:byId("motion").value}, matchMedia).resolvedTheme;
  }
  byId("setup").addEventListener("input", refresh);
  byId("locale").addEventListener("change", refresh);
  ["(prefers-color-scheme: dark)","(prefers-contrast: more)","(forced-colors: active)"].forEach((query) => matchMedia(query).addEventListener("change", refresh));
  byId("setup").addEventListener("submit", (event) => {
    event.preventDefault();
    if (launching) return;
    try {
      const next = read();
      // Round-trip the exact browser handoff through the same bounded parser.
      const query = TabulaLaunch.query(next);
      TabulaLaunch.parse(query);
      byId("setup-error").hidden = true;
      launching = true;
      document.querySelector(".start").disabled = true;
      location.assign(`play.html?${query}`);
    } catch (error) { byId("setup-error").textContent = error.message; byId("setup-error").hidden = false; }
  });
  // A back/forward-cache return must make the form usable again.
  window.addEventListener("pageshow", () => { launching = false; document.querySelector(".start").disabled = false; });
  refresh();
})();
