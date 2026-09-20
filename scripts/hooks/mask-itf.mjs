#!/usr/bin/env node
// Git clean filter for quint ITF traces (bead dpaa2-controlplane-s1o).
//
// The volatile `#meta` fields below move on every re-freeze and would
// otherwise churn git. Registered as `filter.itfmask.clean`, git pipes each
// trace through this program on `git add`/status/diff: it reads the content on
// stdin and writes the masked form to stdout, so the STORED blob is the stable
// masked version. A timestamp-only re-freeze then produces no `git status`
// entry; a real semantic change still shows. Freshly frozen working-tree files
// keep quint's verbatim bytes on disk — git normalizes on the way in.
//
// Masking (extensible for other artifact types): each entry maps a volatile
// pattern to a stable placeholder.
import { readFileSync } from "node:fs";
import { pathToFileURL } from "node:url";

const VOLATILE = [
  [/"timestamp":\s*\d+/g, '"timestamp":0'],
  [/"description":"Created by Quint[^"]*"/g, '"description":"Created by Quint"'],
];

export function maskItf(text) {
  return VOLATILE.reduce((acc, [re, sub]) => acc.replace(re, sub), text);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  // clean-filter mode reads stdin (fd 0); a path arg is a testability aid.
  const src = process.argv[2] ?? 0;
  process.stdout.write(maskItf(readFileSync(src, "utf8")));
}
