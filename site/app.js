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

  function renderAll() { renderTicker(); renderCalc(); renderReleases(); }
  setLang(root.lang);
  loadNet();
  setInterval(loadNet, 60000);
})();
