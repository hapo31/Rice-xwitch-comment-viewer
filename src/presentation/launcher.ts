import type { LauncherItem, LauncherLaunchResult } from "../types";

const tileColors = ["#075985", "#155e75", "#166534", "#5b21b6", "#9f1239", "#92400e"] as const;
const supportedApplicationExtensions = [".exe", ".lnk"] as const;

export function sortLauncherItems(items: LauncherItem[]): LauncherItem[] {
  return [...items].sort((left, right) =>
    left.order - right.order || left.displayName.localeCompare(right.displayName, "ja"),
  );
}

export function launcherTileColor(item: LauncherItem): string {
  if (item.backgroundColor && /^#[0-9a-f]{6}$/i.test(item.backgroundColor)) {
    return item.backgroundColor;
  }

  let hash = 0;
  for (const character of item.id) {
    hash = (hash * 31 + character.charCodeAt(0)) >>> 0;
  }
  return tileColors[hash % tileColors.length];
}

export function partitionApplicationPaths(paths: string[]): {
  accepted: string[];
  rejected: string[];
} {
  const accepted: string[] = [];
  const rejected: string[] = [];

  for (const path of paths) {
    const normalized = path.toLocaleLowerCase();
    if (supportedApplicationExtensions.some((extension) => normalized.endsWith(extension))) {
      accepted.push(path);
    } else {
      rejected.push(path);
    }
  }

  return { accepted, rejected };
}

export function launcherLaunchSummary(result: LauncherLaunchResult): string {
  if (result.launchedCount === 0 && result.failures.length === 0) {
    return "起動するアプリがありません。";
  }
  if (result.failures.length === 0) {
    return `${result.launchedCount} 件のアプリを起動しました。`;
  }
  const failedNames = result.failures.slice(0, 2).map(({ displayName }) => displayName).join("、");
  const remainingCount = result.failures.length - 2;
  const failureDetail = remainingCount > 0 ? `${failedNames} ほか ${remainingCount} 件` : failedNames;
  if (result.launchedCount === 0) {
    return `${result.failures.length} 件のアプリを起動できませんでした（${failureDetail}）。`;
  }
  return `${result.launchedCount} 件を起動し、${result.failures.length} 件は起動できませんでした（${failureDetail}）。`;
}
