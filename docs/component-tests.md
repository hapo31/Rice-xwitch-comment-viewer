# フロントエンドのcomponent test

`pnpm test`は`vitest.config.ts`の2 projectを実行する。既存のstore、protocol、純関数のテストはNode環境の`unit`、`*.dom.test.ts(x)`はjsdom環境の`dom`へ分離する。PR/main/releaseの共通frontend-test gateも同じコマンドを使う。

- `pnpm run test:unit`: Node unitのみ。
- `pnpm run test:dom`: DOM/componentのみ。
- `pnpm test`: 両方。未処理のPromise rejectionは失敗とし、無視しない。

## DOMで検証する内容

React Testing LibraryでAppShellを実際にmountし、memory data routerと独立したdomain storesを使う。Activity Barから全7画面へ移動し、表示、見出し、document title、選択中のリンクと遷移後のfocusを検証する。Settings/Filterはuser-eventの入力・Tab・保存・validation・未保存変更のキャンセル/破棄を操作し、エラーのフィールド関連付けと、変更がない保存ボタンの非表示を確認する。

Windows Launcherの追加と一斉起動の部分失敗、非Windowsの操作無効化、Queue snapshotの再描画、command rejectのLogs表示もDOMから検査する。StrictModeで遅延したnative subscriptionを解決し、1 eventが1 store更新と1 Chat行になること、部分的な購読失敗とunmount前の未解決購読でもlistenerが残らないことを確認する。

## 共通Tauri mock

`src/testing/tauriMock.ts`はcore invoke、event listen/unlisten、window、dialogを置き換える。実際のTCP、Twitch、keyring、filesystem、native windowへは接続しない。

- `setCommand(name, valueOrHandler)`で成功応答、非同期応答と引数検査を設定する。未知のcommandは失敗する。
- `rejectCommand(name, reason)`で失敗を注入する。
- `emit(name, payload)`で登録中のcallbackへnative形状のeventを送る。
- `delaySubscriptions()`、`releaseSubscriptions()`、`rejectNextSubscription(name)`で購読競合を検査する。
- `listenerCount(name?)`と`pendingCount`で残存を検査する。

各テスト終了時はReact cleanup後、遅延subscriptionを解決して1 tick待ち、listener/pendingが0であることを検査する。先にmockをresetして漏れを隠さない。mock自身のresetも残存listenerがあれば例外にする。mockの遅延・reject・漏れ検出も独立してテストする。

## 検証の境界

jsdom 27.4.0を固定し、Vitest 5に合わせてNodeの対応範囲はpackage.jsonのenginesに従う。開発コンテナ・CI・releaseではNode 22.22.0に揃える。ResizeObserverは仮想リストが描画可能な固定viewportを返す。これはブラウザ/WebViewのpixel、実レイアウト、スクリーンリーダーの発話、Windows shellやTauri ACLの実動作の証明ではない。それらはWindows smoke（#91）と実機確認で検証する。DOM mockへ実環境の秘密情報を入れない。
