import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { UnsavedChangesDialog } from "./unsavedChanges";

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
});
