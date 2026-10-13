# Quint 0.33.0 `verify` overflows Apalache 0.62.1's JSON string cap

This document packages an upstream-filing investigation: a reproducible defect
in the Quint -> Apalache `verify` transport, a candidate-fix investigation with
measurements, and a less-developed secondary observation. It is self-contained:
a reader who has never seen this repository can file the issue and begin the fix
from this file alone. No proprietary model content is reproduced; only
structural excerpts of the serialized intermediate representation (IR).

## Summary

`quint verify` serializes the whole compiling-stage IR as one JSON string and
submits it in the gRPC `CHECK` request's `config` field. For any non-trivial
spec the per-node `types` and `table` maps dominate that string. Apalache
0.62.1 embeds Jackson >= 2.15, whose `StreamReadConstraints.getMaxStringLength()`
defaults to 20,000,000 characters; the reader aborts on the oversized `config`
string before model checking begins. The surfaced error reports the abort
offset, not the real size, so it reads as a fixed `20051112` across every spec.
A trivial one-variable spec verifies green (config ~9 KB), so the quint/Apalache
pairing itself works — the wall is IR size. Quint 0.32.0 with Apalache 0.56.1
checked the same specs because that older Apalache shipped a pre-2.15 Jackson
with no string cap.

---

## 1. Ready-to-file issue for quint-co/quint

````markdown
Title: `verify` submits the whole IR as one JSON string and overflows Apalache 0.62.1's 20 MB Jackson string cap

### What happens

`quint verify` sends the compiling-stage IR to Apalache as the `source.content`
field of the gRPC `CHECK` request, serialized as a single JSON string inside the
request's `config` field. For non-trivial specs this string exceeds 20 MB, and
Apalache 0.62.1 (Jackson >= 2.15) rejects it before checking with:

```
error: <RPC configuration>: Invalid strict JSON: String value length (20051112) exceeds the maximum allowed (20000000, from `StreamReadConstraints.getMaxStringLength()`)
```

The `20051112` is Jackson's abort offset (≈ the 20,000,000 cap plus one read
buffer), not the payload size, so it looks identical for every spec that trips
it. The checker never starts — the Apalache run directory's `detailed.log`
records only server start, no checker pass.

### Why it is the IR size, not a broken pairing

A trivial spec verifies green:

```
module hello {
  var n: int
  action init = { n' = 0 }
  action step = { n' = n + 1 }
  val inv = n >= 0
}
```

`quint verify hello.qnt --main=hello --invariant=inv --max-steps=1` succeeds;
its `config` string is ~9.3 KB (`source.content` ~7.5 KB). The size scales with
the model: any spec whose serialized compiling-stage IR approaches ~20 MB trips
the cap.

### Where the size comes from

The dominant contributors are the per-node `types` and name-resolution `table`
maps that quint includes in `source.content`, not the flattened `modules`. A
measured mid-size production spec (a ~600-line module plus a shared import tree;
one of a suite of hardware-lifecycle models):

| `source.content` section | size      | entries |
|--------------------------|-----------|---------|
| `modules` (flattened)    | ~0.67 MB  | 1 module |
| `table`                  | ~41.95 MB | 7,282   |
| `types`                  | ~62.30 MB | 19,136  |
| `effects`                | ~3.75 MB  | 19,123  |
| total `source.content`   | ~96.78 MB |         |
| total `config` string    | ~117.47 MB |        |

The `types` and `table` entries are highly repetitive — most are identical
copies of a few large record/sum types and resolved names (see "redundancy"
below). `source.content` structure (key names only):

```json
{ "stage": "compiling", "warnings": [], "modules": [ ... ],
  "table": { "<nodeId>": { ... } }, "types": { "<nodeId>": { ... } },
  "effects": { "<nodeId>": { ... } }, "errors": [], "main": "<name>" }
```

An example `types` entry (the most repeated one, appearing thousands of times):

```json
{ "rowVariables": [], "type": { "kind": "str" }, "typeVariables": [] }
```

An example `table` entry:

```json
{ "depth": 0, "id": 2492, "kind": "var", "name": "world", "typeAnnotation": { ... } }
```

### Client submit site

`dist/src/apalache.js`, the `check`/`tla` closures:

```js
const check = async (c) => {
  return cmdExecutor.run({ cmd: 'CHECK', config: JSON.stringify(c) })
    .then(handleResponse, handleRpcError);
};
```

`c` is built by `createConfig`:

```js
return {
  ...loadedConfig,
  source: { kind: 'string', format: 'qnt', content: parsedSpec },
  checker: { ... },
};
```

`parsedSpec` is the ~96 MB IR JSON string; `JSON.stringify(c)` becomes the
~117 MB `config` string that Apalache rejects.

### Regression

Quint 0.32.0 with Apalache 0.56.1 verified the same specs. Apalache 0.56.1
shipped a pre-2.15 Jackson with no `maxStringLength` cap; the 20,000,000 default
arrived with Jackson 2.15, bundled in current Apalache (0.62.x). So the break is
the combination of (a) quint embedding the full `types`/`table` IR inline and
(b) the newer Apalache's Jackson cap.

### Repro without the reporter's models

Any spec whose serialized compiling-stage IR approaches ~20 MB reproduces it.
Scaling evidence: trivial 1-variable spec -> ~9 KB config, green; a mid-size
production spec -> ~117 MB config, rejected. To reproduce directly, instrument
`dist/src/apalache.js`'s `check` closure to `fs.writeFileSync` the length of
`JSON.stringify(c)` before `cmdExecutor.run`, and verify progressively larger
specs until the `config` string passes 20 MB.

The reporter's spec suite is public (https://github.com/cunabli/dpaa2-controlplane,
`models/`) should a maintainer want a real corpus that trips this; the report
above is self-contained without it.

### Environment

- quint 0.33.0
- Apalache 0.62.1 (build a321771), default for quint 0.33.0
- JDK 25 (host); the cap also applies under JDK 21 (Apalache's supported floor)
- Linux (Fedora 44, kernel 7.2.x)
- pnpm 12.10.x
````

---

## 2. Candidate fix avenues (with measured evidence)

### 2a. Strip test — are `types`/`table` droppable?

Method: instrument the `check` closure to `JSON.parse(c.source.content)`, delete
candidate top-level keys, re-`stringify`, then submit; record resulting `config`
size and Apalache's response. (Same copy-on-write discipline on the private
`node_modules` copy, restored afterward.)

| Stripped keys (dpio spec)                 | `config` size | Apalache result |
|-------------------------------------------|---------------|-----------------|
| `warnings,errors,effects`                 | ~113.49 MB    | Jackson cap abort (still > 20 MB) |
| `types`                                   | ~50.04 MB     | Jackson cap abort |
| `table`                                   | ~72.14 MB     | Jackson cap abort |
| `types,table`                             | ~4.71 MB      | **past Jackson**, `PASS #0: SanyParser`, then IR-decode error (below) |
| `types,table,effects,warnings,errors`     | ~0.72 MB      | past Jackson, same IR-decode error |
| (hello spec) `warnings,errors,effects`    | ~6.1 KB       | green (`PASS`) |

Only removing **both** `types` and `table` brings dpio under the cap — but
Apalache then rejects the IR as invalid:

```
> Error parsing file StringSource(qnt)
> Input was not a valid representation of the QuintIR: missing keys in dictionary: types, table at index 3990616
error: internal error: while parsing in Apalache:
'Input was not a valid representation of the QuintIR: missing keys in dictionary: types, table ...'
```

**Conclusion: `types` and `table` are mandatory IR keys; a naive strip is not a
fix.** The payload must shrink while keeping both keys' information — i.e. by
interning/sharing (2b), a transport change, or a larger Apalache cap (2c).

### 2b. Redundancy — how much would interning save?

Measured over the dpio dump (distinct = count of distinct JSON serializations of
the map's values):

| Map     | entries | distinct | distinct % | dup factor | value bytes total | distinct-only bytes |
|---------|---------|----------|------------|------------|-------------------|---------------------|
| `types` | 19,136  | 1,123    | 5.9%       | 17.0x      | ~62.10 MB         | ~3.62 MB (5.8%)     |
| `table` | 7,282   | 2,444    | 33.6%      | 3.0x       | ~41.88 MB         | ~13.59 MB (32.4%)   |

Interning the values (a shared value pool plus id references) would shrink the
`types`+`table` value payload from ~104 MB to roughly ~17 MB plus key/reference
overhead. That helps substantially, but for this model the interned total plus
`modules` and `effects` still lands near the 20 MB cap, so interning alone is a
large improvement that is not guaranteed sufficient for bigger specs. It pairs
best with 2c.

### 2c. Upstream knob options for a contributor

**Raise Apalache's JSON string cap (Scala).** The limit comes from Jackson's
`com.fasterxml.jackson.core.StreamReadConstraints`. Apalache's RPC server reads
the `config` string of the `CHECK`/`TLA` command; the fix is to build its JSON
reader / `JsonFactory` with a larger (or unbounded) `maxStringLength`. Where to
look in `apalache-mc/apalache`: the gRPC server module (`shai`) that implements
the command executor service handling `CmdExecutor`/`CHECK`, and wherever its
config string is decoded (search the tree for `StreamReadConstraints`,
`JsonFactory`, `jsonFactory`, `ujson`, or `upickle` in the server/`shai` and
`tla-io` modules). Raising `maxStringLength` there unblocks arbitrarily large
configs at the cost of relaxing a DoS guard on a local trusted channel.

**Change quint's transport (TypeScript).** In `dist/src/apalache.js`
(sources under `src/`), `createConfig` sets `source: { kind: 'string', format:
'qnt', content: parsedSpec }` and the `check`/`tla` closures inline
`JSON.stringify(c)`. Two PR shapes: (i) if Apalache's `SourceOption` accepts a
file path, write `parsedSpec` to a temp file and send a path instead of the
inline string (removes the giant string from the RPC entirely); (ii) compress
`source.content` (gzip) if the server is taught to inflate it. Either touches
`createConfig` and the two submit closures plus the matching Apalache-side
reader.

### 2d. Local stopgap assessment

A `pnpm patch` on `apalache.js` that strips keys is **not viable**: 2a shows
`types`/`table` are required, and nothing smaller than removing both fits under
the cap. A `pnpm patch` that writes `source.content` to a temp file would only
work if Apalache's RPC already accepts a file-path source; the quint client
sends only `kind: 'string'`, so that is a real code change, not a config tweak.
The only dependable local unblock is a patched Apalache build with a raised
`StreamReadConstraints` cap. Until one of the upstream fixes lands, the
affected `verify` targets stay at the simulator rung (see ADR-0002 §1).

---

## 3. Secondary observation (candidate second issue, less developed)

Framed as measurements, not a bug claim: a sampler change is legitimate
behavior. Under quint 0.33.0's default Rust evaluator:

- The `compile()`-heavy intent model samples far slower than the other intent
  models: ~5 traces/s on the intent-alphabet coverage run (wall 618,919 ms for
  one coverage invocation) versus 552 traces/s and 1,150 traces/s on the two
  sibling intent models in the same suite. The slowdown tracks the cost of the
  pure `compile()` function under the Rust evaluator.
- With unchanged pinned seeds, seven `--witnesses` went zero-hit on the Rust
  sampler: `wForeignAnchor`, `wQueueEnvelopeExceeded`, `wInfeasible`,
  `wRenameDoubleClaim`, `wKernelDeclared`, `wUnmeasuredCombination` (intent
  alphabet, one seed) and `wDuplicateName` (raw alphabet, another seed). Some of
  these are documented in the model as structurally unreachable against the
  reference inventory regardless of sampler, so the sampler-sensitive subset is
  smaller; a 0.32.0 re-measurement was not taken (the pin advanced), so this is
  a zero-hit list under 0.33.0, not a confirmed per-witness regression.

If filed, this is a performance/sampling-parity note for the Rust evaluator on
heavy pure functions, with a request to confirm expected witness reachability.

---

## Provenance

- Measurement date: 2026-10-13.
- Toolchain: quint 0.33.0; Apalache 0.62.1 (build a321771); JDK 25 host (JDK 21
  supported floor); pnpm 12.10.x; Linux Fedora 44.
- Beads: dpaa2-controlplane-qhb, dpaa2-controlplane-gbj; sealed-gap context in
  ADR-0002 §1 (bead dpaa2-controlplane-gqf.26).
- Raw payload dumps and per-variant logs were captured in ephemeral session
  scratch and are not retained in the repository; Section 1's repro guidance
  regenerates them.
