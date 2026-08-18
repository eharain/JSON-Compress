#!/usr/bin/env node
/**
 * json-compress command line.
 *
 * Reads from a file or from stdin, writes to a file or to stdout, so it drops
 * into a pipeline without ceremony:
 *
 *     curl -s https://api.example.com/orders | json-compress compress > orders.jc.json
 *     json-compress stats orders.json
 */

import { readFileSync, writeFileSync } from 'node:fs';
import { basename } from 'node:path';
import {
  compress,
  decompress,
  isCompressed,
  minify,
  beautify,
  validate,
  measure,
} from '../src/index.js';

const USAGE = `
json-compress - lossless JSON compression that is still JSON

Usage
  json-compress <command> [input] [options]

Commands
  compress [file]     Compress a JSON document
  decompress [file]   Restore a compressed document
  stats [file]        Report what compression would save, and change nothing
  minify [file]       Remove whitespace only
  beautify [file]     Re-indent
  validate [file]     Check the document parses, and say where it does not

Options
  -o, --out <file>    Write to a file instead of stdout
  -p, --pretty        Indent the output
  -i, --indent <n>    Indent width for --pretty and beautify (default 2)
  --no-strings        Do not build a string-value catalogue
  --no-tables         Do not pack arrays of objects into tables
  --no-constants      Do not hoist unchanging columns out of rows
  --no-dict-columns   Do not encode low-cardinality string columns as indices
  --min-rows <n>      Smallest array that may become a table (default 2)
  -h, --help          Show this text
  -v, --version       Show the version

With no input file, the document is read from stdin.
`.trimStart();

function fail(message) {
  process.stderr.write('json-compress: ' + message + '\n');
  process.exit(1);
}

function readInput(file) {
  if (file) {
    try {
      return readFileSync(file, 'utf8');
    } catch (error) {
      fail('cannot read ' + file + ' (' + error.code + ')');
    }
  }
  try {
    return readFileSync(0, 'utf8');
  } catch {
    fail('no input file given and nothing on stdin');
  }
}

function writeOutput(text, file) {
  if (file) {
    writeFileSync(file, text, 'utf8');
    return;
  }
  process.stdout.write(text + '\n');
}

function parseArgs(argv) {
  const options = {
    command: null,
    input: null,
    out: null,
    pretty: false,
    indent: 2,
    compress: {},
  };

  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    switch (arg) {
      case '-h':
      case '--help':
        process.stdout.write(USAGE);
        process.exit(0);
        break;
      case '-v':
      case '--version': {
        const pkg = JSON.parse(
          readFileSync(new URL('../package.json', import.meta.url), 'utf8')
        );
        process.stdout.write(pkg.version + '\n');
        process.exit(0);
        break;
      }
      case '-o':
      case '--out':
        options.out = argv[++i];
        break;
      case '-p':
      case '--pretty':
        options.pretty = true;
        break;
      case '-i':
      case '--indent':
        options.indent = Number(argv[++i]);
        break;
      case '--no-strings':
        options.compress.strings = false;
        break;
      case '--no-tables':
        options.compress.tables = false;
        break;
      case '--no-constants':
        options.compress.constants = false;
        break;
      case '--no-dict-columns':
        options.compress.dictColumns = false;
        break;
      case '--min-rows':
        options.compress.minRows = Number(argv[++i]);
        break;
      default:
        if (arg.startsWith('-')) fail('unknown option ' + arg);
        else if (options.command === null) options.command = arg;
        else if (options.input === null) options.input = arg;
        else fail('unexpected argument ' + arg);
    }
  }

  return options;
}

function serialise(value, options) {
  return options.pretty
    ? JSON.stringify(value, null, options.indent)
    : JSON.stringify(value);
}

/** "31% smaller" or "31% larger" - never a negative percentage. */
function change(percent) {
  const rounded = Math.abs(percent).toFixed(1);
  return rounded + '% ' + (percent < 0 ? 'larger' : 'smaller');
}

function kb(bytes) {
  if (bytes < 1024) return bytes + ' B';
  if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB';
  return (bytes / (1024 * 1024)).toFixed(2) + ' MB';
}

async function main() {
  const options = parseArgs(process.argv.slice(2));
  if (!options.command) {
    process.stdout.write(USAGE);
    process.exit(0);
  }

  const text = readInput(options.input);

  if (options.command === 'validate') {
    const result = validate(text);
    if (result.valid) {
      process.stdout.write('valid JSON\n');
      return;
    }
    const where = result.line ? ' at line ' + result.line + ', column ' + result.column : '';
    fail('invalid JSON' + where + ': ' + result.error);
  }

  if (options.command === 'minify') {
    writeOutput(minify(text), options.out);
    return;
  }

  if (options.command === 'beautify') {
    writeOutput(beautify(text, options.indent), options.out);
    return;
  }

  let parsed;
  try {
    parsed = JSON.parse(text);
  } catch (error) {
    fail('input is not valid JSON: ' + error.message);
  }

  switch (options.command) {
    case 'compress': {
      writeOutput(serialise(compress(parsed, options.compress), options), options.out);
      return;
    }
    case 'decompress': {
      if (!isCompressed(parsed)) fail('input is not a compressed document');
      writeOutput(serialise(decompress(parsed), options), options.out);
      return;
    }
    case 'stats': {
      const report = await measure(parsed, { ...options.compress, source: text });
      const name = options.input ? basename(options.input) : 'stdin';
      const lines = [
        name,
        '  as given          ' + kb(report.original).padStart(10),
        '  minified          ' + kb(report.minified).padStart(10),
        '  compressed        ' + kb(report.compressed).padStart(10),
        '  saved             ' + kb(report.saved).padStart(10) +
          '   ' + change(report.percent) + ' than minified',
      ];
      if (report.gzipMinified !== null) {
        const gzipSaved = report.gzipMinified - report.gzipCompressed;
        const gzipPercent = report.gzipMinified === 0 ? 0 : (gzipSaved / report.gzipMinified) * 100;
        lines.push(
          '',
          '  minified + gzip   ' + kb(report.gzipMinified).padStart(10),
          '  compressed + gzip ' + kb(report.gzipCompressed).padStart(10),
          '  saved             ' + kb(gzipSaved).padStart(10) +
            '   ' + change(gzipPercent) + ' than minified + gzip'
        );
      }
      process.stdout.write(lines.join('\n') + '\n');
      return;
    }
    default:
      fail('unknown command "' + options.command + '". Try --help.');
  }
}

main().catch((error) => fail(error.message));
