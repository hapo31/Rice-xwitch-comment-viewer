// Included only by the Windows libtest fixture. Uses the production frontend,
// production IPC commands and unchanged ACL; no test-only backend permission.
(async () => {
  let peak = 0;
  let baseline = 0;
  let sampling;
  const sample = () => { if (performance.memory) peak = Math.max(peak, performance.memory.usedJSHeapSize); };
  const invoke = async (...args) => {
    try { return await window.__TAURI_INTERNALS__.invoke(...args); }
    finally { sample(); }
  };
  const wait = async (predicate) => {
    const deadline = performance.now() + 30000;
    while (!predicate()) {
      if (performance.now() > deadline) throw new Error("native UI readiness timed out");
      await new Promise((resolve) => setTimeout(resolve, 20));
    }
  };
  const rejected = async (command, args) => {
    try { await invoke(command, args); }
    catch (error) { if (!String(error).length) throw new Error("missing recovery message"); return; }
    throw new Error(`${command} accepted an invalid payload`);
  };
  // Exact structural comparison, without creating two additional 8MiB JSON
  // strings solely for instrumentation (Unicode would widen their buffers).
  const equal = (left, right) => {
    if (left === right) return true;
    if (!left || !right || typeof left !== "object" || typeof right !== "object") return false;
    if (Array.isArray(left) !== Array.isArray(right)) return false;
    const keys = Object.keys(left);
    return keys.length === Object.keys(right).length && keys.every((key) => Object.hasOwn(right, key) && equal(left[key], right[key]));
  };
  const verifyStyles = () => {
    let checks = 0;
    const check = (condition, message) => {
      if (!condition) throw new Error(`stylesheet compatibility: ${message}`);
      checks += 1;
    };
    const main = document.querySelector("main");
    const article = main?.querySelector("article");
    const icon = article?.querySelector("img");
    if (!main || !article || !icon) throw new Error("missing actual Launcher style targets");
    const rem = Number.parseFloat(getComputedStyle(document.documentElement).fontSize);
    check(getComputedStyle(main).backgroundColor === "rgb(9, 9, 11)", "main background");
    check(getComputedStyle(main.querySelector("header")).backgroundColor === "rgb(24, 24, 27)", "header background");
    check(getComputedStyle(main.querySelector("h1")).color === "rgb(244, 244, 245)", "heading color");
    check(getComputedStyle(main.querySelector("p")).color === "rgb(161, 161, 170)", "readable secondary text");
    check(article.getBoundingClientRect().height === 156, "tile row height");
    check(Math.abs(icon.getBoundingClientRect().width - 4 * rem) < 0.1, "icon width");
    check(Math.abs(icon.getBoundingClientRect().height - 4 * rem) < 0.1, "icon height");
    const iconFilter = getComputedStyle(icon).filter;
    check(iconFilter.includes("0.1)") && iconFilter.includes("0.06)"), "unchanged two-layer icon shadow");
    check(getComputedStyle(document.body).fontFamily.includes("Yu Gothic UI"), "Japanese font fallback");
    const probe = document.createElement("input");
    probe.className = "bg-zinc-850 text-zinc-400 font-mono outline-hidden focus-visible:ring-2 focus-visible:ring-sky-400";
    probe.setAttribute("aria-label", "isolated stylesheet test");
    document.body.append(probe);
    try {
      check(getComputedStyle(probe).backgroundColor === "rgb(27, 27, 32)", "custom panel palette");
      check(getComputedStyle(probe).color === "rgb(161, 161, 170)", "input text palette");
      const scaleOutput = document.querySelector('output[aria-label="現在の表示倍率"]');
      check(scaleOutput && getComputedStyle(scaleOutput).fontFamily.includes("Cascadia Mono"), "monospace fallback");
      check(getComputedStyle(article.querySelector("button")).cursor === "pointer", "button affordance");
      probe.focus();
      check(document.activeElement === probe && probe.matches(":focus-visible"), "text input focus");
      check(getComputedStyle(probe).boxShadow.includes("rgb(56, 189, 248)"), "keyboard focus color");
      check(getComputedStyle(probe).boxShadow.includes("2px"), "keyboard focus width");
    } finally {
      probe.remove();
    }
    return checks;
  };
  let result;
  try {
    await wait(() => window.__TAURI_INTERNALS__?.invoke && document.querySelector('a[aria-label="Launcher"]'));
    if (!performance.memory?.usedJSHeapSize) throw new Error("WebView heap measurement is unavailable");
    baseline = performance.memory.usedJSHeapSize;
    peak = baseline;
    sampling = setInterval(sample, 20);
    const getStart = performance.now();
    const before = await invoke("settings_get");
    let getMs = performance.now() - getStart;
    if (before.launcher.items.length !== 200) throw new Error("maximum fixture is missing");
    const start = performance.now();
    document.querySelector('a[aria-label="Launcher"]').click();
    await wait(() => {
      const icons = [...document.querySelectorAll("article img")];
      return icons.length === 200 && icons.every((icon) => icon.complete && icon.naturalWidth === 128 && icon.naturalHeight === 128);
    });
    await Promise.all([...document.querySelectorAll("article img")].map((icon) => icon.decode()));
    await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
    const renderMs = performance.now() - start;
    const styleChecks = verifyStyles();
    const first = before.launcher.items[0];
    const edit = { id: first.id, displayName: first.displayName, order: first.order };
    await rejected("settings_update", { patch: { launcher: { items: [{ ...edit, target: "C:\\injected.exe" }] } } });
    await rejected("settings_update", { patch: { launcher: { items: [{ ...edit, iconDataUrl: "https://example.com/tracker.png" }] } } });
    await rejected("settings_update", { patch: { launcher: { items: [{ ...edit, id: "not-registered" }] } } });
    await rejected("launcher_add", { paths: ["x".repeat(4096) + ".exe"] });
    let validationRejected = 0;
    for (const [command, args, field] of [
      ["settings_update", { patch: { twitch: { channelLogin: "@invalid" } } }, "twitch.channelLogin"],
      ["twitch_connect", { channelLogin: "ab" }, "twitch.channelLogin"],
      ["settings_update", { patch: { speech: { bouyomiHost: "" } } }, "speech.bouyomiHost"],
      ["settings_update", { patch: { speech: { bouyomiPort: 0 } } }, "speech.bouyomiPort"],
      ["settings_update", { patch: { speech: { blockedWords: ["x".repeat(501)] } } }, "speech.blockedWords"],
      ["settings_update", { patch: { speech: { autoSpeek: true } } }, "speech"],
    ]) {
      let failure;
      try { await invoke(command, args); } catch (error) { failure = error; }
      if (!failure || failure.field !== field || typeof failure.code !== "string" || typeof failure.recovery !== "string") throw new Error(`structured validation missing: ${command}/${field}`);
      validationRejected += 1;
    }
    const secondGetStart = performance.now();
    const after = await invoke("settings_get");
    getMs = Math.max(getMs, performance.now() - secondGetStart);
    if (!equal(before, after)) throw new Error("invalid requests mutated settings");
    peak = Math.max(peak, performance.memory.usedJSHeapSize);
    await invoke("settings_update", { patch: { speech: { bouyomiHost: "10.0.0.1", bouyomiRemoteMode: true } } });
    let remoteFailure;
    try { await invoke("speech_health_probe"); } catch (error) { remoteFailure = error; }
    if (typeof remoteFailure !== "string" || !remoteFailure.includes("外部へは送信していません")) throw new Error("renderer flag bypassed native consent");
    const diagnostics = await invoke("speech_connection_diagnostics");
    if (!diagnostics.attempted[0].message.includes("外部へは送信していません")) throw new Error("diagnostics bypassed destination policy");
    await invoke("settings_update", { patch: { speech: { bouyomiHost: before.speech.bouyomiHost, bouyomiRemoteMode: before.speech.bouyomiRemoteMode } } });
    if (!equal(before, await invoke("settings_get"))) throw new Error("remote rejection fixture did not restore settings");
    result = { count: 200, getMs, renderMs, incrementalJsHeap: Math.max(0, peak - baseline), baselineJsHeap: baseline, peakJsHeap: peak, rejected: 4, validationRejected, remoteRejected: 2, unchanged: true, styleChecks };
  } catch (error) {
    result = { error: String(error).slice(0, 160) };
  } finally {
    clearInterval(sampling);
  }
  // This isolated result fits the validated NG-word domain (<=500 chars), not
  // a fake Twitch login. No new production command/permission or TCP is used.
  await invoke("settings_update", { patch: { speech: { blockedWords: [`RICE_LAUNCHER_RESULT ${JSON.stringify(result)}`] } } });
})();
