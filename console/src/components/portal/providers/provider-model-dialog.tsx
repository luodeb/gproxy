import { useState } from "react"
import { useTranslation } from "react-i18next"
import type { ProviderModelDto, ProviderModelWriteRequest } from "@/api/control"
import { Button } from "@/components/ui/button"
import { Dialog, DialogBody, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import { Switch } from "@/components/portal/providers/switch"

export function ProviderModelDialog({ open, onOpenChange, providerId, model, pending, onSubmit }: {
  open: boolean
  onOpenChange: (open: boolean) => void
  providerId: number
  model: ProviderModelDto | null
  pending: boolean
  onSubmit: (value: ProviderModelWriteRequest) => void
}) {
  const { t } = useTranslation()
  const [modelId, setModelId] = useState(model?.model_id ?? "")
  const [displayName, setDisplayName] = useState(model?.display_name ?? "")
  const [contextWindow, setContextWindow] = useState(model?.context_window == null ? "" : String(model.context_window))
  const [maxOutput, setMaxOutput] = useState(model?.max_output_tokens == null ? "" : String(model.max_output_tokens))
  const [enabled, setEnabled] = useState(model?.enabled ?? true)

  const submit = () => {
    onSubmit({
      provider_id: providerId,
      model_id: modelId.trim(),
      display_name: displayName.trim() || null,
      variants: model?.variants ?? null,
      context_window: contextWindow === "" ? null : Number(contextWindow),
      max_output_tokens: maxOutput === "" ? null : Number(maxOutput),
      thinking_supported: model?.thinking_supported ?? null,
      thinking_adaptive_supported: model?.thinking_adaptive_supported ?? null,
      thinking_enabled_supported: model?.thinking_enabled_supported ?? null,
      metadata: model?.metadata ?? {},
      enabled,
    })
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-lg" closeLabel={t("portal.common.close")}>
        <DialogHeader>
          <DialogTitle>{model ? t("portal.providers.editUpstreamModel") : t("portal.providers.addUpstreamModel")}</DialogTitle>
        </DialogHeader>
        <DialogBody>
          <FieldGroup className="sm:grid-cols-1">
          <Field><FieldLabel htmlFor="pm-id">{t("portal.providers.modelId")}</FieldLabel><Input id="pm-id" value={modelId} onChange={(event) => setModelId(event.target.value)} /></Field>
          <Field><FieldLabel htmlFor="pm-display">{t("portal.providers.displayName")}</FieldLabel><Input id="pm-display" value={displayName} onChange={(event) => setDisplayName(event.target.value)} /></Field>
          <Field><FieldLabel htmlFor="pm-ctx">{t("portal.providers.contextWindow")}</FieldLabel><Input id="pm-ctx" type="number" value={contextWindow} onChange={(event) => setContextWindow(event.target.value)} /></Field>
          <Field><FieldLabel htmlFor="pm-out">{t("portal.providers.maxOutput")}</FieldLabel><Input id="pm-out" type="number" value={maxOutput} onChange={(event) => setMaxOutput(event.target.value)} /></Field>
          <Field orientation="horizontal">
            <FieldLabel>{t("portal.providers.enabled")}</FieldLabel>
            <Switch checked={enabled} onChange={setEnabled} />
          </Field>
        </FieldGroup>
        </DialogBody>
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>{t("portal.common.cancel")}</Button>
          <Button onClick={submit} disabled={pending || !modelId.trim()}>{t("portal.common.save")}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
