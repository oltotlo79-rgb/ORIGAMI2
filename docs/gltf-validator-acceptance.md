# Generated GLB acceptance

CI pins the official Khronos `gltf-validator` npm package to
`2.0.0-dev.3.10` through `apps/desktop/package-lock.json`.

Run the automated acceptance locally:

```text
cd apps/desktop
npm ci
npm run test:gltf-validator
```

The command builds fresh artifacts through the production `ori-formats`
export APIs and requires validator error count zero for:

- static GLB;
- static GLB with an embedded PNG texture;
- animated GLB with STEP morph-target frames.

The generated files are temporary and are removed after validation. The
validator checks the embedded buffer and image resources. Warnings and
informational diagnostics are printed but do not weaken the error-zero gate.

This automated format acceptance is not a substitute for a final physical
print and manual inspection of the complete user workflow.

## Khronos Sample Viewer runtime gate

`npm run test:gltf-sample-viewer` is a separate runtime acceptance. It builds
fresh release-mode GLBs, checks out the official Khronos Sample Viewer at
release commit `d4eabef31e6eb70cbefb939767637539c37c7a33`, and loads each artifact in
headless Chromium with WebGL enabled. CI requires:

- no browser console or uncaught runtime errors;
- a non-empty visible WebGL canvas for static and textured GLB;
- visible frame changes while the animated GLB is playing.

This does not replace physical-print acceptance.

## Blender LTS import gate

`npm run test:blender` generates release-mode OBJ, binary STL, and static,
textured, and animated GLB fixtures. CI downloads the official Blender 4.5.11
LTS Linux archive and verifies SHA-256
`05ed7bd41bf3e61ae4f4a7cdc364c43088bf8b3fed702c2269c018fdf63a2188`
on every run, including cache hits, before extracting the cached archive.

Blender runs with `--background`, factory settings, automatic scripts disabled,
and a nonzero Python exception exit code. The gate requires clean stderr and
checks imported mesh and triangle counts, material and embedded-image presence,
animation and morph-target playback, open-sheet manifold status, documented
millimetre/metre conversion, axes, and world-space bounds.

## PrusaSlicer CLI gate

`npm run test:prusaslicer` generates a 10 × 10 × 2 mm closed positive-thickness
STL in release mode and analyzes it with the official PrusaSlicer 2.9.6 Windows
CLI. CI caches the official ZIP but verifies SHA-256
`5aaf22e42f95accecfa122d23a835911f289ecc2ff606db3e83d637ddcc0a209`
on every run before extraction.

The gate requires a manifold single-part mesh, exact millimetre bounds, 12
triangles, 200 mm³ volume, no repair fields, warnings, errors, or stderr. An
open sheet must fail loading, while a deliberately duplicated-face mesh must
be rejected by the acceptance runner because PrusaSlicer repairs it. A G-code
export must contain six finite, in-bounds model layers and only finite XYZ
motion coordinates. Physical-print and complete UI workflow checks remain
manual.
