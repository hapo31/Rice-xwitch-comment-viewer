import type { AppState } from "../stores/appStore";

const startupGuideSessionKey = "rice.startup-guide-shown";

export interface StartupGuideSessionStorage {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

export function claimStartupGuideForSession(storage: StartupGuideSessionStorage): boolean {
  if (storage.getItem(startupGuideSessionKey)) {
    return false;
  }
  storage.setItem(startupGuideSessionKey, "true");
  return true;
}

export type StartupGuideAction = "login";

export interface StartupGuideMessage {
  id: string;
  receivedAt: string;
  userDisplayName: "system";
  text: string;
  action?: StartupGuideAction;
}

export function getStartupGuideMessages(state: AppState, receivedAt: string): StartupGuideMessage[] {
  const isAuthenticated = state.twitchAuthStatus === "authenticated";
  const channelLogin = state.settings?.twitch.channelLogin.trim() ?? "";
  const isBouyomiConnected = ["idle", "speaking", "paused"].includes(state.speechStatus);
  const messages: StartupGuideMessage[] = [
    message("startup-check", receivedAt, "Twitchと棒読みちゃんの状態を確認しています…"),
    isAuthenticated
      ? message("startup-auth", receivedAt, "Twitchとの連携が完了しています。")
      : message("startup-auth", receivedAt, "をクリックして、Twitchとの連携を完了しましょう。", "login"),
    channelLogin
      ? message("startup-channel", receivedAt, `読み上げチャンネルは「${channelLogin}」に設定されています。`)
      : message("startup-channel", receivedAt, "をクリックして、読み上げるTwitchチャンネルを設定しましょう。", "login"),
    isBouyomiConnected
      ? message("startup-bouyomi", receivedAt, "棒読みちゃんとの接続を確認しました。チャットを読み上げる準備ができています。")
      : message("startup-bouyomi", receivedAt, "棒読みちゃんの起動を確認できません。棒読みちゃんを起動すると、チャットの読み上げを始められます。"),
  ];

  if (isAuthenticated && channelLogin && isBouyomiConnected) {
    messages.push(message("startup-ready", receivedAt, "準備ができました。チャット受信を開始すると、Twitchのチャットがここに表示されます。"));
  }

  return messages;
}

function message(
  id: string,
  receivedAt: string,
  text: string,
  action?: StartupGuideAction,
): StartupGuideMessage {
  return { id, receivedAt, userDisplayName: "system", text, action };
}
