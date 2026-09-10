import { spawnSync } from 'node:child_process';
import { mkdtempSync, writeFileSync, rmSync, readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import assert from 'node:assert/strict';
const dir = mkdtempSync(join(tmpdir(), 'rice-client-id-'));
try {
  const binary = join(dir, 'rice.exe');
  const id = 'testPublicClient123';
  writeFileSync(binary, Buffer.concat([Buffer.from([0, 255]), Buffer.from(id), Buffer.from([0])]));
  for (const [value, args, success] of [
    ['', [], false], ['   ', [], false], ['bad id', [], false], ['id\n', [], false],
    [id, [], true], [id, [binary], true], ['differentClient', [binary], false],
    [id, [join(dir, 'missing.exe')], false],
  ]) {
    const result = spawnSync(process.execPath, ['scripts/verify-twitch-client-id.mjs', ...args], {
      env: { ...process.env, RICE_TWITCH_CLIENT_ID: value }, encoding: 'utf8',
    });
    assert.equal(result.status === 0, success);
    if (value.trim()) assert.ok(!(result.stdout + result.stderr).includes(value));
  }
  const workflow = readFileSync('.github/workflows/release-windows.yml', 'utf8');
  assert.ok(workflow.indexOf('run: node scripts/verify-twitch-client-id.mjs') < workflow.indexOf('name: Enable pinned pnpm'));
  assert.ok(!workflow.includes('::warning::RICE_TWITCH_CLIENT_ID'));
  const docker = readFileSync('Dockerfile', 'utf8');
  assert.ok(docker.indexOf('RUN node scripts/verify-twitch-client-id.mjs') < docker.indexOf('RUN pnpm install'));
  assert.ok(docker.includes('RUN node scripts/verify-twitch-client-id.mjs "src-tauri/target/${WINDOWS_TARGET}/release/rice.exe"'));
  assert.ok(readFileSync('scripts/build-windows-docker.sh', 'utf8').includes('node scripts/verify-twitch-client-id.mjs'));
  console.log('Twitch Client ID release gate tests passed');
} finally {
  rmSync(dir, { recursive: true, force: true });
}
