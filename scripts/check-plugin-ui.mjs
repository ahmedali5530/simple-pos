import fs from 'node:fs'
import path from 'node:path'
import ts from 'typescript'

const root = process.cwd()
const uiRoot = path.join(root, 'src/plugins')
const rustRoot = path.join(root, 'crates/plugins')
const cargoText = fs.readFileSync(path.join(root, 'crates/shop-api/Cargo.toml'), 'utf8')
const wired = new Set([...cargoText.matchAll(/^plugin-([a-z0-9_]+)\s*=\s*\{/gm)].map((match) => match[1]))
const compiled = new Set(fs.readdirSync(rustRoot, { withFileTypes: true }).filter((entry) => entry.isDirectory()).map((entry) => entry.name))
const disabledRoot = path.join(root, 'crates/plugins-disabled')
const disabled = new Set(fs.existsSync(disabledRoot) ? fs.readdirSync(disabledRoot, { withFileTypes: true }).filter((entry) => entry.isDirectory()).map((entry) => entry.name) : [])
const failures = []
const claims = new Map()
const slotHosts = new Set()
const slotContributions = []
const uiById = new Map()

function fail(message) { failures.push(message) }
function property(object, name) {
  return object?.properties?.find((item) => ts.isPropertyAssignment(item) && item.name.getText().replaceAll(/["']/g, '') === name)?.initializer
}
function stringValue(node) {
  return node && (ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node)) ? node.text : undefined
}
function propertyString(object, name) { return stringValue(property(object, name)) }
function claim(kind, value, owner) {
  if (!value) return
  const key = `${kind}:${value}`
  const previous = claims.get(key)
  if (previous && previous !== owner) fail(`${value} is claimed by both ${previous} and ${owner} (${kind})`)
  else claims.set(key, owner)
}
function entries(node) { return node && ts.isArrayLiteralExpression(node) ? [...node.elements] : [] }
function collectNav(node, owner) {
  for (const item of entries(node)) {
    if (!ts.isObjectLiteralExpression(item)) continue
    claim('nav id', propertyString(item, 'id'), owner)
    claim('nav href', propertyString(item, 'href'), owner)
    collectNav(property(item, 'children'), owner)
  }
}

for (const entry of fs.readdirSync(uiRoot, { withFileTypes: true }).filter((item) => item.isDirectory())) {
  const id = entry.name
  const file = path.join(uiRoot, id, 'index.ts')
  if (!fs.existsSync(file)) { fail(`UI plugin ${id} has no index.ts`); continue }
  const source = ts.createSourceFile(file, fs.readFileSync(file, 'utf8'), ts.ScriptTarget.Latest, true)
  const descriptors = []
  for (const statement of source.statements) {
    if (!ts.isVariableStatement(statement) || !statement.modifiers?.some((modifier) => modifier.kind === ts.SyntaxKind.ExportKeyword)) continue
    for (const declaration of statement.declarationList.declarations) {
      const value = declaration.initializer
      if (value && ts.isCallExpression(value) && value.expression.getText(source) === 'defineUiPlugin') descriptors.push(value.arguments[0])
    }
  }
  if (descriptors.length !== 1 || !descriptors[0] || !ts.isObjectLiteralExpression(descriptors[0])) {
    fail(`${file} must export exactly one defineUiPlugin descriptor (found ${descriptors.length})`)
    continue
  }
  const descriptor = descriptors[0]
  const declaredId = propertyString(descriptor, 'id')
  if (declaredId !== id) fail(`${file} declares id ${declaredId ?? '(missing)'}, expected ${id}`)
  if (declaredId) uiById.set(declaredId, descriptor)
  const requiresBackend = property(descriptor, 'requiresBackend')?.kind !== ts.SyntaxKind.FalseKeyword
  if (requiresBackend && !disabled.has(id) && (!wired.has(id) || !compiled.has(id))) fail(`${id} requires a backend but is not wired into shop-api`)

  collectNav(property(descriptor, 'nav'), id)
  for (const field of ['settingsPages', 'reportPages']) {
    for (const page of entries(property(descriptor, field))) {
      if (!ts.isObjectLiteralExpression(page)) continue
      claim(`${field} id`, propertyString(page, 'id'), id)
      claim(`${field} href`, propertyString(page, 'href'), id)
    }
  }
  for (const page of entries(property(descriptor, 'pages'))) {
    if (ts.isObjectLiteralExpression(page)) claim('page path', propertyString(page, 'path'), id)
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

for (const contribution of slotContributions) {
  if (!slotHosts.has(contribution.slot)) {
    fail(`${contribution.owner} contributes to slot ${contribution.slot}, but no plugin provides it`)
  }
}

for (const id of compiled) {
  if (!wired.has(id)) fail(`crates/plugins/${id} is neither wired nor explicitly moved to crates/plugins-disabled`)
}
for (const id of wired) {
  if (!compiled.has(id)) fail(`shop-api wires plugin-${id}, but crates/plugins/${id} does not exist`)
}

// Compare the UI descriptor with the Rust manifest when the caller supplies the
// compiled manifest catalog. This catches the common "edited one side" drift.
const manifestFile = process.argv[2]
if (manifestFile) {
  const manifests = JSON.parse(fs.readFileSync(manifestFile, 'utf8'))
  const sorted = (value) => JSON.stringify(value)
  const pairs = (items) => items.map((item) => `${item[0]}:${item[1]}`).sort()
  const navPairs = (items) => items.flatMap((item) => [[item.id, item.href ?? ''], ...navPairs(item.children ?? [])])
  const uiNavPairs = (items) => items.filter(ts.isObjectLiteralExpression).flatMap((item) => {
    const id = propertyString(item, 'id')
    const href = propertyString(item, 'href') ?? ''
    return [[id, href], ...uiNavPairs(entries(property(item, 'children')))]
  })
  for (const manifest of manifests) {
    const descriptor = uiById.get(manifest.id)
    if (!descriptor) continue
    const expected = {
      permissions: [...(manifest.permissions ?? [])].sort(),
      nav: pairs(navPairs(manifest.nav ?? [])),
      settings: pairs((manifest.settings_pages ?? []).map((page) => [page.id, page.href])),
      reports: pairs((manifest.report_pages ?? []).map((page) => [page.id, page.href])),
      slots: [...(manifest.slots_provided ?? [])].sort(),
      contributions: pairs((manifest.slot_contributions ?? []).map((slot) => [slot.name, slot.contribution])),
    }
    const actual = {
      permissions: [...(entries(property(descriptor, 'permissions')).map(stringValue).filter(Boolean))].sort(),
      nav: pairs(uiNavPairs(entries(property(descriptor, 'nav')))),
      settings: pairs(entries(property(descriptor, 'settingsPages')).filter(ts.isObjectLiteralExpression).map((page) => [propertyString(page, 'id'), propertyString(page, 'href')])),
      reports: pairs(entries(property(descriptor, 'reportPages')).filter(ts.isObjectLiteralExpression).map((page) => [propertyString(page, 'id'), propertyString(page, 'href')])),
      slots: [...(entries(property(descriptor, 'slotsProvided')).map(stringValue).filter(Boolean))].sort(),
      contributions: pairs(entries(property(descriptor, 'contributions')).filter(ts.isObjectLiteralExpression).map((slot) => [propertyString(slot, 'slot'), propertyString(slot, 'id')])),
    }
    for (const field of Object.keys(expected)) {
      if (sorted(expected[field]) !== sorted(actual[field])) {
        fail(`${manifest.id}: UI ${field} does not match the Rust manifest (ui=${JSON.stringify(actual[field])}, rust=${JSON.stringify(expected[field])})`)
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
