// Builds fixtures/aisdk/VERSIONS.json without losing pinned integrity (1.12).
//
// Each entry has the `{"version","integrity"}` shape that
// scripts/fetch-aisdk.sh verifies. The integrity is kept from the previous
// file when the version is unchanged. Otherwise it is taken from
// package-lock.json when the lock pins that same version. If neither source
// has it, the entry is written without integrity, and fetch-aisdk.sh falls
// back to `npm view`.

import { existsSync, readFileSync } from 'node:fs';

/** Read a JSON file, or return `fallback` when it does not exist. */
export function readJson(path, fallback) {
  return existsSync(path) ? JSON.parse(readFileSync(path, 'utf8')) : fallback;
}

/** `{pkg: {version, integrity}}` from a package-lock.json (lockfileVersion 2/3). */
export function lockIntegrity(lock) {
  const out = {};
  for (const [key, entry] of Object.entries(lock?.packages ?? {})) {
    if (!key.startsWith('node_modules/') || !entry?.integrity) continue;
    out[key.slice('node_modules/'.length)] = { version: entry.version, integrity: entry.integrity };
  }
  return out;
}

/**
 * The VERSIONS.json document for `installed` (`{pkg: version}`), keyed in
 * sorted order.
 *
 * @param {Record<string,string>} installed
 * @param {Record<string, string | {version: string, integrity?: string}>} previous
 * @param {Record<string, {version: string, integrity: string}>} locked
 */
export function versionsDocument(installed, previous = {}, locked = {}) {
  const out = {};
  for (const pkg of Object.keys(installed).sort()) {
    const version = installed[pkg];
    const prev = previous[pkg];
    let integrity;
    if (prev && typeof prev === 'object' && prev.version === version && prev.integrity) {
      integrity = prev.integrity;
    } else if (locked[pkg]?.version === version) {
      integrity = locked[pkg].integrity;
    }
    out[pkg] = integrity ? { version, integrity } : { version };
  }
  return out;
}
