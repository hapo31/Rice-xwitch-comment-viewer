// Windows libtest only: real production startup, commands and unchanged ACL.
(async () => {
  let phase = "startup";
  const stage = (next) => {
    phase = next;
    // HashRouter owns the fragment. Keep a real route so Navigate cannot
    // replace the native observer's result with the default Chat location.
    window.location.hash = `/chat?riceSchemaStage=${next}`;
  };
  try {
    const deadline = performance.now() + 30000;
    while (!window.__TAURI_INTERNALS__?.invoke || !document.querySelector('a[aria-label="Chat"]')) {
      if (performance.now() > deadline) throw new Error("native UI readiness timed out");
      await new Promise((resolve) => setTimeout(resolve, 20));
    }
    const invoke = (...args) => window.__TAURI_INTERNALS__.invoke(...args);
    stage("get");
    const before = await invoke("settings_get");
    if (before.twitch.autoConnect || before.twitch.channelLogin || before.launcher.items.length) {
      throw new Error("future schema must use safe defaults without auto-connect");
    }
    stage("update");
    try {
      await invoke("settings_update", { patch: { twitch: { channelLogin: "new_channel" } } });
      throw new Error("future settings_update unexpectedly succeeded");
    } catch (error) {
      if (error?.code !== "unsupportedSchema" || !error.message.includes("元ファイル")) throw error;
    }
    stage("launcher");
    try {
      // Registration only; never launch a test or user executable.
      await invoke("launcher_add", { paths: [window.__RICE_SCHEMA_TARGET] });
      throw new Error("future launcher_add unexpectedly succeeded");
    } catch (error) {
      if (!String(error).includes("保存しません")) throw error;
    }
    stage("compare");
    const after = await invoke("settings_get");
    if (JSON.stringify(after) !== JSON.stringify(before)) throw new Error("rejected updates changed memory");
    window.location.hash = "/chat?riceSchemaResult=ok";
  } catch (error) {
    console.error("Native settings schema failed", error);
    window.location.hash = `/chat?riceSchemaResult=failed&stage=${phase}`;
  }
})();
