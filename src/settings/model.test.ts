import { expect, it } from "vitest";
import { createDefaultAppSettings, createDefaultSpeechSettings } from "./model";

it("returns independent mutable defaults for each consumer", () => {
  const first = createDefaultAppSettings();
  const second = createDefaultAppSettings();

  first.speech.blockedUsers.push("viewer");
  first.launcher.items.push({
    id: "app",
    kind: "application",
    target: "C:/app.exe",
    displayName: "App",
    order: 0,
  });

  expect(second.speech.blockedUsers).toEqual([]);
  expect(second.launcher.items).toEqual([]);
  expect(createDefaultSpeechSettings().blockedUsers).toEqual([]);
});
