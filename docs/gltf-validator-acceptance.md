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

This automated format acceptance is not a substitute for opening exported
files in external GUI applications. Blender and other target viewers still
require a separate manual interoperability pass covering appearance,
animation playback, axes, units, and user workflow.
