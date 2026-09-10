#!/usr/bin/env node
import { readFileSync } from "node:fs";

// Client IDs are public identifiers; never print their value in diagnostics.
const clientId = process.env.RICE_TWITCH_CLIENT_ID ?? "";
if (!/^[A-Za-z0-9]+$/.test(clientId)) {
  console.error("エラー: RICE_TWITCH_CLIENT_ID を空白のない英数字で設定して再ビルドしてください。");
  process.exit(1);
}
if (process.argv.length > 3) process.exit(64);
if (process.argv[2]) {
  try {
    if (!readFileSync(process.argv[2]).includes(Buffer.from(clientId))) {
      throw new Error("missing");
    }
  } catch {
    console.error("エラー: 実行ファイルの Twitch Client ID 埋め込みを確認できません。公開を中止します。");
    process.exit(1);
  }
}
console.log("Twitch authentication build configuration: verified");
