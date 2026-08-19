/**
 * Check that the Rust and JavaScript implementations really do read each other.
 *
 * The conformance test inside the Rust suite proves this implementation reads
 * what JavaScript wrote, from a checked-in corpus. This proves the other
 * direction, which no fixture can: it runs the Rust command line over every
 * source document in the corpus and hands the result to the JavaScript decoder.
 *
 * Needs both toolchains, so it is a local and CI check rather than part of
 * either unit suite. Run from this directory:
 *
 *     node verify-interop.mjs [path-to-json-compress]
 *
 * With no argument it builds the command line from the crate in `rust/`, which
 * needs `cargo` on PATH.
 */

import { readFileSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { compress, decompress } from '../../../javascript/src/index.js';

const here = dirname(fileURLToPath(import.meta.url));
const crate = join(here, '..', '..');
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

/** The command line under test: the one given, or one built for the occasion. */
function commandLine() {
  if (process.argv[2]) return process.argv[2];

  execFileSync('cargo', ['build', '--release', '--bin', 'json-compress'], {
    cwd: crate,
    stdio: 'inherit',
  });
  return join(
    crate,
    'target',
    'release',
    process.platform === 'win32' ? 'json-compress.exe' : 'json-compress'
  );
}

const binary = commandLine();

/** Run the Rust command line over one document. */
function run(command, input) {
  return execFileSync(binary, [command], {
    input,
    encoding: 'utf8',
    maxBuffer: 64 * 1024 * 1024,
  }).trim();
}

const cases = JSON.parse(readFileSync(corpus, 'utf8')).cases;

let checked = 0;
const failures = [];

for (const item of cases) {
  // Rust compresses it, JavaScript restores it.
  const envelope = run('compress', item.source);

  const restored = JSON.stringify(decompress(JSON.parse(envelope)));
  if (restored !== item.source) {
    failures.push(
      item.name +
        ': JavaScript did not restore what Rust compressed' +
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
        '\n  rust       ' +
        envelope
    );
    continue;
  }

  // Rust restores what JavaScript compressed.
  const back = run('decompress', ours);

  if (back !== item.source) {
    failures.push(
      item.name +
        ': Rust did not restore what JavaScript compressed' +
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
