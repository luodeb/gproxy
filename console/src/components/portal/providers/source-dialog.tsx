import { useState } from "react"
import { useTranslation } from "react-i18next"
import type { ModelSourceDto, ModelSourceWriteRequest, ProviderDto, ProviderModelDto } from "@/api/control"
import { Button } from "@/components/ui/button"
import { Dialog, DialogBody, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { Switch } from "@/components/portal/providers/switch"

export function SourceDialog({ open, onOpenChange, source, providers, providerModels, pending, onSubmit }: {
  open: boolean
  onOpenChange: (open: boolean) => void
  source: ModelSourceDto | null
  providers: Array<ProviderDto>
  providerModels: Array<ProviderModelDto>
  pending: boolean
  onSubmit: (value: ModelSourceWriteRequest) => void
}) {
  const { t } = useTranslation()
  const [providerId, setProviderId] = useState(source ? String(source.provider_id) : providers[0] ? String(providers[0].id) : "")
  const [upstream, setUpstream] = useState(source?.upstream_model ?? "")
  const [weight, setWeight] = useState(String(source?.weight ?? 100))
  const [tier, setTier] = useState(String(source?.tier ?? 0))
  const [enabled, setEnabled] = useState(source?.enabled ?? true)

  const options = providerModels.filter((model) => model.provider_id === Number(providerId))
  const changeProvider = (next: string) => { setProviderId(next); setUpstream("") }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent closeLabel={t("portal.common.close")}>
        <DialogHeader><DialogTitle>{source ? t("portal.providers.editSource") : t("portal.providers.addSource")}</DialogTitle></DialogHeader>
        <DialogBody>
          <FieldGroup className="sm:grid-cols-1">
          <Field><FieldLabel>{t("portal.providers.provider")}</FieldLabel>
            <Select value={providerId} onValueChange={changeProvider}>
              <SelectTrigger><SelectValue /></SelectTrigger>
              <SelectContent>{providers.map((provider) => <SelectItem key={provider.id} value={String(provider.id)}>{provider.label ?? provider.name}</SelectItem>)}</SelectContent>
            </Select>
          </Field>
          <Field><FieldLabel>{t("portal.providers.upstreamModel")}</FieldLabel>
            <Select value={upstream} onValueChange={setUpstream} disabled={options.length === 0}>
              <SelectTrigger><SelectValue placeholder={options.length ? undefined : t("portal.providers.noUpstreamModels")} /></SelectTrigger>
              <SelectContent>{options.map((model) => <SelectItem key={model.id} value={model.model_id}>{model.display_name ? `${model.display_name} (${model.model_id})` : model.model_id}</SelectItem>)}</SelectContent>
            </Select>
          </Field>
          <Field><FieldLabel htmlFor="s-weight">{t("portal.providers.weight")}</FieldLabel><Input id="s-weight" type="number" min={1} value={weight} onChange={(event) => setWeight(event.target.value)} /></Field>
          <Field><FieldLabel htmlFor="s-tier">{t("portal.providers.tier")}</FieldLabel><Input id="s-tier" type="number" value={tier} onChange={(event) => setTier(event.target.value)} /></Field>
          <Field orientation="horizontal">
            <FieldLabel>{t("portal.providers.enabled")}</FieldLabel>
            <Switch checked={enabled} onChange={setEnabled} />
          </Field>
        </FieldGroup>
        </DialogBody>
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>{t("portal.common.cancel")}</Button>
          <Button disabled={pending || !upstream} onClick={() => onSubmit({ provider_id: Number(providerId), upstream_model: upstream, tier: Number(tier) || 0, weight: Math.max(1, Number(weight) || 1), enabled })}>{t("portal.common.save")}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
