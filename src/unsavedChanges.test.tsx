import type { MutableRefObject } from "react";
import { describe, expect, it, vi } from "vitest";
import { createNativeCloseHandler } from "./unsavedChanges";

describe("未保存変更の確認ダイアログ", () => {
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
});
