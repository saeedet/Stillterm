// Static checks that protect the GitHub Pages project path and browser inputs.
import assert from 'node:assert/strict';
import {readFileSync, statSync} from 'node:fs';
import {fileURLToPath} from 'node:url';
import {spawnSync} from 'node:child_process';
import {parseWords} from '../website/rain.mjs';

const root = new URL('../website/', import.meta.url);
for (const name of ['app.js', 'rain.mjs']) {
  const result = spawnSync(process.execPath, ['--check', fileURLToPath(new URL(name, root))], {stdio: 'inherit'});
  assert.equal(result.status, 0, `${name} must parse`);
}
const html = readFileSync(new URL('index.html', root), 'utf8');
const base = new URL('https://saeedet.github.io/Stillterm/');
for (const [, value] of html.matchAll(/(?:src|href)="([^"]+)"/g)) {
  if (value.startsWith('#')) {
    if (value.length > 1) assert.ok(html.includes(`id="${value.slice(1)}"`), `Missing anchor ${value}`);
    continue;
  }
  if (/^https?:/.test(value)) continue;
  const url = new URL(value, base);
  assert.ok(url.pathname.startsWith('/Stillterm/'), `Asset escapes project URL: ${value}`);
  assert.ok(statSync(new URL(value, root)).isFile(), `Missing asset ${value}`);
}
assert.ok(html.includes('https://saeedet.github.io/Stillterm/'));
assert.deepEqual(parseWords(' FREE, YOUR\r\nMIND, ,FREE '), ['FREE', 'YOUR', 'MIND', 'FREE']);
assert.deepEqual(parseWords(' , \n'), []);
assert.throws(() => parseWords('a,'.repeat(17)));
assert.throws(() => parseWords('a'.repeat(65)));
assert.throws(() => parseWords('bad\u001b'));
console.log('PASS JavaScript, project-relative assets, anchors, phrase order, and input limits.');
