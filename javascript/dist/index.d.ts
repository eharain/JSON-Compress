/**
 * json-compress - lossless JSON compression that is still JSON.
 */

/** Options accepted by {@link compress} and everything built on it. */
export interface CompressOptions {
  /** De-duplicate repeated string values into a catalogue. Default `true`. */
  strings?: boolean;
  /** Pack arrays of like-shaped objects into columnar tables. Default `true`. */
  tables?: boolean;
  /** Hoist columns whose value never changes out of the rows. Default `true`. */
  constants?: boolean;
  /** Encode low-cardinality string columns as integer indices. Default `true`. */
  dictColumns?: boolean;
  /** Smallest array that may become a table. Default `2`. */
  minRows?: number;
  /** Largest share of missing cells a table may carry, 0 to 1. Default `0.3`. */
  maxHoleRatio?: number;
  /** Times a string must occur before it can be catalogued. Default `2`. */
  minStringCount?: number;
  /** Shortest string that may be catalogued. Default `1`. */
  minStringLength?: number;
}

/** The compressed document. Plain JSON - store it, post it, log it. */
export interface CompressedEnvelope {
  /** Format version. */
  jc: number;
  /** Key catalogue: the original key names, most-used first. */
  k: string[];
  /** String-value catalogue. Absent when nothing was worth cataloguing. */
  s?: string[];
  /** The encoded document. */
  d: unknown;
}

/** A record set flattened by {@link pack}. */
export interface PackedTable<T = unknown> {
  fields: string[];
  data: T[][];
  count: number;
}

/** What {@link measure} reports. */
export interface SizeReport {
  /** Bytes of the input as given. */
  original: number;
  /** Bytes once whitespace is removed. */
  minified: number;
  /** Bytes of the compressed envelope. */
  compressed: number;
  /** Bytes saved against the minified form. */
  saved: number;
  /** Compressed divided by minified, 0 to 1. */
  ratio: number;
  /** Percentage saved against the minified form. */
  percent: number;
  /** Minified bytes after gzip, or null where the runtime cannot gzip. */
  gzipMinified: number | null;
  /** Compressed bytes after gzip, or null where the runtime cannot gzip. */
  gzipCompressed: number | null;
}

/** Where a JSON document stops being valid. */
export interface ValidationResult {
  valid: boolean;
  error?: string;
  line?: number;
  column?: number;
}

/** Options bound once by {@link createCodec}. */
export interface Codec {
  compress(value: unknown): CompressedEnvelope;
  decompress(envelope: unknown): unknown;
  stringify(value: unknown): string;
  parse(text: string): unknown;
  measure(value: unknown): Promise<SizeReport>;
  readonly options?: CompressOptions;
}

/** Compress a value into a json-compress envelope. */
export function compress(value: unknown, options?: CompressOptions): CompressedEnvelope;

/** Restore the original value from a json-compress envelope. */
export function decompress<T = unknown>(envelope: unknown): T;

/** Restore a value whether or not it was compressed. */
export function decompressIfNeeded<T = unknown>(value: unknown): T;

/** True when `value` looks like an envelope this build can read. */
export function isCompressed(value: unknown): value is CompressedEnvelope;

/** Compress a value and serialise it in one step. */
export function stringify(value: unknown, options?: CompressOptions): string;

/** Parse a compressed document and restore the original value. */
export function parse<T = unknown>(text: string): T;

/** Strip the whitespace from a JSON document without changing what it means. */
export function minify(text: string): string;

/** Re-indent a JSON document. */
export function beautify(text: string, indent?: number | string): string;

/** Check that a string is valid JSON, and say where it stops being valid. */
export function validate(text: string): ValidationResult;

/** Measure what compression buys on a given value. */
export function measure(
  value: unknown,
  options?: CompressOptions & { source?: string; gzip?: boolean }
): Promise<SizeReport>;

/** Bind a set of options once and reuse them. */
export function createCodec(options?: CompressOptions): Codec;

/** Pack an array of records into a field list and a rows matrix. */
export function pack<T extends Record<string, unknown>>(records: T[]): PackedTable;
export function pack(records: unknown): unknown;

/** Rebuild the array of records produced by {@link pack}. */
export function unpack<T = Record<string, unknown>>(packed: PackedTable | unknown): T[] | unknown;

/** Pack every result set in a list of result sets. */
export function packAll(resultSets: unknown): unknown;

/** Unpack every result set packed by {@link packAll}. */
export function unpackAll(resultSets: unknown): unknown;

/** True when `value` has the shape {@link unpack} knows how to read. */
export function isPacked(value: unknown): value is PackedTable;

/** Read a delimited-integer column back into an array of numbers. */
export function textToArray(
  record: Record<string, unknown>,
  property: string,
  shape?: { open?: number; close?: number; separator?: string }
): Record<string, unknown>;

/** Length of a string in UTF-8 bytes. */
export function byteLength(text: string): number;

/** True when the runtime can gzip without help. */
export function canGzip(): boolean;

/** Gzip a string. */
export function gzip(text: string, format?: 'gzip' | 'deflate' | 'deflate-raw'): Promise<Uint8Array>;

/** Reverse {@link gzip}. */
export function gunzip(
  bytes: Uint8Array,
  format?: 'gzip' | 'deflate' | 'deflate-raw'
): Promise<string>;

/** How many bytes a string takes once gzipped, or null if unavailable. */
export function gzipSize(text: string): Promise<number | null>;

/** The option defaults, frozen. */
export const defaultOptions: Required<CompressOptions>;

/** The wire-format version this build reads and writes. */
export const FORMAT_VERSION: number;

/** Thrown when an envelope cannot be read. */
export class DecodeError extends Error {
  name: 'DecodeError';
}

declare const jsonCompress: {
  compress: typeof compress;
  decompress: typeof decompress;
  decompressIfNeeded: typeof decompressIfNeeded;
  isCompressed: typeof isCompressed;
  stringify: typeof stringify;
  parse: typeof parse;
  minify: typeof minify;
  beautify: typeof beautify;
  validate: typeof validate;
  measure: typeof measure;
  createCodec: typeof createCodec;
  pack: typeof pack;
  unpack: typeof unpack;
  packAll: typeof packAll;
  unpackAll: typeof unpackAll;
  isPacked: typeof isPacked;
  textToArray: typeof textToArray;
};

export default jsonCompress;
