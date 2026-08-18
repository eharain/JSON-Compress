/**
 * Build the published bundles.
 *
 * The source is plain ESM with no dependencies, so the only real work here is
 * producing the two shapes the source cannot be consumed as directly: a
 * CommonJS file for `require`, and a single minified global for a script tag.
 */

import { mkdirSync, copyFileSync, statSync, rmSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import { build } from 'esbuild';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const dist = join(root, 'dist');
const entry = join(root, 'src', 'index.js');

const banner = {
  js: '/*! json-compress | MIT | https://github.com/eharain/json-compress */',
};

rmSync(dist, { recursive: true, force: true });
mkdirSync(dist, { recursive: true });

const targets = [
  {
    outfile: join(dist, 'index.mjs'),
    format: 'esm',
    minify: false,
    label: 'ESM',
  },
  {
    outfile: join(dist, 'index.cjs'),
    format: 'cjs',
    minify: false,
    label: 'CommonJS',
  },
  {
    outfile: join(dist, 'json-compress.min.js'),
    format: 'iife',
    globalName: 'jsonCompress',
    minify: true,
    label: 'browser global',
  },
];

for (const target of targets) {
  await build({
    entryPoints: [entry],
    outfile: target.outfile,
    bundle: true,
    format: target.format,
    globalName: target.globalName,
    minify: target.minify,
    sourcemap: false,
    target: ['es2020', 'node18'],
    platform: 'neutral',
    banner,
    legalComments: 'none',
  });
  const size = statSync(target.outfile).size;
  process.stdout.write(
    target.label.padEnd(16) + ' ' + String(size).padStart(7) + ' bytes  ' +
    target.outfile.slice(root.length + 1) + '\n'
  );
}

copyFileSync(join(root, 'src', 'index.d.ts'), join(dist, 'index.d.ts'));
process.stdout.write('types            ' + '        ' + 'dist/index.d.ts\n');
