# Vendored third-party assets

## pico.classless.min.css

Pico CSS v2.1.1 — MIT licensed, copyright 2019-2025.
Upstream: https://picocss.com

Vendored rather than loaded from a CDN for three reasons:

1. **Supply chain.** A CDN `<link>` lets a third party inject arbitrary CSS
   into every page. CSS is not inert — it can exfiltrate form contents via
   attribute selectors and background-image requests, and can reposition or
   hide UI. Subresource Integrity mitigates this, but only for a *pinned*
   version; the previous link floated on `@picocss/pico@2`, and SRI on a
   floating range breaks the moment upstream publishes a patch.
2. **Latency.** Cold fetch measured 22-80 ms from the CDN versus ~2.6 ms
   served locally, plus a third-party DNS lookup and TLS handshake.
3. **Offline / air-gapped use.** These tools are single static files and
   should work without internet access.

### Provenance

Downloaded from:

    https://cdn.jsdelivr.net/npm/@picocss/pico@2.1.1/css/pico.classless.min.css

    sha384-NZhm4G1I7BpEGdjDKnzEfy3d78xvy7ECKUwwnKTYi036z42IyF056PbHfpQLIYgL

Verify the vendored copy still matches upstream:

```bash
curl -sL https://cdn.jsdelivr.net/npm/@picocss/pico@2.1.1/css/pico.classless.min.css \
  | openssl dgst -sha384 -binary | openssl base64 -A
```

### Updating

Download the new exact version (never a floating `@2`), update the URL and
hash above, and re-check each consuming page renders correctly. Consumers:
`regexer/index.html`, `yield_max/index.html`. (`crosswind` has its own
`style.css` and does not use Pico.)
