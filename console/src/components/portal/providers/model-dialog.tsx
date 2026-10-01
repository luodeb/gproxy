import { useState } from "react"
import { useTranslation } from "react-i18next"
import type { ManagedModelDto, ManagedModelWriteRequest, ProviderDto, ProviderModelDto } from "@/api/control"
import { Button } from "@/components/ui/button"
import { Dialog, DialogBody, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { Switch } from "@/components/portal/providers/switch"

export type NewModelSource = { provider_id: number; upstream_model: string }

export function ModelDialog({ open, onOpenChange, model, providers, providerModels, pending, onSubmit }: {
  open: boolean
  onOpenChange: (open: boolean) => void
  model: ManagedModelDto | null
  providers: Array<ProviderDto>
  providerModels: Array<ProviderModelDto>
  pending: boolean
  onSubmit: (value: ManagedModelWriteRequest, source: NewModelSource | null) => void
}) {
  const { t } = useTranslation()
  const [name, setName] = useState(model?.name ?? "")
  const [displayName, setDisplayName] = useState(model?.display_name ?? "")
  const [enabled, setEnabled] = useState(model?.enabled ?? true)
  const [sourceKey, setSourceKey] = useState("")

  const providerName = (id: number) => providers.find((p) => p.id === id)?.label ?? providers.find((p) => p.id === id)?.name ?? `#${id}`
  const options = providerModels.map((pm) => ({ key: `${pm.provider_id}:${pm.model_id}`, provider: providerName(pm.provider_id), model: pm.model_id, display: pm.display_name }))

  const pickSource = (key: string) => {
    setSourceKey(key)
    const picked = providerModels.find((pm) => `${pm.provider_id}:${pm.model_id}` === key)
    if (picked) {
      setName(picked.model_id)
      setDisplayName(picked.display_name ?? picked.model_id)
    }
  }

  const submit = () => {
    const source = sourceKey
      ? { provider_id: Number(sourceKey.slice(0, sourceKey.indexOf(":"))), upstream_model: sourceKey.slice(sourceKey.indexOf(":") + 1) }
      : null
    onSubmit({ name: name.trim(), display_name: displayName.trim() || null, enabled }, model ? null : source)
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent closeLabel={t("portal.common.close")}>
        <DialogHeader>
          <DialogTitle>{model ? t("portal.providers.editModel") : t("portal.providers.addModel")}</DialogTitle>
          <DialogDescription>{t("portal.providers.modelHint")}</DialogDescription>
        </DialogHeader>
        <DialogBody>
          <FieldGroup className="sm:grid-cols-1">
            {model == null ? (
              <Field>
                <FieldLabel>{t("portal.providers.modelSource")}</FieldLabel>
                <Select value={sourceKey} onValueChange={pickSource} disabled={options.length === 0}>
                  <SelectTrigger><SelectValue placeholder={options.length ? t("portal.providers.modelSourceHint") : t("portal.providers.noUpstreamModels")} /></SelectTrigger>
                  <SelectContent>
                    {options.map((option) => (
                      <SelectItem key={option.key} value={option.key}>
                        {option.provider} · {option.display ? `${option.display} (${option.model})` : option.model}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </Field>
            ) : null}
            <Field><FieldLabel htmlFor="m-id">{t("portal.providers.modelId")}</FieldLabel><Input id="m-id" value={name} onChange={(event) => setName(event.target.value)} placeholder="gpt-5" /></Field>
            <Field><FieldLabel htmlFor="m-display">{t("portal.providers.displayName")}</FieldLabel><Input id="m-display" value={displayName} onChange={(event) => setDisplayName(event.target.value)} /></Field>
            <Field orientation="horizontal">
              <FieldLabel>{t("portal.providers.enabled")}</FieldLabel>
              <Switch checked={enabled} onChange={setEnabled} />
            </Field>
          </FieldGroup>
        </DialogBody>
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>{t("portal.common.cancel")}</Button>
          <Button onClick={submit} disabled={pending || !name.trim()}>{t("portal.common.save")}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
