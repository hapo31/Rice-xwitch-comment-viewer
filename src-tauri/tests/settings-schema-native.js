// Windows libtest only: real production startup, commands and unchanged ACL.
(async () => {
  try {
    const invoke = (...args) => window.__TAURI_INTERNALS__.invoke(...args);
    const before = await invoke("settings_get");
    if (before.twitch.autoConnect || before.twitch.channelLogin || before.launcher.items.length) {
      throw new Error("future schema must use safe defaults without auto-connect");
    }
    try {
      await invoke("settings_update", { patch: { twitch: { channelLogin: "new_channel" } } });
      throw new Error("future settings_update unexpectedly succeeded");
    } catch (error) {
      if (error?.code !== "unsupportedSchema" || !error.message.includes("元ファイル")) throw error;
    }
    try {
      // Registration only; never launch a test or user executable.
      await invoke("launcher_add", { paths: [window.__RICE_SCHEMA_TARGET] });
      throw new Error("future launcher_add unexpectedly succeeded");
    } catch (error) {
      if (!String(error).includes("保存しません")) throw error;
    }
    const after = await invoke("settings_get");
    if (JSON.stringify(after) !== JSON.stringify(before)) throw new Error("rejected updates changed memory");
    window.location.hash = "rice-schema-ok";
  } catch (error) {
    console.error("Native settings schema failed", error);
    window.location.hash = "rice-schema-failed";
  }
})();
