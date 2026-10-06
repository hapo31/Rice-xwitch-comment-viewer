import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ActiveOperationsExitDialog, UnsavedChangesDialog } from "./unsavedChanges";

describe("confirmation modal dialogs", () => {
  it("opens a shared modal, focuses its first action, handles Escape as cancel, and restores focus", async () => {
    const onCancel = vi.fn();
    const { rerender } = render(
      <div>
        <button type="button">終了要求</button>
      </div>,
    );
    const trigger = screen.getByRole("button", { name: "終了要求" });
    trigger.focus();
    rerender(
      <div>
        <button type="button">終了要求</button>
        <UnsavedChangesDialog
          onSave={() => undefined}
          onDiscard={() => undefined}
          onCancel={onCancel}
        />
      </div>,
    );

    const dialog = screen.getByRole("dialog", { name: "未保存の変更があります" });
    expect(dialog).toBeVisible();
    expect(dialog).toHaveAttribute("aria-modal", "true");
    expect(dialog).toHaveAccessibleDescription("保存してから移動または終了しますか？");
    expect(screen.getByRole("button", { name: "保存して続ける" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "破棄して続ける" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "キャンセル" })).toHaveFocus();

    fireEvent.keyDown(dialog, { key: "Escape" });
    expect(onCancel).toHaveBeenCalledOnce();

    rerender(
      <div>
        <button type="button">終了要求</button>
      </div>,
    );
    await waitFor(() => expect(screen.getByRole("button", { name: "終了要求" })).toHaveFocus());
  });

  it("keeps cancellation available while saving and announces the pending operation", () => {
    const onCancel = vi.fn();
    render(
      <UnsavedChangesDialog
        onSave={() => undefined}
        onDiscard={() => undefined}
        onCancel={onCancel}
        isSaving
        saveDisabled
      />,
    );

    const dialog = screen.getByRole("dialog");
    expect(screen.getByRole("button", { name: "保存しています…" })).toBeDisabled();
    fireEvent.keyDown(dialog, { key: "Escape" });
    expect(onCancel).toHaveBeenCalledOnce();
  });

  it("moves focus to the app shell when the dialog trigger no longer exists", async () => {
    const { rerender } = render(
      <div>
        <main data-modal-focus-fallback tabIndex={-1} />
        <button type="button">遷移元</button>
      </div>,
    );
    const trigger = screen.getByRole("button", { name: "遷移元" });
    trigger.focus();
    expect(trigger).toHaveFocus();
    rerender(
      <div>
        <main data-modal-focus-fallback tabIndex={-1} />
        <UnsavedChangesDialog
          onSave={() => undefined}
          onDiscard={() => undefined}
          onCancel={() => undefined}
        />
      </div>,
    );
    rerender(
      <div>
        <main data-modal-focus-fallback tabIndex={-1} />
      </div>,
    );

    await waitFor(() =>
      expect(document.querySelector("[data-modal-focus-fallback]")).toHaveFocus(),
    );
  });

  it("does not let Escape dismiss an exit confirmation while shutdown is in progress", () => {
    const onCancel = vi.fn();
    render(
      <ActiveOperationsExitDialog onConfirm={() => undefined} onCancel={onCancel} isClosing />,
    );

    const dialog = screen.getByRole("dialog", { name: "配信支援を停止して終了しますか？" });
    fireEvent.keyDown(dialog, { key: "Escape" });
    expect(onCancel).not.toHaveBeenCalled();
    expect(dialog).toBeVisible();
    expect(screen.getByRole("button", { name: "停止しています…" })).toBeDisabled();
  });
});
