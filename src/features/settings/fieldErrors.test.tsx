import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { FloatingSaveButton } from "../../components/SettingsFormControls";
import { AuthView } from "../auth/AuthView";
import { initialAppState } from "../../stores/appStore";
import type { AppSettings } from "../../types";
import { SettingsView } from "./SettingsView";
import { defaultSpeechSettings, defaultTwitchSettings } from "./defaults";

const invalidSettings: AppSettings = {
  twitch: defaultTwitchSettings,
  speech: {
    ...defaultSpeechSettings,
    bouyomiHost: "",
    bouyomiPort: 0,
    bouyomiVoice: 30001,
  },
  launcher: { items: [] },
};

describe("field validation errors", () => {
  it("associates invalid Bouyomi fields and the disabled save button with their error text", () => {
    const markup = renderToStaticMarkup(
      <SettingsView
        settings={invalidSettings}
        onSettingsUpdate={() => undefined}
        onSpeechHealthCheck={() => undefined}
        onSpeechDiagnostics={async () => ({ configuredAddr: "127.0.0.1:50001", attempted: [], recommendation: "" })}
        onSpeechTest={() => undefined}
      />,
    );

    expect(markup).toContain('id="bouyomi-host" aria-invalid="true" aria-describedby="bouyomi-host-error"');
    expect(markup).toContain('id="bouyomi-port" inputMode="numeric" aria-invalid="true" aria-describedby="bouyomi-port-error"');
    expect(markup).toContain('id="bouyomi-voice" inputMode="numeric" aria-invalid="true" aria-describedby="bouyomi-voice-error"');
    expect(markup).toContain('id="bouyomi-host-error"');
    expect(markup).toContain("棒読みちゃんのホストを入力してください。");
    const saveMarkup = renderToStaticMarkup(
      <FloatingSaveButton visible disabled disabledReason="設定を保存できません。" onClick={() => undefined} />,
    );
    expect(saveMarkup).toContain('aria-describedby="settings-save-disabled-reason"');
    expect(saveMarkup).toContain('id="settings-save-disabled-reason"');
  });

  it("associates an invalid Twitch channel with its specific error", () => {
    const markup = renderToStaticMarkup(
      <AuthView
        state={{
          ...initialAppState,
          settings: { ...invalidSettings, twitch: { ...defaultTwitchSettings, channelLogin: "!" } },
        }}
        onSettingsUpdate={() => undefined}
        onTwitchStartAuth={() => undefined}
        onTwitchPollAuth={() => undefined}
        onTwitchValidateAuth={async () => false}
        onTwitchDisconnect={() => undefined}
        onOpenExternalUrl={() => undefined}
      />,
    );

    expect(markup).toContain('id="twitch-channel" aria-invalid="true" aria-describedby="twitch-channel-error"');
    expect(markup).toContain('id="twitch-channel-error"');
    expect(markup).toContain("Twitch チャンネル名は 3 から 25 文字の英数字またはアンダースコアで入力してください。");
  });
});
