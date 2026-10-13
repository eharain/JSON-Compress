# Using it in a browser, with nothing installed

The built bundles are committed to this repository, so
[jsDelivr](https://www.jsdelivr.com/) serves them straight from GitHub over a
CDN. There is nothing to install, no build step, and no npm account involved —
paste a script tag into a page and `compress` is there.

## A script tag

```html
<script src="https://cdn.jsdelivr.net/gh/eharain/JSON-Compress@1.0.3/javascript/dist/json-compress.min.js"></script>
<script>
  const small = jsonCompress.compress(myData);
  const back  = jsonCompress.decompress(small);
</script>
```

The bundle defines one global, `jsonCompress`, carrying the whole API:
`compress`, `decompress`, `stringify`, `parse`, `measure`, `validate`,
`locate`, `minify`, `beautify`, `pack`, `unpack`, and the gzip helpers.

About 21 KB, 7 KB over the wire once the CDN gzips it.

## As an ES module

No global, no build tool, no bundler:

```html
<script type="module">
  import { compress, decompress, measure } from
    'https://cdn.jsdelivr.net/gh/eharain/JSON-Compress@1.0.3/javascript/dist/index.mjs';

  const small = compress(myData);
  console.log(await measure(myData));
</script>
```

The same URL works from a service worker, a Deno script, or an `import()` in the
browser console.

## Pinning a version

| URL | Resolves to |
|---|---|
| `…/JSON-Compress@1.0.3/…` | That release, forever. **Use this in production.** |
| `…/JSON-Compress@main/…` | The tip of `main`, cached by the CDN for up to 12 hours. |
| `…/JSON-Compress@1a2b3c4/…` | One exact commit. |

A tag is immutable, so a pinned URL cannot change under you. A branch URL can.

**Mind the `v`.** The git tags in this repository are `v1.0.0`, `v1.0.2` and so
on, but jsDelivr strips the prefix and serves them as versions — so the URL is
`@1.0.2`, not `@v1.0.2`. Asking for the `v` form returns a 404 whose message is
about the *file* being missing rather than the version, which sends you looking
in entirely the wrong place.

If a freshly pushed tag 404s for a minute or two, the CDN has not fetched that
file yet. Requesting
`https://purge.jsdelivr.net/gh/eharain/JSON-Compress@1.0.3/javascript/dist/json-compress.min.js`
prompts it, and the next request succeeds.

## Straight from npm instead

If you would rather the CDN pulled from the registry than from the repository,
the same files are on unpkg and on jsDelivr's npm endpoint:

```html
<script src="https://unpkg.com/@tech-style/json-compress/dist/json-compress.min.js"></script>
<script src="https://cdn.jsdelivr.net/npm/@tech-style/json-compress/dist/json-compress.min.js"></script>
```

## Subresource integrity

For a page that must not execute a bundle that has changed, jsDelivr will hand
you the hash: append `?meta` to any file URL, or use
[srihash.org](https://www.srihash.org/). Then:

```html
<script
  src="https://cdn.jsdelivr.net/gh/eharain/JSON-Compress@1.0.3/javascript/dist/json-compress.min.js"
  integrity="sha384-…"
  crossorigin="anonymous"></script>
```

An SRI hash only makes sense against a pinned tag or commit — against a branch
it will start failing the moment the branch moves.

## Everything runs locally

There are no network calls in the library. Nothing is uploaded, nothing is
logged, and no telemetry is collected. Once the bundle has loaded, the page
works with the network off — which is the point of doing it in the browser
rather than on somebody's server.

## A complete page

Copy this into a file and open it. No server needed.

```html
<!doctype html>
<meta charset="utf-8">
<title>json-compress</title>
<textarea id="in" rows="10" cols="70">
[{"id":1,"status":"active"},{"id":2,"status":"active"},{"id":3,"status":"idle"}]
</textarea>
<button id="go">Compress</button>
<pre id="out"></pre>

<script src="https://cdn.jsdelivr.net/gh/eharain/JSON-Compress@1.0.3/javascript/dist/json-compress.min.js"></script>
<script>
  document.getElementById('go').onclick = async () => {
    const source = JSON.parse(document.getElementById('in').value);
    const packed = jsonCompress.stringify(source);
    const report = await jsonCompress.measure(source);
    document.getElementById('out').textContent =
      packed + '\n\n' + report.percent.toFixed(1) + '% smaller than minified JSON';
  };
</script>
```
