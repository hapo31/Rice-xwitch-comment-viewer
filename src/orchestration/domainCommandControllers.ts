import type { BouyomiConnectionDiagnostics, LauncherLaunchResult, SpeechStatus } from "../types";
import type { ErrorOperation } from "../presentation/errors";
import {
  launcherAdd,
  launcherLaunch,
  launcherLaunchAll,
  launcherRemove,
  speechConnectionDiagnostics,
  speechControl,
  speechHealthCheck,
  speechQueueDismiss,
  speechQueueDismissHistory,
  speechQueueReload,
  speechQueueRemove,
  speechQueueRetry,
  speechTest,
  isDesktopRuntime,
} from "../tauri/client";
import type { AppSettings } from "../types";

export interface ControllerFeedback {
  reportError: (error: unknown, operation?: ErrorOperation) => unknown;
  reportInfo: (message: string) => void;
}

export function createSpeechController(
  feedback: ControllerFeedback & {
    dispatchSpeechStatus: (status: SpeechStatus) => void;
  },
) {
  async function test(text?: string) {
    try {
      if (!isDesktopRuntime()) feedback.dispatchSpeechStatus("speaking");
      await speechTest(typeof text === "string" ? text : "テスト読み上げです。");
      if (!isDesktopRuntime()) feedback.dispatchSpeechStatus("idle");
      feedback.reportInfo("テスト読み上げを送信しました。");
    } catch (error) {
      if (!isDesktopRuntime()) feedback.dispatchSpeechStatus("error");
      feedback.reportError(error, "speech");
    }
  }

  async function healthCheck() {
    try {
      const message = await speechHealthCheck();
      if (!isDesktopRuntime()) feedback.dispatchSpeechStatus("idle");
      feedback.reportInfo(message);
    } catch (error) {
      if (!isDesktopRuntime()) feedback.dispatchSpeechStatus("disconnected");
      feedback.reportError(error, "speech");
    }
  }

  async function diagnostics(): Promise<BouyomiConnectionDiagnostics> {
    try {
      const result = await speechConnectionDiagnostics();
      feedback.reportInfo(result.recommendation);
      return result;
    } catch (error) {
      feedback.reportError(error, "speech");
      throw error;
    }
  }

  async function control(command: "pause" | "resume" | "skip" | "clear") {
    if (command === "clear" && !window.confirm("待機中の読み上げをクリアしますか？")) return;
    try {
      await speechControl(command);
      if (!isDesktopRuntime())
        feedback.dispatchSpeechStatus(command === "pause" ? "paused" : "idle");
    } catch (error) {
      if (!isDesktopRuntime()) feedback.dispatchSpeechStatus("error");
      feedback.reportError(error, "speech");
    }
  }

  return { test, healthCheck, diagnostics, control };
}

export function createQueueController(
  feedback: ControllerFeedback & {
    dispatchQueueSnapshot: (snapshot: Awaited<ReturnType<typeof speechQueueReload>>) => void;
  },
) {
  return {
    async reload() {
      try {
        const snapshot = await speechQueueReload();
        if (snapshot) feedback.dispatchQueueSnapshot(snapshot);
      } catch (error) {
        feedback.reportError(error, "queue");
      }
    },
    async remove(itemId: string) {
      try {
        await speechQueueRemove(itemId);
      } catch (error) {
        feedback.reportError(error, "queue");
      }
    },
    async dismiss(itemId: string) {
      try {
        await speechQueueDismiss(itemId);
      } catch (error) {
        feedback.reportError(error, "queue");
      }
    },
    async dismissHistory() {
      if (!window.confirm("表示中の読み上げ履歴をクリアしますか？")) return;
      try {
        await speechQueueDismissHistory();
      } catch (error) {
        feedback.reportError(error, "queue");
      }
    },
    async retry(itemId: string) {
      try {
        await speechQueueRetry(itemId);
      } catch (error) {
        feedback.reportError(error, "queue");
      }
    },
  };
}

export function createLauncherController(
  feedback: ControllerFeedback & {
    dispatchLauncherItems: (items: AppSettings["launcher"]["items"]) => void;
  },
) {
  const reportResult = (result: LauncherLaunchResult) => {
    const firstFailure = result.failures[0];
    if (firstFailure) {
      feedback.reportError(
        new Error(`${firstFailure.displayName} を起動できませんでした: ${firstFailure.message}`),
        "launcher",
      );
    }
    return result;
  };
  return {
    async add(paths: string[]) {
      try {
        const result = await launcherAdd(paths);
        feedback.dispatchLauncherItems(result.items);
        return result;
      } catch (error) {
        feedback.reportError(error, "launcher");
        throw error;
      }
    },
    async remove(itemId: string) {
      try {
        const items = await launcherRemove(itemId);
        feedback.dispatchLauncherItems(items);
        return items;
      } catch (error) {
        feedback.reportError(error, "launcher");
        throw error;
      }
    },
    async launch(itemId: string) {
      try {
        return reportResult(await launcherLaunch(itemId));
      } catch (error) {
        feedback.reportError(error, "launcher");
        throw error;
      }
    },
    async launchAll() {
      try {
        return reportResult(await launcherLaunchAll());
      } catch (error) {
        feedback.reportError(error, "launcher");
        throw error;
      }
    },
  };
}
