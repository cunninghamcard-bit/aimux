// node --test scripts/aisdk-fixtures/versions.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { lockIntegrity, readJson, versionsDocument } from './versions.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const pinnedPath = join(here, '..', '..', 'fixtures', 'aisdk', 'VERSIONS.json');
const pinnedText = readFileSync(pinnedPath, 'utf8');
const pinned = JSON.parse(pinnedText);
const installed = Object.fromEntries(Object.entries(pinned).map(([p, e]) => [p, e.version]));

test('rewriting the pinned file preserves every integrity, byte for byte', () => {
  const doc = versionsDocument(installed, pinned, {});
  assert.deepEqual(doc, pinned);
  assert.equal(JSON.stringify(doc, null, 2) + '\n', pinnedText);
});

test('the lockfile supplies the same integrity when the previous file is flat', () => {
  const flat = Object.fromEntries(Object.entries(installed));
  const locked = lockIntegrity(readJson(join(here, 'package-lock.json'), {}));
  assert.deepEqual(versionsDocument(installed, flat, locked), pinned);
});

test('a version bump drops the stale pin unless the lock pins the new version', () => {
  const bumped = { ...installed, ai: '9.9.9' };
  assert.deepEqual(versionsDocument(bumped, pinned, {}).ai, { version: '9.9.9' });
  const locked = { ai: { version: '9.9.9', integrity: 'sha512-new' } };
  assert.deepEqual(versionsDocument(bumped, pinned, locked).ai, { version: '9.9.9', integrity: 'sha512-new' });
});
