// Regenerates fixtures/aisdk/provider-utils/t3-unattributable-deltas.json by
// replaying each case's deltas through the pinned @ai-sdk/provider-utils
// StreamingToolCallTracker (typeValidation 'if-present', as openai chat).
// Usage: node scripts/gen-t3-tracker-fixture.mjs <node_modules dir with @ai-sdk/provider-utils@5.0.53>
import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
const nm = process.argv[2];
const req = createRequire(path.join(nm, 'x.js'));
const pkg = req('@ai-sdk/provider-utils/package.json');
if (pkg.version !== '5.0.53') throw new Error(`expected provider-utils 5.0.53, got ${pkg.version}`);
const { StreamingToolCallTracker } = req('@ai-sdk/provider-utils');
const out = new URL('../fixtures/aisdk/provider-utils/t3-unattributable-deltas.json', import.meta.url);
const fx = JSON.parse(fs.readFileSync(out, 'utf8'));
for (const c of fx.cases) {
  const steps = [];
  let parts = [];
  let n = 0;
  const t = new StreamingToolCallTracker({ enqueue: p => parts.push(p) }, {
    typeValidation: 'if-present', generateId: () => `gen-${n++}`,
  });
  let error = null;
  for (const d of c.deltas) {
    parts = [];
    try { t.processDelta(d); } catch (e) { error = { name: e.name, message: e.message }; }
    steps.push({ parts });
    if (error) break;
  }
  let flush = null;
  if (!error) { parts = []; t.flush(); flush = parts; }
  c.aisdk = { steps, error, flush };
}
fs.writeFileSync(out, JSON.stringify(fx, null, 2) + '\n');
