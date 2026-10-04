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
    const first = before.launcher.items[0];
    const edit = { id: first.id, displayName: first.displayName, order: first.order };
    await rejected("settings_update", { patch: { launcher: { items: [{ ...edit, target: "C:\\injected.exe" }] } } });
    await rejected("settings_update", { patch: { launcher: { items: [{ ...edit, iconDataUrl: "https://example.com/tracker.png" }] } } });
    await rejected("settings_update", { patch: { launcher: { items: [{ ...edit, id: "not-registered" }] } } });
    await rejected("launcher_add", { paths: ["x".repeat(4096) + ".exe"] });
    const secondGetStart = performance.now();
    const after = await invoke("settings_get");
    getMs = Math.max(getMs, performance.now() - secondGetStart);
    if (JSON.stringify(before) !== JSON.stringify(after)) throw new Error("invalid requests mutated settings");
    peak = Math.max(peak, performance.memory.usedJSHeapSize);
    result = { count: 200, getMs, renderMs, incrementalJsHeap: Math.max(0, peak - baseline), baselineJsHeap: baseline, peakJsHeap: peak, rejected: 4, unchanged: true };
  } catch (error) {
    result = { error: String(error).slice(0, 500) };
  } finally {
    clearInterval(sampling);
  }
  // The isolated fixture uses this existing settings field as a bounded result
  // channel. It never connects to Twitch or touches a real account/settings.
  await invoke("settings_update", { patch: { twitch: { channelLogin: `RICE_LAUNCHER_RESULT ${JSON.stringify(result)}` } } });
})();
