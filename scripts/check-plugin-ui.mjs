#!/usr/bin/env node
/**
 * Validate UI plugin descriptors against composition wiring and (when supplied)
 * the compiled Rust plugin manifest catalog.
 */
import fs from 'node:fs'
import path from 'node:path'
import ts from 'typescript'
import { createRequire } from 'node:module'

const require = createRequire(import.meta.url)
const root = process.cwd()
const uiRoot = path.join(root, 'src/plugins')
const rustRoot = path.join(root, 'crates/plugins')
const cargoText = fs.readFileSync(path.join(root, 'crates/shop-api/Cargo.toml'), 'utf8')
const wired = new Set([...cargoText.matchAll(/^plugin-([a-z0-9_]+)\s*=\s*\{/gm)].map((match) => match[1]))
const compiled = new Set(
  fs
    .readdirSync(rustRoot, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => entry.name),
)
const disabledRoot = path.join(root, 'crates/plugins-disabled')
const disabled = new Set(
  fs.existsSync(disabledRoot)
    ? fs
        .readdirSync(disabledRoot, { withFileTypes: true })
        .filter((entry) => entry.isDirectory())
        .map((entry) => entry.name)
    : [],
)
const failures = []
const claims = new Map()
const slotHosts = new Set()
const slotContributions = []
const uiMetaById = new Map()

function fail(message) {
  failures.push(message)
}
function property(object, name) {
  return object?.properties?.find(
    (item) => ts.isPropertyAssignment(item) && item.name.getText().replaceAll(/["']/g, '') === name,
  )?.initializer
}
function stringValue(node) {
  return node && (ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node))
    ? node.text
    : undefined
}
function propertyString(object, name) {
  return stringValue(property(object, name))
}
function claim(kind, value, owner) {
  if (!value) return
  const key = `${kind}:${value}`
  const previous = claims.get(key)
  if (previous && previous !== owner) fail(`${value} is claimed by both ${previous} and ${owner} (${kind})`)
  else claims.set(key, owner)
}
function entries(node) {
  return node && ts.isArrayLiteralExpression(node) ? [...node.elements] : []
}
function collectNav(items, owner) {
  for (const item of items ?? []) {
    claim('nav id', item.id, owner)
    claim('nav href', item.href, owner)
    collectNav(item.children, owner)
  }
}

function loadGenerated(id) {
  const file = path.join(uiRoot, id, 'manifest.generated.ts')
  if (!fs.existsSync(file)) return null
  // Evaluate the generated TS by stripping types and importing via a temp CJS bridge is heavy;
  // parse the exported object with a light regex/JSON approach instead: re-run through ts.transpile.
  const source = fs.readFileSync(file, 'utf8')
  const js = ts.transpileModule(source, {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2020 },
  }).outputText
  const module = { exports: {} }
  const fn = new Function('exports', 'require', 'module', js)
  fn(module.exports, require, module)
  return module.exports.metadata ?? null
}

for (const entry of fs.readdirSync(uiRoot, { withFileTypes: true }).filter((item) => item.isDirectory())) {
  const id = entry.name
  const file = path.join(uiRoot, id, 'index.ts')
  if (!fs.existsSync(file)) {
    fail(`UI plugin ${id} has no index.ts`)
    continue
  }
  const sourceText = fs.readFileSync(file, 'utf8')
  const source = ts.createSourceFile(file, sourceText, ts.ScriptTarget.Latest, true)
  const descriptors = []
  for (const statement of source.statements) {
    if (
      !ts.isVariableStatement(statement) ||
      !statement.modifiers?.some((modifier) => modifier.kind === ts.SyntaxKind.ExportKeyword)
    ) {
      continue
    }
    for (const declaration of statement.declarationList.declarations) {
      const value = declaration.initializer
      if (
        value &&
        ts.isCallExpression(value) &&
        value.expression.getText(source) === 'defineUiPlugin'
      ) {
        descriptors.push(value.arguments[0])
      }
    }
  }
  if (descriptors.length !== 1 || !descriptors[0] || !ts.isObjectLiteralExpression(descriptors[0])) {
    fail(`${file} must export exactly one defineUiPlugin descriptor (found ${descriptors.length})`)
    continue
  }
  const descriptor = descriptors[0]
  const generated = loadGenerated(id)
  const declaredId = propertyString(descriptor, 'id') ?? generated?.id
  if (declaredId !== id) fail(`${file} declares id ${declaredId ?? '(missing)'}, expected ${id}`)

  const requiresBackendExplicit = property(descriptor, 'requiresBackend')
  const requiresBackend =
    requiresBackendExplicit?.kind === ts.SyntaxKind.FalseKeyword
      ? false
      : (generated?.requiresBackend ?? true)
  if (requiresBackend && !disabled.has(id) && (!wired.has(id) || !compiled.has(id))) {
    fail(`${id} requires a backend but is not wired into shop-api`)
  }
  if (requiresBackend && !generated) {
    fail(`${id} is backend-backed but missing manifest.generated.ts (run codegen)`)
  }

  const meta = generated ?? {
    id,
    permissions: entries(property(descriptor, 'permissions')).map(stringValue).filter(Boolean),
    nav: [],
    settingsPages: [],
    reportPages: [],
    slotsProvided: entries(property(descriptor, 'slotsProvided')).map(stringValue).filter(Boolean),
    contributions: [],
  }
  // When generated, prefer it for nav/settings/slots; still collect page paths from index.
  if (generated) {
    uiMetaById.set(id, generated)
    collectNav(generated.nav, id)
    for (const page of generated.settingsPages ?? []) {
      claim('settingsPages id', page.id, id)
      claim('settingsPages href', page.href, id)
    }
    for (const page of generated.reportPages ?? []) {
      claim('reportPages id', page.id, id)
      claim('reportPages href', page.href, id)
    }
    for (const slot of generated.slotsProvided ?? []) {
      claim('slot provider', slot, id)
      slotHosts.add(slot)
    }
    for (const contribution of generated.contributions ?? []) {
      claim('slot contribution', `${contribution.slot}:${contribution.id}`, id)
      slotContributions.push({ slot: contribution.slot, owner: id })
    }
  } else {
    uiMetaById.set(id, meta)
    // Frontend-only plugins (admin): walk the object literal.
    for (const item of entries(property(descriptor, 'nav'))) {
      if (!ts.isObjectLiteralExpression(item)) continue
      claim('nav id', propertyString(item, 'id'), id)
      claim('nav href', propertyString(item, 'href'), id)
    }
    for (const field of ['settingsPages', 'reportPages']) {
      for (const page of entries(property(descriptor, field))) {
        if (!ts.isObjectLiteralExpression(page)) continue
        claim(`${field} id`, propertyString(page, 'id'), id)
        claim(`${field} href`, propertyString(page, 'href'), id)
      }
    }
    for (const slot of entries(property(descriptor, 'slotsProvided'))) {
      const name = stringValue(slot)
      claim('slot provider', name, id)
      if (name) slotHosts.add(name)
    }
    for (const contribution of entries(property(descriptor, 'contributions'))) {
      if (ts.isObjectLiteralExpression(contribution)) {
        const slot = propertyString(contribution, 'slot')
        claim('slot contribution', `${slot}:${propertyString(contribution, 'id')}`, id)
        if (slot) slotContributions.push({ slot, owner: id })
      }
    }
  }

  for (const page of entries(property(descriptor, 'pages'))) {
    if (ts.isObjectLiteralExpression(page)) claim('page path', propertyString(page, 'path'), id)
  }
}

for (const contribution of slotContributions) {
  if (!slotHosts.has(contribution.slot)) {
    fail(`${contribution.owner} contributes to slot ${contribution.slot}, but no plugin provides it`)
  }
}

for (const id of compiled) {
  if (!wired.has(id)) {
    fail(`crates/plugins/${id} is neither wired nor explicitly moved to crates/plugins-disabled`)
  }
}
for (const id of wired) {
  if (!compiled.has(id)) fail(`shop-api wires plugin-${id}, but crates/plugins/${id} does not exist`)
}

const manifestFile = process.argv[2]
if (manifestFile) {
  const manifests = JSON.parse(fs.readFileSync(manifestFile, 'utf8'))
  const sorted = (value) => JSON.stringify(value)
  const pairs = (items) => items.map((item) => `${item[0]}:${item[1]}`).sort()
  const navPairs = (items) =>
    (items ?? []).flatMap((item) => [[item.id, item.href ?? ''], ...navPairs(item.children ?? [])])
  for (const manifest of manifests) {
    const descriptor = uiMetaById.get(manifest.id)
    if (!descriptor) continue
    const expected = {
      permissions: [...(manifest.permissions ?? [])].sort(),
      nav: pairs(navPairs(manifest.nav ?? [])),
      settings: pairs((manifest.settings_pages ?? []).map((page) => [page.id, page.href ?? ''])),
      reports: pairs((manifest.report_pages ?? []).map((page) => [page.id, page.href ?? ''])),
      slots: [...(manifest.slots_provided ?? [])].sort(),
      contributions: pairs(
        (manifest.slot_contributions ?? []).map((slot) => [slot.name, slot.contribution]),
      ),
    }
    const actual = {
      permissions: [...(descriptor.permissions ?? [])].sort(),
      nav: pairs(navPairs(descriptor.nav ?? [])),
      settings: pairs((descriptor.settingsPages ?? []).map((page) => [page.id, page.href ?? ''])),
      reports: pairs((descriptor.reportPages ?? []).map((page) => [page.id, page.href ?? ''])),
      slots: [...(descriptor.slotsProvided ?? [])].sort(),
      contributions: pairs(
        (descriptor.contributions ?? []).map((slot) => [slot.slot, slot.id]),
      ),
    }
    for (const field of Object.keys(expected)) {
      if (sorted(expected[field]) !== sorted(actual[field])) {
        fail(
          `${manifest.id}: UI ${field} does not match the Rust manifest (ui=${JSON.stringify(actual[field])}, rust=${JSON.stringify(expected[field])})`,
        )
      }
    }
  }
}

if (failures.length) {
  console.error(failures.map((message) => `- ${message}`).join('\n'))
  process.exitCode = 1
} else {
  console.log(`Plugin UI and composition metadata are consistent (${wired.size} backend plugins).`)
}
