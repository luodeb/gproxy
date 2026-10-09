import { readFile, readdir } from "node:fs/promises"
import path from "node:path"
import { fileURLToPath } from "node:url"

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../src/locales")
const locales = ["en", "zh-CN", "zh-TW"]
const domains = ["common", "portal"]
const flatten = (value, prefix = "") =>
  Object.entries(value).flatMap(([key, child]) => {
    const path = prefix ? `${prefix}.${key}` : key
    return child && typeof child === "object" ? flatten(child, path) : [path]
  })

// i18next interpolates {{name}}; a lone {name} is a typo that renders verbatim.
const singleBrace = /(?<!\{)\{\s*[a-zA-Z_][a-zA-Z0-9_]*\s*\}(?!\})/
const placeholderNames = (value) =>
  [...value.matchAll(/\{\{([a-zA-Z_][a-zA-Z0-9_]*)\}\}/g)].map((match) => match[1]).sort()

const collect = (value, prefix = "") =>
  Object.entries(value).flatMap(([key, child]) => {
    const path = prefix ? `${prefix}.${key}` : key
    return child && typeof child === "object" ? collect(child, path) : [[path, child]]
  })

for (const locale of locales) {
  const files = (await readdir(path.join(root, locale))).filter((file) => file.endsWith(".json")).map((file) => file.slice(0, -5)).sort()
  if (files.join(",") !== [...domains].sort().join(",")) {
    throw new Error(`${locale} domain drift; expected=${domains.join(",")} actual=${files.join(",")}`)
  }
}

const problems = []
const fail = (message) => problems.push(message)

for (const domain of domains) {
  const tables = await Promise.all(locales.map(async (locale) => {
    const value = JSON.parse(await readFile(path.join(root, locale, `${domain}.json`), "utf8"))
    return [locale, new Map(collect(value))]
  }))
  const referenceKeys = tables[0][1]
  for (const [locale, current] of tables.slice(1)) {
    const missing = [...referenceKeys.keys()].filter((key) => !current.has(key))
    const extra = [...current.keys()].filter((key) => !referenceKeys.has(key))
    if (missing.length || extra.length) {
      fail(`${locale}/${domain} locale drift; missing=${missing.join(",")} extra=${extra.join(",")}`)
    }
  }
  // Placeholder hygiene: every interpolation is {{name}}, and every locale
  // interpolates the same names for the same key — a {{conut}} typo in one
  // language would render the raw key there and only there.
  for (const [locale, entries] of tables) {
    for (const [key, value] of entries) {
      if (typeof value !== "string") continue
      if (singleBrace.test(value)) {
        fail(`${locale}/${domain} ${key}: single-brace placeholder in ${JSON.stringify(value)}; use {{name}}`)
      }
    }
  }
  const [referenceLocale, referenceEntries] = tables[0]
  for (const [key, value] of referenceEntries) {
    if (typeof value !== "string") continue
    const expected = placeholderNames(value).join(",")
    for (const [locale, current] of tables.slice(1)) {
      const actual = placeholderNames(current.get(key) ?? "").join(",")
      if (actual !== expected) {
        fail(`${locale}/${domain} ${key}: placeholders (${actual}) do not match ${referenceLocale} (${expected})`)
      }
    }
  }
}

if (problems.length) {
  throw new Error(`i18n placeholder problems:\n  ${problems.join("\n  ")}`)
}
