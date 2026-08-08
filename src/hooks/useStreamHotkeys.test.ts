import { describe, expect, it } from "vitest";
import { getStreamHotkey } from "./useStreamHotkeys";

function keyEvent(overrides: Partial<KeyboardEvent> = {}) {
  return {
    altKey: false,
    code: "",
    ctrlKey: false,
    defaultPrevented: false,
    isComposing: false,
    key: "",
    metaKey: false,
    repeat: false,
    target: null,
    ...overrides,
  } as KeyboardEvent;
}

describe("stream hotkeys", () => {
  it("maps Space and S without modifiers to speech controls", () => {
    expect(getStreamHotkey(keyEvent({ code: "Space", key: " " }))).toBe("toggleSpeech");
    expect(getStreamHotkey(keyEvent({ key: "S" }))).toBe("skipSpeech");
  });

  it.each(["ctrlKey", "metaKey"] as const)("opens Settings with %s + comma", (modifier) => {
    expect(getStreamHotkey(keyEvent({ key: ",", [modifier]: true }))).toBe("openSettings");
  });

  it("does not interfere with text editing, composition, buttons, or repeated keys", () => {
    expect(
      getStreamHotkey(keyEvent({ code: "Space", target: { tagName: "INPUT" } as unknown as EventTarget })),
    ).toBeUndefined();
    expect(
      getStreamHotkey(keyEvent({ key: "s", target: { isContentEditable: true } as unknown as EventTarget })),
    ).toBeUndefined();
    expect(getStreamHotkey(keyEvent({ key: "s", isComposing: true }))).toBeUndefined();
    expect(getStreamHotkey(keyEvent({ key: "s", repeat: true }))).toBeUndefined();
    expect(
      getStreamHotkey(keyEvent({ code: "Space", target: { tagName: "BUTTON" } as unknown as EventTarget })),
    ).toBeUndefined();
  });
});
