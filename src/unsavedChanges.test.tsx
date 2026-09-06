import type { MutableRefObject } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { ActiveOperationsExitDialog, createNativeCloseHandler, UnsavedChangesDialog } from "./unsavedChanges";

describe("未保存変更の確認ダイアログ", () => {
  it("画面遷移後に再入場する前の保存・破棄・キャンセルをキーボード操作できる", () => {
    const markup = renderToStaticMarkup(
      <UnsavedChangesDialog onSave={() => undefined} onDiscard={() => undefined} onCancel={() => undefined} />,
    );

    expect(markup).toContain('role="dialog"');
    expect(markup).toContain('aria-modal="true"');
    expect(markup).toContain('type="button" autofocus=""');
    expect(markup).toContain("保存して続ける");
    expect(markup).toContain("破棄して続ける");
    expect(markup).toContain("キャンセル");
  });

  it("protects an immediate native close request after an exit risk begins without re-registering", () => {
    const closeConfirmationRequiredRef: MutableRefObject<boolean> = { current: false };
    let confirmationRequests = 0;
    const onCloseRequested = createNativeCloseHandler(closeConfirmationRequiredRef, () => {
      confirmationRequests += 1;
    });
    const event = { preventDefault: () => undefined };
    const preventDefault = vi.spyOn(event, "preventDefault");

    closeConfirmationRequiredRef.current = true;
    onCloseRequested(event);

    expect(preventDefault).toHaveBeenCalledOnce();
    expect(confirmationRequests).toBe(1);
  });

  it("安全な状態の native close request は妨げない", () => {
    const closeConfirmationRequiredRef: MutableRefObject<boolean> = { current: false };
    const event = { preventDefault: () => undefined };
    const preventDefault = vi.spyOn(event, "preventDefault");

    createNativeCloseHandler(closeConfirmationRequiredRef, () => undefined)(event);

    expect(preventDefault).not.toHaveBeenCalled();
  });

  it("説明付きの終了確認で停止とキャンセルを選べる", () => {
    const markup = renderToStaticMarkup(
      <ActiveOperationsExitDialog onConfirm={() => undefined} onCancel={() => undefined} />,
    );

    expect(markup).toContain('role="dialog"');
    expect(markup).toContain("待機中の読み上げをクリアします");
    expect(markup).toContain("停止して終了");
    expect(markup).toContain("キャンセル");
  });
});
