/**
 * Check that the Python and JavaScript implementations really do read each
 * other.
 *
 * The conformance test inside the Python suite proves this implementation reads
 * what JavaScript wrote, from a checked-in corpus. This proves the other
 * direction, which no fixture can: it runs the Python command line over every
 * source document in the corpus and hands the result to the JavaScript decoder.
 *
 * Needs both toolchains, so it is a local and CI check rather than part of
 * either unit suite. Run from this directory:
 *
 *     node verify-interop.mjs [path-to-python]
 *
 * With no argument it uses `python` from PATH. The Python package must be
 * importable - `pip install -e ..` or PYTHONPATH pointing at `python/src`.
 */

import { readFileSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { compress, decompress } from '../../../javascript/src/index.js';

const here = dirname(fileURLToPath(import.meta.url));
const corpus = join(
  here,
  '..',
  '..',
  '..',
  'dotnet',
  'tests',
  'TechStyle.JsonCompress.Tests',
  'fixtures',
  'conformance.json'
);

const python = process.argv[2] || 'python';
const source = join(here, '..', '..', 'src');

/** Run the Python command line over one document. */
function run(command, input) {
  return execFileSync(python, ['-m', 'json_compress', command], {
    input,
    encoding: 'utf8',
    maxBuffer: 64 * 1024 * 1024,
    env: { ...process.env, PYTHONPATH: source, PYTHONIOENCODING: 'utf-8' },
  }).trim();
}

const cases = JSON.parse(readFileSync(corpus, 'utf8')).cases;

let checked = 0;
const failures = [];

for (const item of cases) {
  // Python compresses it, JavaScript restores it.
  const envelope = run('compress', item.source);

  const restored = JSON.stringify(decompress(JSON.parse(envelope)));
  if (restored !== item.source) {
    failures.push(
      item.name +
        ': JavaScript did not restore what Python compressed' +
        '\n  expected ' +
        item.source +
        '\n  actual   ' +
        restored
    );
    continue;
  }

  // And the same envelope, which is a stronger claim than the format makes but
  // one every released implementation currently meets.
  const ours = JSON.stringify(compress(JSON.parse(item.source)));
  if (ours !== envelope) {
    failures.push(
      item.name +
        ': the two encoders wrote different envelopes' +
        '\n  javascript ' +
        ours +
        '\n  python     ' +
        envelope
    );
    continue;
  }

  // Python restores what JavaScript compressed.
  const back = run('decompress', ours);

  if (back !== item.source) {
    failures.push(
      item.name +
        ': Python did not restore what JavaScript compressed' +
        '\n  expected ' +
        item.source +
        '\n  actual   ' +
        back
    );
    continue;
  }

  checked++;
}

if (failures.length > 0) {
  process.stderr.write(failures.join('\n\n') + '\n\n');
  process.stderr.write(failures.length + ' of ' + cases.length + ' cases disagreed\n');
  process.exit(1);
}

process.stdout.write(
  checked + ' documents survive a round trip through both implementations, in both directions\n'
);
