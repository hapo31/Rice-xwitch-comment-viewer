import { expect, it, vi } from "vitest";
import { routeAuthStorageWarning } from "./authWarnings";

const profile = { userId: "user-id", login: "viewer", scopes: ["user:read:chat"], expiresIn: 3600 };

it("routes a Device Code storage warning to notifications and system Chat", () => {
  const reportNotification = vi.fn();
  const addSystemChatMessage = vi.fn();
  const warning = "認証情報は今回の起動中だけ有効です。";

  routeAuthStorageWarning(
    { status: "authorized", profile, storageWarning: warning },
    reportNotification,
    addSystemChatMessage,
  );

  expect(reportNotification).toHaveBeenCalledWith("warning", "system", warning);
  expect(addSystemChatMessage).toHaveBeenCalledWith(warning);
});

it("does not create a warning route when storage succeeds", () => {
  const reportNotification = vi.fn();
  const addSystemChatMessage = vi.fn();

  routeAuthStorageWarning({ profile }, reportNotification, addSystemChatMessage);

  expect(reportNotification).not.toHaveBeenCalled();
  expect(addSystemChatMessage).not.toHaveBeenCalled();
});
