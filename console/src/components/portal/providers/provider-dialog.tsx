import { useState } from "react"
import { useTranslation } from "react-i18next"
import type { ChannelDto, ChannelFieldDto, ProviderDto, ProviderWriteRequest } from "@/api/control"
import { Button } from "@/components/ui/button"
import { Dialog, DialogBody, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { ChannelFields } from "@/components/portal/providers/channel-fields"
import { defaultsFor, type FieldValues } from "@/components/portal/providers/field-values"
import { Switch } from "@/components/portal/providers/switch"

export function ProviderDialog({ open, onOpenChange, provider, channels, pending, onSubmit }: {
  open: boolean
  onOpenChange: (open: boolean) => void
  provider: ProviderDto | null
  channels: Array<ChannelDto>
  pending: boolean
  onSubmit: (value: ProviderWriteRequest) => void
}) {
  const { t } = useTranslation()
  const [name, setName] = useState(provider?.name ?? "")
  const [label, setLabel] = useState(provider?.label ?? "")
  const [channel, setChannel] = useState(provider?.channel ?? channels[0]?.id ?? "")
  const [strategy, setStrategy] = useState(provider?.credential_strategy ?? "round_robin")
  const [proxyUrl, setProxyUrl] = useState(provider?.proxy_url ?? "")
  const [enabled, setEnabled] = useState(provider?.enabled ?? true)
  const [advanced, setAdvanced] = useState(false)
  const channelDef = channels.find((candidate) => candidate.id === channel)
  const [settings, setSettings] = useState<FieldValues>(() => initialSettings(provider, channels))

  const changeChannel = (next: string) => {
    setChannel(next)
    setSettings(defaultsFor(channels.find((candidate) => candidate.id === next)?.provider_fields ?? []))
  }

  const submit = () => {
    const fields = channelDef?.provider_fields ?? []
    onSubmit({
      name: name.trim(),
      label: label.trim() || null,
      channel,
      settings: buildSettings(fields, settings, provider?.settings ?? {}),
      traffic_policy: provider?.traffic_policy ?? null,
      credential_strategy: strategy,
      proxy_url: proxyUrl.trim() || null,
      tls_fingerprint: provider?.tls_fingerprint ?? null,
      enabled,
    })
  }

  const missingRequired = (channelDef?.provider_fields ?? []).some((field) => field.required && !settings[field.key])

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-lg" closeLabel={t("portal.common.close")}>
        <DialogHeader>
          <DialogTitle>{provider ? t("portal.providers.editProvider") : t("portal.providers.addProvider")}</DialogTitle>
        </DialogHeader>
        <DialogBody>
          <FieldGroup className="sm:grid-cols-1">
          <Field><FieldLabel htmlFor="p-name">{t("portal.providers.name")}</FieldLabel><Input id="p-name" value={name} onChange={(event) => setName(event.target.value)} /></Field>
          <Field><FieldLabel htmlFor="p-label">{t("portal.providers.label")}</FieldLabel><Input id="p-label" value={label} onChange={(event) => setLabel(event.target.value)} /></Field>
          <Field><FieldLabel>{t("portal.providers.channel")}</FieldLabel>
            <Select value={channel} onValueChange={changeChannel}>
              <SelectTrigger><SelectValue /></SelectTrigger>
              <SelectContent>{channels.map((candidate) => <SelectItem key={candidate.id} value={candidate.id}>{candidate.display_name}</SelectItem>)}</SelectContent>
            </Select>
          </Field>
          <ChannelFields fields={channelDef?.provider_fields ?? []} values={settings} onChange={(key, value) => setSettings((current) => ({ ...current, [key]: value }))} advanced={advanced} />
          <Field><FieldLabel>{t("portal.providers.strategy")}</FieldLabel>
            <Select value={strategy} onValueChange={setStrategy}>
              <SelectTrigger><SelectValue /></SelectTrigger>
              <SelectContent>
                <SelectItem value="round_robin">{t("portal.providers.roundRobin")}</SelectItem>
                <SelectItem value="sticky">{t("portal.providers.sticky")}</SelectItem>
              </SelectContent>
            </Select>
          </Field>
          <Field><FieldLabel htmlFor="p-proxy">{t("portal.providers.proxyUrl")}</FieldLabel><Input id="p-proxy" value={proxyUrl} onChange={(event) => setProxyUrl(event.target.value)} /></Field>
          <Field orientation="horizontal">
            <FieldLabel>{t("portal.providers.advanced")}</FieldLabel>
            <Switch checked={advanced} onChange={setAdvanced} />
          </Field>
          <Field orientation="horizontal">
            <FieldLabel>{t("portal.providers.enabled")}</FieldLabel>
            <Switch checked={enabled} onChange={setEnabled} />
          </Field>
        </FieldGroup>
        </DialogBody>
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>{t("portal.common.cancel")}</Button>
          <Button onClick={submit} disabled={pending || !name.trim() || !channel || missingRequired}>{t("portal.common.save")}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}

function initialSettings(provider: ProviderDto | null, channels: Array<ChannelDto>): FieldValues {
  const channelDef = channels.find((candidate) => candidate.id === (provider?.channel ?? channels[0]?.id))
  const values = defaultsFor(channelDef?.provider_fields ?? [])
  if (!provider) return values
  for (const field of channelDef?.provider_fields ?? []) {
    const raw = provider.settings[field.key]
    if (raw === undefined || raw === null) continue
    values[field.key] = Array.isArray(raw) ? raw.join("\n") : String(raw)
  }
  return values
}

function buildSettings(fields: Array<ChannelFieldDto>, values: FieldValues, existing: Record<string, unknown>): Record<string, unknown> {
  const out = { ...existing }
  for (const field of fields) {
    const raw = values[field.key] ?? ""
    if (raw === "") {
      delete out[field.key]
      continue
    }
    out[field.key] = coerce(field.control, raw)
  }
  return out
}

function coerce(control: ChannelFieldDto["control"], raw: string): unknown {
  if (control === "integer") return Number(raw)
  if (control === "boolean") return raw === "true"
  if (control === "string_list") return raw.split(/\r?\n/).map((value) => value.trim()).filter(Boolean)
  return raw
}
