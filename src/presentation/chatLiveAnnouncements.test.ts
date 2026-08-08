import { describe, expect, it } from "vitest";
import { ChatLiveAnnouncementController } from "./chatLiveAnnouncements";
import type { ChatMessage } from "../types";

function twitchMessage(id: string): ChatMessage {
  return {
    kind: "user",
    id,
    receivedAt: "2026-08-08T00:00:00Z",
    userDisplayName: "viewer",
    text: "こんにちは",
    status: "queued",
    platform: "twitch",
  };
}

function systemMessage(id: string): ChatMessage {
  return {
    kind: "system",
    id,
    receivedAt: "2026-08-08T00:00:00Z",
    userDisplayName: "system",
    text: "起動ガイド",
  };
}

describe("ChatLiveAnnouncementController", () => {
  it("does not announce messages that were already present when Chat opens", () => {
    const controller = new ChatLiveAnnouncementController();
    const message = twitchMessage("existing");

    controller.initialize([message]);
    controller.queueNewMessages([message]);

    expect(controller.takeSummary()).toBeUndefined();
  });

  it("summarizes a burst once and ignores duplicate events and system guidance", () => {
    const controller = new ChatLiveAnnouncementController();
    const first = twitchMessage("first");
    const second = twitchMessage("second");

    controller.initialize([]);
    controller.queueNewMessages([first]);
    controller.queueNewMessages([second, first, systemMessage("startup")]);

    expect(controller.takeSummary()).toBe("新しいチャットが2件届きました。");
    expect(controller.takeSummary()).toBeUndefined();
  });

  it("marks messages as seen while notifications are suppressed", () => {
    const controller = new ChatLiveAnnouncementController();
    const message = twitchMessage("suppressed");

    controller.initialize([]);
    controller.suppress([message]);
    controller.queueNewMessages([message]);

    expect(controller.takeSummary()).toBeUndefined();
  });
});
