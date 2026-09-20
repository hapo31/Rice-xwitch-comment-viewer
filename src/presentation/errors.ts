export type ErrorOperation = "general" | "settings" | "auth" | "chat" | "speech" | "queue" | "launcher" | "externalUrl" | "exit";

const recovery: Record<ErrorOperation, string> = {
  general: "操作に失敗しました。Logs の詳細を確認し、もう一度操作してください。",
  settings: "設定を保存・読み込みできませんでした。入力内容と保存先の空き容量・権限を確認してください。",
  auth: "Twitch の認証を確認できませんでした。接続環境を確認し、Login から再認証してください。",
  chat: "Twitch チャットの接続操作に失敗しました。接続環境とチャンネル設定を確認して再接続してください。",
  speech: "読み上げの操作に失敗しました。棒読みちゃんを起動し、Settings の［診断］で接続を確認してください。",
  queue: "読み上げキューを更新できませんでした。Queue を再読み込みして状態を確認してください。",
  launcher: "ランチャーの操作に失敗しました。登録先のファイルとアクセス権限を確認してください。",
  externalUrl: "リンクを開けませんでした。既定のブラウザー設定を確認してもう一度開いてください。",
  exit: "終了処理に失敗しました。接続と読み上げの状態を確認してから再度終了してください。",
};

export interface PresentedError {
  message: string;
  details: string;
}

function errorDetails(error: unknown): string {
  const seen = new WeakSet<object>();
  try {
    const serialized = typeof error === "string" ? error : JSON.stringify(error, (_key, value: unknown) => {
      if (typeof value === "bigint") return value.toString();
      if (value && typeof value === "object") {
        if (seen.has(value)) return "[circular]";
        seen.add(value);
        if (value instanceof Error) return { ...value, name: value.name, message: value.message, stack: value.stack };
      }
      return value;
    });
    return (serialized || "エラー詳細が返されませんでした。").slice(0, 4000);
  } catch {
    return "エラー詳細を読み取れませんでした。";
  }
}

/** UI text stays actionable; the original rejection is retained separately for Logs. */
export function presentError(error: unknown, operation: ErrorOperation = "general"): PresentedError {
  const details = errorDetails(error);
  let message = "";
  let code = "";
  try {
    if (typeof error === "string") message = error;
    else if (error && typeof error === "object") {
      const object = error as { message?: unknown; code?: unknown; status?: unknown };
      if (typeof object.message === "string") message = object.message;
      if (typeof object.code === "string") code = object.code;
      if (object.status === 401) code = "unauthorized";
    }
  } catch { /* Unknown rejection objects can have throwing getters. */ }
  message = message.replace(/^(?:Error:\s*)+/, "").trim();
  // Preserve the backend's Japanese explanation, including partial-success warnings.
  if (/[ぁ-んァ-ヶ一-龯]/.test(message)) {
    const explanation = message.slice(0, 600);
    const actionable = /ください|再試行|再ログイン|再認証|お試し/.test(explanation);
    return { message: actionable ? explanation : `${explanation} ${recovery[operation]}`, details };
  }
  const identifier = `${code} ${message}`.toLowerCase();
  let cause = "";
  if (/unauthorized|invalid[_ ]token|token[_ ]expired|missingrequiredscope/.test(identifier)) cause = "Twitch の認証が無効か、必要な権限がありません。";
  else if (/econnrefused|connection refused/.test(identifier)) cause = "接続先が応答を受け付けていません。";
  else if (/etimedout|timeout|timed out/.test(identifier)) cause = "応答待ちがタイムアウトしました。";
  else if (/eacces|eperm|permission denied|access denied/.test(identifier)) cause = "アクセス権限がありません。";
  else if (/enoent|not found/.test(identifier)) cause = "対象が見つかりません。";
  return { message: `${cause}${recovery[operation]}`, details };
}

export function reportPresentedError(
  error: unknown,
  operation: ErrorOperation,
  sinks: { notify: (message: string) => void; log: (details: string) => void },
): PresentedError {
  const presented = presentError(error, operation);
  sinks.log(`[${operation}] ${presented.details}`);
  sinks.notify(presented.message);
  return presented;
}
