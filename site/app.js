(function () {
  "use strict";
  var REPO = "tnarnold/nerdminer-emu";
  var root = document.documentElement;
  var isPt = function () { return /^pt/i.test(root.lang); };
  var nf = function (n, d) {
    return n.toLocaleString(isPt() ? "pt-BR" : "en-US", { maximumFractionDigits: d == null ? 0 : d });
  };

  /* ---------- language ---------- */
  function setLang(l) {
    root.lang = l;
    try { localStorage.setItem("lang", l); } catch (e) {}
    document.querySelectorAll("[data-set-lang]").forEach(function (b) {
      b.classList.toggle("on", b.getAttribute("data-set-lang") === l);
    });
    renderAll();
  }
  document.querySelectorAll("[data-set-lang]").forEach(function (b) {
    b.addEventListener("click", function () { setLang(b.getAttribute("data-set-lang")); });
  });

  /* ---------- nav ---------- */
  var nav = document.getElementById("nav");
  function onScroll() { nav.classList.toggle("scrolled", window.scrollY > 40); }
  window.addEventListener("scroll", onScroll, { passive: true });
  onScroll();

  /* ---------- reveal ---------- */
  if ("IntersectionObserver" in window) {
    var io = new IntersectionObserver(function (es) {
      es.forEach(function (e) { if (e.isIntersecting) { e.target.classList.add("in"); io.unobserve(e.target); } });
    }, { rootMargin: "0px 0px -8% 0px" });
    document.querySelectorAll(".reveal").forEach(function (el) { io.observe(el); });
  } else {
    document.querySelectorAll(".reveal").forEach(function (el) { el.classList.add("in"); });
  }

  /* ---------- formatting ---------- */
  function fmtHash(h) {
    var u = ["H/s", "kH/s", "MH/s", "GH/s", "TH/s", "PH/s", "EH/s", "ZH/s"], i = 0;
    while (h >= 1000 && i < u.length - 1) { h /= 1000; i++; }
    return nf(h, h < 10 ? 2 : h < 100 ? 1 : 0) + " " + u[i];
  }
  function fmtBig(n) {
    var u = isPt()
      ? [[1e12, " tri"], [1e9, " bi"], [1e6, " mi"]]
      : [[1e12, "T"], [1e9, "B"], [1e6, "M"]];
    for (var i = 0; i < u.length; i++) if (n >= u[i][0]) return nf(n / u[i][0], 2) + u[i][1];
    return nf(n);
  }
  function fmtDuration(sec) {
    var pt = isPt();
    var y = sec / 31557600;
    if (y >= 1e6) return fmtBig(y) + (pt ? " anos" : " years");
    if (y >= 2) return nf(y) + (pt ? " anos" : " years");
    var d = sec / 86400;
    if (d >= 2) return nf(d) + (pt ? " dias" : " days");
    var h = sec / 3600;
    if (h >= 2) return nf(h, 1) + " h";
    var m = sec / 60;
    if (m >= 1) return nf(m, 1) + " min";
    return nf(sec, 1) + " s";
  }
  function fmtOdds(p) {
    if (p >= 0.995) return "≈ 100%";
    if (p >= 0.01) return nf(p * 100, 1) + "%";
    return (isPt() ? "1 em " : "1 in ") + fmtBig(1 / p);
  }

  /* ---------- live network data (mempool.space) ---------- */
  var net = { diff: 1.5e14, hash: null, height: null, usd: null, brl: null, live: false };
  function getJSON(url) {
    return fetch(url, { cache: "no-store" }).then(function (r) { if (!r.ok) throw r; return r.json(); });
  }
  function renderTicker() {
    var $ = function (id) { return document.getElementById(id); };
    if (net.height) $("t-height").textContent = nf(net.height);
    if (net.hash) $("t-hash").textContent = fmtHash(net.hash);
    if (net.live) $("t-diff").textContent = fmtBig(net.diff);
    if (net.usd) {
      $("t-price").textContent = isPt() && net.brl
        ? "R$ " + nf(net.brl)
        : "$" + nf(net.usd);
    }
    var prize = document.getElementById("r-prize");
    if (net.usd) {
      var v = 3.125 * (isPt() && net.brl ? net.brl : net.usd);
      prize.innerHTML = "3.125 BTC<br><span style=\"font-size:.6em;color:var(--muted)\">≈ " + (isPt() && net.brl ? "R$ " : "$") + fmtBig(v) + "</span>";
    }
  }
  function loadNet() {
    Promise.allSettled([
      getJSON("https://mempool.space/api/blocks/tip/height"),
      getJSON("https://mempool.space/api/v1/mining/hashrate/3d"),
      getJSON("https://mempool.space/api/v1/prices"),
      getJSON("https://api.coinbase.com/v2/prices/BTC-BRL/spot")
    ]).then(function (r) {
      if (r[0].status === "fulfilled") net.height = r[0].value;
      if (r[1].status === "fulfilled" && r[1].value.currentDifficulty) {
        net.diff = r[1].value.currentDifficulty;
        net.hash = r[1].value.currentHashrate;
        net.live = true;
      }
      if (r[2].status === "fulfilled") net.usd = r[2].value.USD;
      if (r[3].status === "fulfilled") net.brl = parseFloat(r[3].value.data.amount) || null;
      renderTicker();
      renderCalc();
      renderStatus();
    });
  }

  /* ---------- odds calculator ---------- */
  var range = document.getElementById("h-range");
  var hOut = document.getElementById("h-out");
  var presets = document.querySelectorAll("#presets button");
  var exactH = 8e7;
  function hashrate() { return exactH || Math.pow(10, parseFloat(range.value)); }
  function renderCalc() {
    var H = hashrate();
    hOut.textContent = fmtHash(H);
    var perBlock = net.diff * 4294967296; // expected hashes per block
    var t = perBlock / H;
    var pDay = 1 - Math.exp(-H * 86400 / perBlock);
    var pYear = 1 - Math.exp(-H * 31557600 / perBlock);
    document.getElementById("r-time").textContent = fmtDuration(t);
    document.getElementById("r-day").textContent = fmtOdds(pDay);
    document.getElementById("r-year").textContent = fmtOdds(pYear);
  }
  range.addEventListener("input", function () {
    exactH = null;
    presets.forEach(function (b) { b.classList.remove("on"); });
    renderCalc();
  });
  presets.forEach(function (b) {
    b.addEventListener("click", function () {
      presets.forEach(function (x) { x.classList.remove("on"); });
      b.classList.add("on");
      exactH = parseFloat(b.getAttribute("data-h"));
      range.value = Math.log10(exactH);
      renderCalc();
    });
  });

  /* ---------- downloads (GitHub Releases) ---------- */
  var ua = navigator.userAgent;
  var os = /Mac|iPhone|iPad/.test(ua) ? "mac" : /Win/.test(ua) ? "win" : /Linux|X11/.test(ua) && !/Android/.test(ua) ? "linux" : null;
  var match = { mac: /macos.*\.dmg$/i, win: /windows.*\.zip$/i, linux: /linux.*\.tar\.gz$/i };
  var osName = { mac: "macOS", win: "Windows", linux: "Linux" };
  var releases = null;

  if (os) {
    var card = document.querySelector('.dl[data-os="' + os + '"]');
    if (card) {
      card.classList.add("mine");
      card.parentNode.insertBefore(card, card.parentNode.firstChild);
    }
  }

  function fmtSize(b) { return nf(b / 1048576, 1) + " MB"; }
  function renderReleases() {
    if (!releases || !releases.length) return;
    var latest = releases[0];
    document.querySelectorAll(".js-version").forEach(function (e) { e.textContent = latest.tag_name.replace(/^v/, ""); });
    document.querySelectorAll(".js-version-full").forEach(function (e) { e.textContent = latest.tag_name; });
    var mine = null;
    document.querySelectorAll(".dl").forEach(function (card) {
      var k = card.getAttribute("data-os");
      var a = (latest.assets || []).filter(function (x) { return match[k].test(x.name); })[0];
      if (!a) return;
      card.querySelector(".js-file").textContent = a.name + " · " + fmtSize(a.size);
      card.querySelector(".js-link").href = a.browser_download_url;
      if (k === os) mine = a;
    });
    var heroBtn = document.querySelector(".js-hero-dl");
    var hint = document.querySelector(".js-os-hint");
    if (mine) {
      heroBtn.href = mine.browser_download_url;
      document.querySelector(".js-hero-dl-text").textContent =
        (isPt() ? "Baixar para " : "Download for ") + osName[os];
      hint.textContent = latest.tag_name + " · " + mine.name + " · " + fmtSize(mine.size);
    }
    var list = document.getElementById("ver-list");
    list.innerHTML = "";
    releases.forEach(function (r) {
      var row = document.createElement("div");
      row.className = "ver";
      var b = document.createElement("b"); b.textContent = r.tag_name; row.appendChild(b);
      var d = document.createElement("span"); d.className = "date";
      d.textContent = new Date(r.published_at).toLocaleDateString(isPt() ? "pt-BR" : "en-US");
      row.appendChild(d);
      (r.assets || []).forEach(function (a) {
        var l = document.createElement("a");
        l.href = a.browser_download_url; l.textContent = a.name;
        row.appendChild(l);
      });
      var n = document.createElement("a");
      n.href = r.html_url; n.textContent = isPt() ? "notas" : "notes";
      row.appendChild(n);
      list.appendChild(row);
    });
  }
  getJSON("https://api.github.com/repos/" + REPO + "/releases?per_page=20")
    .then(function (r) {
      releases = r.filter(function (x) { return !x.draft; });
      renderReleases();
    })
    .catch(function () {});


  /* ---------- tray menu preview (same logic as src/netinfo.rs + src/tray.rs) ---------- */
  var st = null;
  var BR = function (v, d) { return v.toLocaleString("pt-BR", { minimumFractionDigits: d || 0, maximumFractionDigits: d || 0 }); };
  function brBig(n) {
    var u = [[1e12, "tri"], [1e9, "bi"], [1e6, "mi"], [1e3, "mil"]];
    for (var i = 0; i < u.length; i++) if (n >= u[i][0]) { var v = n / u[i][0]; return BR(v, v < 100 && v % 1 > 0.05 ? 1 : 0) + " " + u[i][1]; }
    return BR(n);
  }
  function feerate(f) { return (f >= 10 || Math.abs(f - Math.round(f)) < 0.05) ? BR(Math.round(f)) : BR(f, f < 1 ? 2 : 1); }
  function dur(sec) {
    sec = Math.max(0, sec);
    if (sec < 60) return Math.round(sec) + " s";
    if (sec < 3600) return Math.floor(sec / 60) + " min";
    if (sec < 172800) { var m = Math.floor(sec / 60); return Math.floor(m / 60) + " h " + ("0" + m % 60).slice(-2); }
    if (sec < 86400 * 730) return BR(sec / 86400, 1).replace(",0", "") + " dias";
    return brBig(sec / 31557600) + " anos";
  }
  function odds(p) { return p >= 0.01 ? BR(p * 100, 1) + "%" : "1 em " + brBig(1 / p); }
  var LEVELS = [
    { dot: "🟢", name: "Livre", color: "#34c759", tier: 3, why: "rede livre, não pague mais" },
    { dot: "🟡", name: "Moderada", color: "#ffcc00", tier: 1, why: "deve confirmar em ~30 min" },
    { dot: "🟠", name: "Congestionada", color: "#ff9500", tier: 2, why: "taxa baixa pode demorar horas" },
    { dot: "🔴", name: "Muito congestionada", color: "#ff3b30", tier: 2, why: "rede cheia: envie só se for urgente" }
  ];
  // Level from the ~30 min fee (sat/vB): ≤2 free, ≤10 moderate, ≤50 congested, >50 severe.
  function level(fee) { var f = Math.round(fee * 10) / 10; return f <= 2.49 ? 0 : f <= 10.49 ? 1 : f <= 50.49 ? 2 : 3; }
  function barFrac(f) {
    var seg = function (x, lo, hi) { return Math.min(1, Math.max(0, (x - lo) / (hi - lo))); };
    return f <= 2 ? 0.25 * seg(f, 0, 2) : f <= 10 ? 0.25 + 0.25 * seg(f, 2, 10) : f <= 50 ? 0.5 + 0.25 * seg(f, 10, 50) : 0.75 + 0.25 * seg(f, 50, 300);
  }
  function money(btc) {
    var brl = isPt() && net.brl;
    var p = brl ? net.brl : net.usd;
    if (!p) return "";
    return " ≈ " + (brl ? "R$ " : "US$ ") + BR(btc * p, 2);
  }
  function loadStatus() {
    var B = "https://mempool.space/api";
    Promise.allSettled([
      getJSON(B + "/mempool"), getJSON(B + "/v1/fees/recommended"), getJSON(B + "/v1/fees/mempool-blocks"),
      getJSON(B + "/v1/blocks"), getJSON(B + "/v1/difficulty-adjustment")
    ]).then(function (r) {
      var v = function (i) { return r[i].status === "fulfilled" ? r[i].value : null; };
      if (!v(0) || !v(1)) return;
      st = { m: v(0), f: v(1), mb: v(2), blocks: v(3), adj: v(4) };
      renderStatus();
    });
  }
  function renderStatus() {
    var $ = function (id) { return document.getElementById(id); };
    var clock = new Date();
    $("mb-clock").textContent = clock.toLocaleTimeString(isPt() ? "pt-BR" : "en-US", { hour: "2-digit", minute: "2-digit" });
    if (!st) return;
    var f = st.f, m = st.m, minFee = f.minimumFee || 1;
    var backlog = (m.fee_histogram || []).reduce(function (a, e) { return e[0] >= minFee ? a + e[1] : a; }, 0);
    var blocks = backlog / 1e6, half = f.halfHourFee, lv = LEVELS[level(half)];
    var tiers = [f.fastestFee, f.halfHourFee, f.hourFee, f.economyFee];
    var rec = tiers[lv.tier];
    $("mb-title").textContent = "82.4 MH/s · ⛽" + lv.dot + " " + feerate(rec) + " sat/vB";
    $("m-level").textContent = lv.dot + " " + lv.name;
    $("m-bar").style.width = (barFrac(half) * 100).toFixed(1) + "%";
    $("m-bar").style.background = lv.color;
    $("m-queue").textContent = "taxa p/ ~30 min: " + feerate(half) + " sat/vB";
    var cheap = m.vsize - backlog;
    $("m-mempool").textContent = brBig(m.count) + " tx · " + BR(m.vsize / 1e6, 1) + " vMB · fila pagante " + BR(blocks, blocks < 10 ? 1 : 0) + (blocks <= 1 ? " bloco" : " blocos") + (cheap > m.vsize * 0.5 ? " (maioria abaixo de " + feerate(minFee) + " sat/vB)" : "");
    if (st.mb && st.mb[0]) {
      var nb = st.mb[0], fr = nb.feeRange;
      $("m-next").textContent = feerate(fr[0]) + " – " + feerate(fr[fr.length - 1]) + " sat/vB · mediana " + feerate(nb.medianFee) + " · " + brBig(nb.nTx) + " tx";
    }
    tiers.forEach(function (t, i) {
      var sats = Math.round(t * 141);
      $("m-f" + i).textContent = feerate(t) + " sat/vB · " + BR(sats) + " sats" + money(sats / 1e8);
    });
    $("m-rec").textContent = "✅ Recomendado: " + feerate(rec) + " sat/vB — " + lv.why;
    if (st.blocks && st.blocks[0]) {
      var b = st.blocks[0], pool = (b.extras && b.extras.pool && b.extras.pool.name) || "?";
      $("m-block").textContent = "#" + BR(b.height) + " · " + pool + " · há " + dur(Date.now() / 1000 - b.timestamp);
    }
    if (st.adj) {
      var a = st.adj;
      $("m-adj").textContent = BR(a.progressPercent) + "% · " + (a.difficultyChange >= 0 ? "+" : "−") + BR(Math.abs(a.difficultyChange), 2) + "% em ~" + dur(a.remainingTime / 1000);
    }
    var brl = isPt() && net.brl;
    if (net.usd || net.brl) $("m-price").textContent = brl ? "R$ " + BR(net.brl) : "US$ " + BR(net.usd);
    var per = net.diff * 4294967296, H = 8.24e7;
    $("m-chance").textContent = odds(-Math.expm1(-H * 86400 / per)) + " · bloco a cada ~" + dur(per / H);
    document.querySelectorAll(".levels tr[data-level]").forEach(function (tr) {
      tr.classList.toggle("now", +tr.getAttribute("data-level") === level(half));
    });
  }

  function renderAll() { renderTicker(); renderCalc(); renderReleases(); renderStatus(); }
  setLang(root.lang);
  loadNet();
  setInterval(loadNet, 60000);
  loadStatus();
  setInterval(loadStatus, 30000);
  setInterval(renderStatus, 15000);
})();
