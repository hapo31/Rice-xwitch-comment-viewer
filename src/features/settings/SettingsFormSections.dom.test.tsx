import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { AppSettingsPatch, BouyomiConnectionDiagnostics } from "../../types";
import { defaultSpeechSettings, defaultTwitchSettings } from "./defaults";
import {
  ConnectionDiagnosticsSection,
  SettingsActionsProvider,
  SpeechTestSection,
} from "./SettingsFormSections";

function renderSection(
  children: React.ReactNode,
  {
    onSpeechDiagnostics = vi.fn(async () => diagnostics),
    onSpeechTest = vi.fn(),
  }: {
    onSpeechDiagnostics?: () => Promise<BouyomiConnectionDiagnostics>;
    onSpeechTest?: (text?: string) => void;
  } = {},
) {
  return render(
    <SettingsActionsProvider
      value={{
        twitch: defaultTwitchSettings(),
        savedSpeech: defaultSpeechSettings(),
        isDirty: false,
        onSettingsUpdate: async (_patch: AppSettingsPatch) => true,
        onSpeechHealthCheck: vi.fn(),
        onSpeechDiagnostics,
        onSpeechTest,
      }}
    >
      {children}
    </SettingsActionsProvider>,
  );
}

const diagnostics: BouyomiConnectionDiagnostics = {
  configuredAddr: "127.0.0.1:50001",
  attempted: [
    {
      addr: "127.0.0.1:50001",
      status: "connected",
      message: "接続しました。",
      elapsedMs: 12,
    },
  ],
  recommendation: "棒読みちゃんへ接続できました。",
};

describe("independent Settings operations", () => {
  it("runs diagnostics and displays its result through the limited action provider", async () => {
    const user = userEvent.setup();
    const onSpeechDiagnostics = vi.fn(async () => diagnostics);
    renderSection(<ConnectionDiagnosticsSection />, { onSpeechDiagnostics });

    await user.click(screen.getByRole("button", { name: "診断" }));

    expect(onSpeechDiagnostics).toHaveBeenCalledTimes(1);
    expect(await screen.findByText(diagnostics.recommendation)).toBeInTheDocument();
    expect(screen.getByText("127.0.0.1:50001")).toBeInTheDocument();
  });

  it("submits only the edited test speech through the limited action provider", async () => {
    const user = userEvent.setup();
    const onSpeechTest = vi.fn();
    renderSection(<SpeechTestSection />, { onSpeechTest });

    const input = screen.getByLabelText("テスト文");
    await user.clear(input);
    await user.type(input, "読み上げ確認");
    await user.click(screen.getByRole("button", { name: "テスト読み上げ" }));

    expect(onSpeechTest).toHaveBeenCalledWith("読み上げ確認");
  });
});
