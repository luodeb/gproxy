import { useState } from "react"
import { useTranslation } from "react-i18next"
import type { ChannelDto, ChannelFieldDto, CredentialDto, CredentialWriteRequest, ProviderDto } from "@/api/control"
import { Button } from "@/components/ui/button"
import { Dialog, DialogBody, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { Field, FieldDescription, FieldGroup, FieldLabel } from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import { ChannelFields } from "@/components/portal/providers/channel-fields"
import { defaultsFor, type FieldValues } from "@/components/portal/providers/field-values"
import { Switch } from "@/components/portal/providers/switch"

export function CredentialDialog({ open, onOpenChange, provider, credential, channel, pending, onSubmit }: {
  open: boolean
  onOpenChange: (open: boolean) => void
  provider: ProviderDto
  credential: CredentialDto | null
  channel: ChannelDto | undefined
  pending: boolean
  onSubmit: (value: CredentialWriteRequest) => void
}) {
  const { t } = useTranslation()
  const [label, setLabel] = useState(credential?.label ?? "")
  const [kind, setKind] = useState(credential?.kind ?? "api_key")
  const [weight, setWeight] = useState(String(credential?.weight ?? 100))
  const [rpm, setRpm] = useState(credential?.rpm_limit == null ? "" : String(credential.rpm_limit))
  const [tpm, setTpm] = useState(credential?.tpm_limit == null ? "" : String(credential.tpm_limit))
  const [enabled, setEnabled] = useState(credential?.enabled ?? true)
  const [secret, setSecret] = useState<FieldValues>(() => defaultsFor(channel?.credential_fields ?? []))
  const editing = credential != null
  const hasSecret = (channel?.credential_fields ?? []).some((field) => (secret[field.key] ?? "") !== "")

  const submit = () => {
    const payload = hasSecret ? buildSecret(channel?.credential_fields ?? [], secret) : null
    onSubmit({
      provider_id: provider.id,
      label: label.trim() || null,
      kind: kind.trim() || "api_key",
      secret: payload,
      quota_secret: null,
      enabled,
      weight: Math.max(1, Number(weight) || 1),
      rpm_limit: rpm === "" ? null : Number(rpm),
      tpm_limit: tpm === "" ? null : Number(tpm),
      proxy_url: null,
      tls_fingerprint: null,
    })
  }

  const missingRequired = (channel?.credential_fields ?? []).some((field) => field.required && !editing && !secret[field.key])

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-lg" closeLabel={t("portal.common.close")}>
        <DialogHeader>
          <DialogTitle>{editing ? t("portal.providers.editKey") : t("portal.providers.addKey")}</DialogTitle>
          <DialogDescription>{provider.label ?? provider.name}</DialogDescription>
        </DialogHeader>
        <DialogBody>
          <FieldGroup className="sm:grid-cols-1">
          <Field><FieldLabel htmlFor="c-label">{t("portal.providers.label")}</FieldLabel><Input id="c-label" value={label} onChange={(event) => setLabel(event.target.value)} /></Field>
          <ChannelFields fields={channel?.credential_fields ?? []} values={secret} onChange={(key, value) => setSecret((current) => ({ ...current, [key]: value }))} advanced />
          {editing ? <p className="text-xs text-muted-foreground">{t("portal.providers.secretHint")}</p> : null}
          <Field><FieldLabel htmlFor="c-weight">{t("portal.providers.weight")}</FieldLabel><Input id="c-weight" type="number" min={1} value={weight} onChange={(event) => setWeight(event.target.value)} /><FieldDescription>{t("portal.providers.weightHint")}</FieldDescription></Field>
          <Field><FieldLabel htmlFor="c-rpm">{t("portal.providers.rpm")}</FieldLabel><Input id="c-rpm" type="number" value={rpm} onChange={(event) => setRpm(event.target.value)} /><FieldDescription>{t("portal.providers.rpmHint")}</FieldDescription></Field>
          <Field><FieldLabel htmlFor="c-tpm">{t("portal.providers.tpm")}</FieldLabel><Input id="c-tpm" type="number" value={tpm} onChange={(event) => setTpm(event.target.value)} /><FieldDescription>{t("portal.providers.tpmHint")}</FieldDescription></Field>
          <Field><FieldLabel htmlFor="c-kind">{t("portal.providers.keyKind")}</FieldLabel><Input id="c-kind" value={kind} onChange={(event) => setKind(event.target.value)} /></Field>
          <Field orientation="horizontal">
            <FieldLabel>{t("portal.providers.enabled")}</FieldLabel>
            <Switch checked={enabled} onChange={setEnabled} />
          </Field>
        </FieldGroup>
        </DialogBody>
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>{t("portal.common.cancel")}</Button>
          <Button onClick={submit} disabled={pending || missingRequired}>{t("portal.common.save")}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}

function buildSecret(fields: Array<ChannelFieldDto>, values: FieldValues): Record<string, unknown> {
  const out: Record<string, unknown> = {}
  for (const field of fields) {
    const raw = values[field.key] ?? ""
    if (raw === "") continue
    out[field.key] = field.control === "integer" ? Number(raw) : field.control === "boolean" ? raw === "true" : raw
  }
  return out
}
