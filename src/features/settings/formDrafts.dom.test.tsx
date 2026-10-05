import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";
import type { AppSettings } from "../../types";
import { UnsavedChangesContext, type UnsavedChange } from "../../unsavedChanges";
import { FilterView } from "../filter/FilterView";
import { SettingsView } from "./SettingsView";
import { defaultSpeechSettings, defaultTwitchSettings } from "./defaults";

vi.mock("../../tauri/client", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../tauri/client")>()),
  authorizeSpeechEndpoint: vi.fn(async () => undefined),
}));

const makeSettings = (): AppSettings => ({
  twitch: { ...defaultTwitchSettings },
  speech: { ...defaultSpeechSettings, blockedUsers: [], blockedWords: [] },
  launcher: { items: [] },
});

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((complete) => {
    resolve = complete;
  });
  return { promise, resolve };
}

function SettingsSaveHarness({ saveResult }: { saveResult: Promise<void> }) {
  const [settings, setSettings] = useState(makeSettings);
  async function onSettingsUpdate(patch: { speech?: Partial<AppSettings["speech"]> }) {
    await saveResult;
    setSettings((current) => ({
      ...current,
      speech: { ...current.speech, ...patch.speech },
    }));
    return true;
  }

  return (
    <SettingsView
      settings={settings}
      onSettingsUpdate={onSettingsUpdate}
      onSpeechHealthCheck={() => undefined}
      onSpeechDiagnostics={async () => ({
        configuredAddr: "127.0.0.1:50001",
        attempted: [],
        recommendation: "",
      })}
      onSpeechTest={() => undefined}
    />
  );
}

describe("Filter and Settings form drafts", () => {
  it("keeps Filter input through same-value reloads and unrelated setting updates", async () => {
    const user = userEvent.setup();
    const initial = makeSettings();
    const onSettingsUpdate = vi.fn(async () => true);
    const { rerender } = render(
      <FilterView settings={initial} onSettingsUpdate={onSettingsUpdate} />,
    );
    const words = screen.getByLabelText("NG ワード");
    await user.type(words, "draft word");

    rerender(
      <FilterView
        settings={{
          ...initial,
          speech: { ...initial.speech, blockedWords: [...initial.speech.blockedWords] },
        }}
        onSettingsUpdate={onSettingsUpdate}
      />,
    );
    expect(screen.getByLabelText("NG ワード")).toHaveValue("draft word");

    rerender(
      <FilterView
        settings={{
          ...initial,
          speech: { ...initial.speech, bouyomiHost: "voice.example" },
        }}
        onSettingsUpdate={onSettingsUpdate}
      />,
    );
    expect(screen.getByLabelText("NG ワード")).toHaveValue("draft word");
  });

  it("keeps a Settings edit back to the original value while its save is pending", async () => {
    const user = userEvent.setup();
    const initial = makeSettings();
    const save = deferred<boolean>();
    const onSettingsUpdate = vi.fn(() => save.promise);
    const { rerender } = render(
      <SettingsView
        settings={initial}
        onSettingsUpdate={onSettingsUpdate}
        onSpeechHealthCheck={() => undefined}
        onSpeechDiagnostics={async () => ({
          configuredAddr: "127.0.0.1:50001",
          attempted: [],
          recommendation: "",
        })}
        onSpeechTest={() => undefined}
      />,
    );

    await user.clear(screen.getByLabelText("ポート"));
    await user.type(screen.getByLabelText("ポート"), "50002");
    rerender(
      <SettingsView
        settings={{ ...initial, speech: { ...initial.speech, bouyomiVolume: 42 } }}
        onSettingsUpdate={onSettingsUpdate}
        onSpeechHealthCheck={() => undefined}
        onSpeechDiagnostics={async () => ({
          configuredAddr: "127.0.0.1:50001",
          attempted: [],
          recommendation: "",
        })}
        onSpeechTest={() => undefined}
      />,
    );
    await user.click(screen.getByRole("button", { name: "設定を保存" }));
    expect(onSettingsUpdate).toHaveBeenCalledWith({ speech: { bouyomiPort: 50002 } });

    await user.clear(screen.getByLabelText("ポート"));
    await user.type(screen.getByLabelText("ポート"), "50001");
    const saved = {
      ...initial,
      speech: { ...initial.speech, bouyomiPort: 50002 },
    };
    rerender(
      <SettingsView
        settings={saved}
        onSettingsUpdate={onSettingsUpdate}
        onSpeechHealthCheck={() => undefined}
        onSpeechDiagnostics={async () => ({
          configuredAddr: "127.0.0.1:50002",
          attempted: [],
          recommendation: "",
        })}
        onSpeechTest={() => undefined}
      />,
    );
    save.resolve(true);

    await waitFor(() => expect(screen.getByLabelText("ポート")).toHaveValue("50001"));
    expect(screen.getByRole("button", { name: "設定を保存" })).toBeInTheDocument();
  });

  it("keeps a failed Filter save draft available", async () => {
    const user = userEvent.setup();
    const initial = makeSettings();
    const onSettingsUpdate = vi.fn(async () => false);
    render(<FilterView settings={initial} onSettingsUpdate={onSettingsUpdate} />);

    await user.type(screen.getByLabelText("NG ワード"), "retain me");
    await user.click(screen.getByRole("button", { name: "設定を保存" }));

    expect(onSettingsUpdate).toHaveBeenCalledTimes(1);
    expect(screen.getByLabelText("NG ワード")).toHaveValue("retain me");
    expect(screen.getByRole("button", { name: "設定を保存" })).toBeInTheDocument();
  });

  it("keeps a failed Settings save draft available", async () => {
    const user = userEvent.setup();
    const initial = makeSettings();
    const onSettingsUpdate = vi.fn(async () => false);
    render(
      <SettingsView
        settings={initial}
        onSettingsUpdate={onSettingsUpdate}
        onSpeechHealthCheck={() => undefined}
        onSpeechDiagnostics={async () => ({
          configuredAddr: "127.0.0.1:50001",
          attempted: [],
          recommendation: "",
        })}
        onSpeechTest={() => undefined}
      />,
    );

    await user.clear(screen.getByLabelText("ホスト"));
    await user.type(screen.getByLabelText("ホスト"), "voice.example");
    await user.click(screen.getByRole("button", { name: "設定を保存" }));

    expect(onSettingsUpdate).toHaveBeenCalledTimes(1);
    expect(screen.getByLabelText("ホスト")).toHaveValue("voice.example");
    expect(screen.getByRole("button", { name: "設定を保存" })).toBeInTheDocument();
  });

  it("releases the pending marker when the settings update rejects", async () => {
    const user = userEvent.setup();
    const initial = makeSettings();
    let registered: UnsavedChange | undefined;
    const registry = {
      register: (_id: string, change: UnsavedChange) => {
        registered = change;
      },
      unregister: () => undefined,
    };
    const onSettingsUpdate = vi.fn(async () => {
      throw new Error("save rejected");
    });
    const { rerender } = render(
      <UnsavedChangesContext.Provider value={registry}>
        <FilterView settings={initial} onSettingsUpdate={onSettingsUpdate} />
      </UnsavedChangesContext.Provider>,
    );

    await user.type(screen.getByLabelText("NG ワード"), "retain me");
    await expect(registered?.save()).rejects.toThrow("save rejected");

    rerender(
      <UnsavedChangesContext.Provider value={registry}>
        <FilterView
          settings={{
            ...initial,
            speech: { ...initial.speech, blockedWords: ["server value"] },
          }}
          onSettingsUpdate={onSettingsUpdate}
        />
      </UnsavedChangesContext.Provider>,
    );
    fireEvent.change(screen.getByLabelText("NG ワード"), {
      target: { value: "server value" },
    });
    rerender(
      <UnsavedChangesContext.Provider value={registry}>
        <FilterView
          settings={{
            ...initial,
            speech: { ...initial.speech, blockedWords: ["new server value"] },
          }}
          onSettingsUpdate={onSettingsUpdate}
        />
      </UnsavedChangesContext.Provider>,
    );

    await waitFor(() => expect(screen.getByLabelText("NG ワード")).toHaveValue("new server value"));
  });

  it("keeps the Settings edit when the save harness updates props before resolving", async () => {
    const user = userEvent.setup();
    const save = deferred<void>();
    render(<SettingsSaveHarness saveResult={save.promise} />);

    await user.clear(screen.getByLabelText("ポート"));
    await user.type(screen.getByLabelText("ポート"), "50002");
    await user.click(screen.getByRole("button", { name: "設定を保存" }));
    await user.clear(screen.getByLabelText("ポート"));
    await user.type(screen.getByLabelText("ポート"), "50001");

    await act(async () => save.resolve());

    await waitFor(() => expect(screen.getByLabelText("ポート")).toHaveValue("50001"));
    expect(screen.getByRole("button", { name: "設定を保存" })).toBeInTheDocument();
  });

  it("keeps an edit to another Settings field made while saving", async () => {
    const user = userEvent.setup();
    const initial = makeSettings();
    const save = deferred<boolean>();
    const onSettingsUpdate = vi.fn(() => save.promise);
    const { rerender } = render(
      <SettingsView
        settings={initial}
        onSettingsUpdate={onSettingsUpdate}
        onSpeechHealthCheck={() => undefined}
        onSpeechDiagnostics={async () => ({
          configuredAddr: "127.0.0.1:50001",
          attempted: [],
          recommendation: "",
        })}
        onSpeechTest={() => undefined}
      />,
    );

    await user.clear(screen.getByLabelText("ホスト"));
    await user.type(screen.getByLabelText("ホスト"), "voice.example");
    await user.click(screen.getByRole("button", { name: "設定を保存" }));
    await user.clear(screen.getByLabelText("ポート"));
    await user.type(screen.getByLabelText("ポート"), "50002");
    rerender(
      <SettingsView
        settings={{ ...initial, speech: { ...initial.speech, bouyomiHost: "voice.example" } }}
        onSettingsUpdate={onSettingsUpdate}
        onSpeechHealthCheck={() => undefined}
        onSpeechDiagnostics={async () => ({
          configuredAddr: "voice.example:50001",
          attempted: [],
          recommendation: "",
        })}
        onSpeechTest={() => undefined}
      />,
    );
    save.resolve(true);

    await waitFor(() => expect(screen.getByLabelText("ポート")).toHaveValue("50002"));
    expect(screen.getByRole("button", { name: "設定を保存" })).toBeInTheDocument();
  });

  it("discards a Filter draft through the unsaved changes registry", async () => {
    const user = userEvent.setup();
    const initial = makeSettings();
    let registered: UnsavedChange | undefined;
    const registry = {
      register: (_id: string, change: UnsavedChange) => {
        registered = change;
      },
      unregister: () => undefined,
    };
    render(
      <UnsavedChangesContext.Provider value={registry}>
        <FilterView settings={initial} onSettingsUpdate={async () => true} />
      </UnsavedChangesContext.Provider>,
    );

    await user.type(screen.getByLabelText("NG ワード"), "discard me");
    expect(registered?.isDirty).toBe(true);
    await act(async () => registered?.discard());

    expect(screen.getByLabelText("NG ワード")).toHaveValue("");
    expect(screen.queryByRole("button", { name: "設定を保存" })).not.toBeInTheDocument();
  });

  it("clears endpoint consent only when the saved connection target changes", async () => {
    const user = userEvent.setup();
    const initial = makeSettings();
    initial.speech.bouyomiRemoteMode = true;
    const props = {
      onSettingsUpdate: vi.fn(async () => true),
      onSpeechHealthCheck: () => undefined,
      onSpeechDiagnostics: async () => ({
        configuredAddr: "127.0.0.1:50001",
        attempted: [],
        recommendation: "",
      }),
      onSpeechTest: () => undefined,
    };
    const { rerender } = render(<SettingsView settings={initial} {...props} />);

    await user.click(
      screen.getByRole("button", { name: "保存済みの接続先をネイティブ確認で許可" }),
    );
    expect(screen.getByRole("status")).toHaveTextContent(
      "この起動中だけ、確認した接続先を許可しました。",
    );

    rerender(
      <SettingsView
        settings={{ ...initial, speech: { ...initial.speech, bouyomiVolume: 44 } }}
        {...props}
      />,
    );
    expect(screen.getByRole("status")).toHaveTextContent(
      "この起動中だけ、確認した接続先を許可しました。",
    );

    rerender(
      <SettingsView
        settings={{ ...initial, speech: { ...initial.speech, bouyomiHost: "voice.example" } }}
        {...props}
      />,
    );
    expect(
      screen.queryByText(
        "この起動中だけ、確認した接続先を許可しました。接続確認・診断を実行してください。",
      ),
    ).not.toBeInTheDocument();
  });

  it("keeps a Filter edit back to the original value while its save is pending", async () => {
    const user = userEvent.setup();
    const initial = makeSettings();
    const save = deferred<boolean>();
    const onSettingsUpdate = vi.fn(() => save.promise);
    const { rerender } = render(
      <FilterView settings={initial} onSettingsUpdate={onSettingsUpdate} />,
    );

    await user.type(screen.getByLabelText("NG ワード"), "first");
    await user.click(screen.getByRole("button", { name: "設定を保存" }));
    fireEvent.change(screen.getByLabelText("NG ワード"), { target: { value: "" } });
    const saved = {
      ...initial,
      speech: { ...initial.speech, blockedWords: ["first"] },
    };
    rerender(<FilterView settings={saved} onSettingsUpdate={onSettingsUpdate} />);
    save.resolve(true);

    await waitFor(() => expect(screen.getByLabelText("NG ワード")).toHaveValue(""));
    expect(screen.getByRole("button", { name: "設定を保存" })).toBeInTheDocument();
  });
});
