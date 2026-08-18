/**
 * What the format is worth, on shapes that occur in real systems.
 *
 *     node examples/benchmark.js
 *
 * Every figure below is produced by running the code in this file, so it can be
 * checked rather than taken on trust. The interesting column is the last one:
 * whether the format still helps once gzip has had its turn, which is the only
 * question that matters for anything served over HTTP.
 */

import { compress, decompress, byteLength, gzipSize } from '../src/index.js';

/* A deterministic generator, so the numbers are the same on every machine. */
let seed = 20260818;
function random() {
  seed = (seed * 1664525 + 1013904223) >>> 0;
  return seed / 4294967296;
}
function pick(list) {
  return list[Math.floor(random() * list.length)];
}
function integer(from, to) {
  return from + Math.floor(random() * (to - from + 1));
}

const FIRST = ['Amara', 'Bilal', 'Chen', 'Dara', 'Eesa', 'Farah', 'Gita', 'Hana', 'Idris', 'Jae'];
const LAST = ['Ahmed', 'Bright', 'Chowdhury', 'Doyle', 'Evans', 'Fischer', 'Gomez', 'Haddad'];
const CITY = ['London', 'Manchester', 'Birmingham', 'Leeds', 'Glasgow', 'Cardiff', 'Belfast'];

const scenarios = {
  'API record set (1,000 users)': () =>
    Array.from({ length: 1000 }, (unused, i) => ({
      id: i + 1,
      firstName: pick(FIRST),
      lastName: pick(LAST),
      email: 'user' + (i + 1) + '@example.com',
      role: pick(['admin', 'editor', 'viewer', 'viewer', 'viewer']),
      active: random() > 0.2,
      city: pick(CITY),
      country: 'United Kingdom',
      createdAt: '2026-0' + integer(1, 9) + '-' + String(integer(10, 28)) + 'T09:30:00.000Z',
      loginCount: integer(0, 4000),
    })),

  'Nested API response (orders with lines)': () => ({
    meta: { page: 1, pageSize: 200, total: 8214, generatedAt: '2026-08-18T12:00:00.000Z' },
    data: Array.from({ length: 200 }, (unused, i) => ({
      orderId: 'ORD-' + (100000 + i),
      status: pick(['placed', 'picked', 'shipped', 'delivered']),
      currency: 'GBP',
      placedAt: '2026-08-' + String(integer(10, 18)) + 'T' + String(integer(10, 20)) + ':15:00.000Z',
      customer: { id: integer(1, 5000), name: pick(FIRST) + ' ' + pick(LAST), city: pick(CITY) },
      lines: Array.from({ length: integer(1, 6) }, () => ({
        sku: 'SKU-' + integer(1000, 9999),
        description: pick(['Cotton shirt', 'Wool scarf', 'Leather belt', 'Linen trousers']),
        quantity: integer(1, 4),
        unitPrice: integer(500, 12000) / 100,
        taxRate: 0.2,
      })),
    })),
  }),

  'Time series (2,000 readings)': () =>
    Array.from({ length: 2000 }, (unused, i) => ({
      t: 1755500000 + i * 60,
      sensor: pick(['temp-01', 'temp-02', 'temp-03', 'humidity-01']),
      unit: 'celsius',
      value: Math.round(random() * 3500) / 100,
      quality: pick(['good', 'good', 'good', 'suspect']),
    })),

  'Structured logs (1,500 lines)': () =>
    Array.from({ length: 1500 }, () => ({
      level: pick(['info', 'info', 'info', 'warn', 'error']),
      service: pick(['api-gateway', 'orders', 'billing', 'search']),
      env: 'production',
      region: 'eu-west-2',
      message: pick([
        'request completed',
        'cache miss',
        'upstream timeout, retrying',
        'validation failed',
      ]),
      durationMs: integer(1, 4000),
      traceId: '00-' + integer(1e8, 9e8) + '-' + integer(1e6, 9e6) + '-01',
    })),

  'GeoJSON feature collection (400 points)': () => ({
    type: 'FeatureCollection',
    features: Array.from({ length: 400 }, (unused, i) => ({
      type: 'Feature',
      geometry: {
        type: 'Point',
        coordinates: [
          Math.round((random() * 8 - 6) * 1e6) / 1e6,
          Math.round((49 + random() * 9) * 1e6) / 1e6,
        ],
      },
      properties: { id: i, name: pick(CITY) + ' site ' + i, category: pick(['depot', 'store', 'hub']) },
    })),
  }),

  'Config document (deeply nested, few repeats)': () => ({
    version: 3,
    services: Object.fromEntries(
      Array.from({ length: 12 }, (unused, i) => [
        'service-' + i,
        {
          image: 'registry.example.com/team/service-' + i + ':1.4.' + i,
          replicas: integer(1, 6),
          resources: { cpu: '500m', memory: '512Mi' },
          env: { NODE_ENV: 'production', LOG_LEVEL: 'info', REGION: 'eu-west-2' },
          ports: [{ container: 8080, published: 8080 + i, protocol: 'tcp' }],
        },
      ])
    ),
  }),
};

function pad(text, width) {
  return String(text).padStart(width);
}

function kb(bytes) {
  return (bytes / 1024).toFixed(1);
}

const header =
  'scenario'.padEnd(46) +
  pad('minified', 10) +
  pad('compressed', 12) +
  pad('saved', 8) +
  pad('+gzip', 10) +
  pad('saved', 8);

process.stdout.write('\n' + header + '\n' + '-'.repeat(header.length) + '\n');

for (const [name, build] of Object.entries(scenarios)) {
  seed = 20260818;
  const value = build();

  const minified = JSON.stringify(value);
  const packed = JSON.stringify(compress(value));

  // A benchmark that quietly loses data is not a benchmark.
  if (JSON.stringify(decompress(JSON.parse(packed))) !== minified) {
    throw new Error(name + ' did not round-trip');
  }

  const minifiedBytes = byteLength(minified);
  const packedBytes = byteLength(packed);
  const minifiedGzip = await gzipSize(minified);
  const packedGzip = await gzipSize(packed);

  const saved = ((1 - packedBytes / minifiedBytes) * 100).toFixed(0) + '%';
  const savedGzip =
    minifiedGzip === null ? '-' : ((1 - packedGzip / minifiedGzip) * 100).toFixed(0) + '%';

  process.stdout.write(
    name.padEnd(46) +
      pad(kb(minifiedBytes) + ' KB', 10) +
      pad(kb(packedBytes) + ' KB', 12) +
      pad(saved, 8) +
      pad(minifiedGzip === null ? '-' : kb(packedGzip) + ' KB', 10) +
      pad(savedGzip, 8) +
      '\n'
  );
}

process.stdout.write(
  '\n"saved" is against minified JSON; the gzip columns compare compressed + gzip\n' +
    'against minified + gzip, which is what an HTTP response actually costs.\n\n'
);
