import { expect, it } from "vitest";
import { tauriMock } from "./tauriMock";

it("mock controls delayed results, rejection and listener-leak detection", async () => {
  let resolve!: (value: string) => void;
  tauriMock.setCommand(
    "test_delayed",
    () =>
      new Promise<string>((done) => {
        resolve = done;
      }),
  );
  const pending = tauriMock.invoke("test_delayed");
  resolve("成功");
  await expect(pending).resolves.toBe("成功");
  tauriMock.rejectCommand("test_rejected", "明示エラー");
  await expect(tauriMock.invoke("test_rejected")).rejects.toThrow("明示エラー");
  const unlisten = await tauriMock.listen("test_event", () => undefined);
  expect(() => tauriMock.reset()).toThrow("leaked a native listener");
  unlisten();
  expect(tauriMock.listenerCount()).toBe(0);
});
