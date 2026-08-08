import type { MutableRefObject } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { createNativeCloseHandler, UnsavedChangesDialog, type UnsavedChange } from "./unsavedChanges";

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

  it("protects an immediate native close request after a draft becomes dirty without re-registering", () => {
    const activeChangeRef: MutableRefObject<UnsavedChange | undefined> = { current: undefined };
    let confirmationRequests = 0;
    const onCloseRequested = createNativeCloseHandler(activeChangeRef, () => {
      confirmationRequests += 1;
    });
    const event = { preventDefault: () => undefined };
    const preventDefault = vi.spyOn(event, "preventDefault");

    activeChangeRef.current = { isDirty: true, save: async () => true, discard: () => undefined };
    onCloseRequested(event);

    expect(preventDefault).toHaveBeenCalledOnce();
    expect(confirmationRequests).toBe(1);
  });
});
