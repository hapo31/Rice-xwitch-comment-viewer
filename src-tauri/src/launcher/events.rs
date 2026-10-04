use super::model::{LauncherItem, LauncherLaunchResult};
use super::ports::{IconExtractionWarning, LauncherEventSink};
use crate::app_events::{emit_app_log, AppLogLevel};

pub(super) struct AppEventSink<'a>(pub &'a tauri::AppHandle<tauri::Wry>);
impl LauncherEventSink for AppEventSink<'_> {
    fn added(&self, count: usize) {
        emit_app_log(
            self.0,
            AppLogLevel::Info,
            format!("ランチャーにアプリを {count} 件追加しました。"),
        );
    }
    fn removed(&self, item: &LauncherItem) {
        emit_app_log(
            self.0,
            AppLogLevel::Info,
            format!("ランチャーから「{}」を削除しました。", item.display_name),
        );
    }
    fn launched(&self, result: &LauncherLaunchResult) {
        log_launch_result(self.0, result);
    }
    fn icon_warnings(&self, warnings: &[IconExtractionWarning]) {
        emit_icon_extraction_warnings(self.0, warnings);
    }
}

fn emit_icon_extraction_warnings(
    app: &tauri::AppHandle<tauri::Wry>,
    warnings: &[IconExtractionWarning],
) {
    const MAX_LOGGED_WARNINGS: usize = 3;

    for warning in warnings.iter().take(MAX_LOGGED_WARNINGS) {
        emit_app_log(
            app,
            AppLogLevel::Warning,
            format!(
                "ランチャーのアイコンを取得できなかったため汎用アイコンを使います: {}（{}、{} ms）",
                warning.target.display(),
                warning.message,
                warning.elapsed.as_millis()
            ),
        );
    }
    if warnings.len() > MAX_LOGGED_WARNINGS {
        emit_app_log(
            app,
            AppLogLevel::Warning,
            format!(
                "ランチャーのアイコン取得失敗がさらに {} 件あります。ログは最大 {MAX_LOGGED_WARNINGS} 件まで表示します。",
                warnings.len() - MAX_LOGGED_WARNINGS
            ),
        );
    }
}

fn log_launch_result(app: &tauri::AppHandle<tauri::Wry>, result: &LauncherLaunchResult) {
    if result.failures.is_empty() {
        emit_app_log(
            app,
            AppLogLevel::Info,
            format!(
                "ランチャーからアプリを {} 件起動しました。",
                result.launched_count
            ),
        );
    } else {
        emit_app_log(
            app,
            AppLogLevel::Warning,
            format!(
                "ランチャーから {} 件起動し、{} 件は起動できませんでした。",
                result.launched_count,
                result.failures.len()
            ),
        );
    }
}
