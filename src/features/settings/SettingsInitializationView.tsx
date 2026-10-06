import { routeHeadingId } from "../../routeAccessibility";
import type { SettingsInitialization } from "../../stores/settingsStore";

export function SettingsInitializationView({
  title,
  initialization,
  onRetry,
}: {
  title: "Settings" | "Filter";
  initialization: Exclude<SettingsInitialization, { status: "ready" }>;
  onRetry: () => void;
}) {
  return (
    <main className="col-start-3 row-start-2 min-w-0 overflow-auto bg-zinc-950">
      <header className="flex h-12 items-center border-b border-zinc-800 bg-zinc-900 px-4">
        <h1 id={routeHeadingId} tabIndex={-1} className="text-sm font-semibold text-zinc-100">
          {title}
        </h1>
      </header>
      <div className="space-y-3 px-4 py-5 text-sm text-zinc-400">
        {initialization.status === "error" ? (
          <>
            <p role="alert" className="text-rose-400">
              {initialization.message}
            </p>
            <button
              type="button"
              onClick={onRetry}
              className="rounded border border-zinc-700 px-3 py-1.5 text-zinc-100"
            >
              設定を再読み込み
            </button>
          </>
        ) : (
          <p role="status">設定を読み込んでいます。読み込み完了後に編集できます。</p>
        )}
      </div>
    </main>
  );
}
