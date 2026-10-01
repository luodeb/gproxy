import type { ChannelFieldDto } from "@/api/control"

export type FieldValues = Record<string, string>

export function defaultsFor(fields: Array<ChannelFieldDto>): FieldValues {
  const values: FieldValues = {}
  for (const field of fields) values[field.key] = field.default_value ?? ""
  return values
}

export function humanize(key: string): string {
  return key.replace(/[_-]+/g, " ").replace(/\b\w/g, (character) => character.toUpperCase())
}
