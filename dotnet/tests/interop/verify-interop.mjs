/**
 * Check that the two implementations really do read each other.
 *
 * The corpus test inside the .NET suite proves this implementation reads what
 * JavaScript wrote. This proves the other direction, which no checked-in
 * fixture can: it runs the .NET command line over every source document in the
 * corpus and hands the result to the JavaScript decoder.
 *
 * Needs both toolchains, so it is a local and CI check rather than part of
 * either unit suite. Run from this directory:
 *
 *     node verify-interop.mjs [path-to-json-compress-executable]
 *
 * With no argument it looks for the Release build of the CLI.
 */

import { readFileSync, existsSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { compress, decompress } from '../../../javascript/src/index.js';

const here = dirname(fileURLToPath(import.meta.url));
const corpus = join(here, '..', 'TechStyle.JsonCompress.Tests', 'fixtures', 'conformance.json');

function findCli() {
  if (process.argv[2]) return process.argv[2];

  const cli = join(here, '..', '..', 'cli', 'TechStyle.JsonCompress.Cli', 'bin');
  const names = process.platform === 'win32' ? ['json-compress.exe'] : ['json-compress'];
  for (const configuration of ['Release', 'Debug']) {
    for (const framework of ['net10.0', 'net8.0']) {
      for (const name of names) {
        const path = join(cli, configuration, framework, name);
        if (existsSync(path)) return path;
      }
    }
  }

  throw new Error(
    'cannot find the json-compress executable - build it with\n' +
      '  dotnet build dotnet/cli/TechStyle.JsonCompress.Cli -c Release\n' +
      'or pass its path as an argument'
  );
}

const cli = findCli();
const cases = JSON.parse(readFileSync(corpus, 'utf8')).cases;

let checked = 0;
const failures = [];

for (const item of cases) {
  // .NET compresses it, JavaScript restores it.
  const envelope = execFileSync(cli, ['compress'], {
    input: item.source,
    encoding: 'utf8',
    maxBuffer: 64 * 1024 * 1024,
  }).trim();

  const restored = JSON.stringify(decompress(JSON.parse(envelope)));
  if (restored !== item.source) {
    failures.push(
      item.name + ': JavaScript did not restore what .NET compressed' +
        '\n  expected ' + item.source + '\n  actual   ' + restored
    );
    continue;
  }

  // And the same envelope, which is a stronger claim than the format makes but
  // one both implementations currently meet.
  const ours = JSON.stringify(compress(JSON.parse(item.source)));
  if (ours !== envelope) {
    failures.push(
      item.name + ': the two encoders wrote different envelopes' +
        '\n  javascript ' + ours + '\n  dotnet     ' + envelope
    );
    continue;
  }

  // .NET restores what JavaScript compressed.
  const back = execFileSync(cli, ['decompress'], {
    input: ours,
    encoding: 'utf8',
    maxBuffer: 64 * 1024 * 1024,
  }).trim();

  if (back !== item.source) {
    failures.push(
      item.name + ': .NET did not restore what JavaScript compressed' +
        '\n  expected ' + item.source + '\n  actual   ' + back
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
