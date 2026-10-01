import type { ChannelFieldDto } from "@/api/control"
import { Field, FieldLabel } from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import { Textarea } from "@/components/ui/textarea"
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { Switch } from "@/components/portal/providers/switch"
import { humanize, type FieldValues } from "@/components/portal/providers/field-values"

export function ChannelFields({ fields, values, onChange, advanced }: { fields: Array<ChannelFieldDto>; values: FieldValues; onChange: (key: string, value: string) => void; advanced: boolean }) {
  const visible = fields.filter((field) => advanced || !field.advanced)
  if (visible.length === 0) return <p className="text-sm text-muted-foreground">—</p>
  return (
    <>
      {visible.map((field) => (
        <Field key={field.key} orientation={field.control === "boolean" ? "horizontal" : "vertical"}>
          <FieldLabel htmlFor={`field-${field.key}`}>
            {humanize(field.key)}
            {field.required ? <span className="text-destructive"> *</span> : null}
          </FieldLabel>
          <FieldInput field={field} value={values[field.key] ?? ""} onChange={(value) => onChange(field.key, value)} />
        </Field>
      ))}
    </>
  )
}

function FieldInput({ field, value, onChange }: { field: ChannelFieldDto; value: string; onChange: (value: string) => void }) {
  const id = `field-${field.key}`
  switch (field.control) {
    case "boolean":
      return <Switch checked={value === "true"} onChange={(next) => onChange(next ? "true" : "false")} label={field.key} />
    case "select":
      return (
        <Select value={value} onValueChange={onChange}>
          <SelectTrigger id={id}><SelectValue placeholder={field.key} /></SelectTrigger>
          <SelectContent>
            {field.options.map((option) => <SelectItem key={option} value={option}>{option}</SelectItem>)}
          </SelectContent>
        </Select>
      )
    case "integer":
      return <Input id={id} type="number" value={value} onChange={(event) => onChange(event.target.value)} />
    case "string_list":
      return <Textarea id={id} value={value} onChange={(event) => onChange(event.target.value)} placeholder="one per line" />
    case "secret":
      return <Input id={id} type="password" autoComplete="off" value={value} onChange={(event) => onChange(event.target.value)} />
    case "url":
      return <Input id={id} type="url" value={value} onChange={(event) => onChange(event.target.value)} placeholder="https://" />
    default:
      return <Input id={id} value={value} onChange={(event) => onChange(event.target.value)} />
  }
}
