import type { SettingsInitialization } from "../stores/settingsStore";
import type { AppSettings, AppSettingsPatch, SettingsRecoveryNotice } from "../types";

export interface SettingsMutationDependencies {
  updateSettings: (patch: AppSettingsPatch) => Promise<AppSettings>;
  onSettingsLoaded: (settings: AppSettings) => void;
  onError: (error: unknown) => void;
}
export interface SettingsControllerDependencies extends SettingsMutationDependencies {
  loadSettings: () => Promise<AppSettings>;
  takeRecoveryNotice: () => Promise<SettingsRecoveryNotice | undefined>;
  initialGeneration: number;
  getSettingsRevision: () => number;
  onInitializationChanged: (state: SettingsInitialization) => void;
  onRecoveryNotice: (notice: SettingsRecoveryNotice) => void;
  loadErrorMessage: (error: unknown) => string;
}

/** Reads and serial writes share publication ownership, even across effect lifetimes. */
export function createSettingsController(dependencies: SettingsControllerDependencies) {
  let tail = Promise.resolve();
  let active = true;
  let lifetime = 0;
  let loadGeneration = dependencies.initialGeneration;
  let recoveryNoticeRequest: Promise<SettingsRecoveryNotice | undefined> | undefined;
  let recoveryNoticeReported = false;
  return {
    activate() {
      active = true;
      lifetime += 1;
    },
    invalidate() {
      active = false;
      lifetime += 1;
    },
    async load(): Promise<boolean> {
      if (!active) return false;
      const generation = ++loadGeneration;
      const owner = lifetime;
      const revision = dependencies.getSettingsRevision();
      const current = () =>
        active &&
        lifetime === owner &&
        loadGeneration === generation &&
        dependencies.getSettingsRevision() === revision;
      dependencies.onInitializationChanged({ status: "loading", generation });
      try {
        // StrictMode can invalidate the first load after the one-shot notice has
        // been taken. Share its result with the current load until it is reported.
        const noticeRequest = (recoveryNoticeRequest ??= dependencies
          .takeRecoveryNotice()
          .catch((error) => {
            recoveryNoticeRequest = undefined;
            throw error;
          }));
        const [settings, recoveryNotice] = await Promise.all([
          dependencies.loadSettings(),
          noticeRequest,
        ]);
        if (!current()) return false;
        dependencies.onSettingsLoaded(settings);
        dependencies.onInitializationChanged({ status: "ready", generation });
        if (recoveryNotice && !recoveryNoticeReported) {
          recoveryNoticeReported = true;
          dependencies.onRecoveryNotice(recoveryNotice);
        }
        return true;
      } catch (error) {
        if (!current()) return false;
        dependencies.onInitializationChanged({
          status: "error",
          generation,
          message: dependencies.loadErrorMessage(error),
        });
        dependencies.onError(error);
        return false;
      }
    },
    mutate(patch: AppSettingsPatch): Promise<boolean> {
      const owner = lifetime;
      const current = () => active && lifetime === owner;
      const operation = tail.then(async () => {
        if (!current()) return false;
        try {
          const settings = await dependencies.updateSettings(patch);
          if (!current()) return false;
          dependencies.onSettingsLoaded(settings);
          return true;
        } catch (error) {
          if (current()) dependencies.onError(error);
          return false;
        }
      });
      tail = operation.then(
        () => undefined,
        () => undefined,
      );
      return operation;
    },
    waitForIdle(): Promise<void> {
      return tail;
    },
  };
}
