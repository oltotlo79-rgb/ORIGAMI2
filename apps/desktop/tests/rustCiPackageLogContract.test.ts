import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const workflow = readFileSync('../../.github/workflows/ci.yml', 'utf8')

test('each Windows crate test has stable begin end and numeric status markers', () => {
  assert.match(workflow, /ORIGAMI2_CARGO_PACKAGE_BEGIN::%s/u)
  assert.match(workflow, /ORIGAMI2_CARGO_PACKAGE_END::%s::status=%s/u)
  assert.match(workflow, /"\$package" "\$package_status"/u)
  assert.match(workflow, /printf '%s\\n' "\$package" > cargo-test-failed-package\.txt/u)
  const begin = workflow.indexOf('ORIGAMI2_CARGO_PACKAGE_BEGIN::%s')
  const command = workflow.indexOf('cargo test -p "$package"', begin)
  const end = workflow.indexOf('ORIGAMI2_CARGO_PACKAGE_END::%s::status=%s', command)
  assert.ok(begin >= 0 && command > begin && end > command)
})

test('failure annotations always identify the package and include the final 30 lines', () => {
  assert.match(workflow, /failed_package="\$\(cat cargo-test-failed-package\.txt\)"/u)
  assert.match(workflow, /tail -n 30 cargo-test\.log/u)
  assert.match(workflow, /failure_summary="package=\$failed_package final-30-lines: \$failure_summary"/u)
  assert.match(workflow, /title=Rust test log tail \(\$failed_package\)::package=\$failed_package final-30-lines:/u)
})

test('macOS workspace execution uses the same marker and annotation protocol', () => {
  assert.match(workflow, /ORIGAMI2_CARGO_PACKAGE_BEGIN::workspace/u)
  assert.match(workflow, /ORIGAMI2_CARGO_PACKAGE_END::workspace::status=%s/u)
  assert.match(workflow, /printf '%s\\n' workspace > cargo-test-failed-package\.txt/u)
})
