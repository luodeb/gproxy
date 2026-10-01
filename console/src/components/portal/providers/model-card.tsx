import { useState } from "react"
import { useTranslation } from "react-i18next"
import { ChevronDownIcon, ChevronRightIcon, PencilIcon, PlusIcon } from "lucide-react"
import type { ManagedModelDto, ModelSourceDto, ProviderDto } from "@/api/control"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardHeader } from "@/components/ui/card"
import { DeleteButton } from "@/components/portal/providers/delete-button"
import { Switch } from "@/components/portal/providers/switch"
import { cn } from "@/lib/utils"

export type ModelActions = {
  onEdit: (model: ManagedModelDto) => void
  onToggle: (model: ManagedModelDto, next: boolean) => void
  onDelete: (model: ManagedModelDto) => void
  onAddSource: (model: ManagedModelDto) => void
  onEditSource: (model: ManagedModelDto, source: ModelSourceDto) => void
  onToggleSource: (model: ManagedModelDto, source: ModelSourceDto, next: boolean) => void
  onDeleteSource: (model: ManagedModelDto, source: ModelSourceDto) => void
}

export function ModelCard({ model, providers, actions }: { model: ManagedModelDto; providers: Array<ProviderDto>; actions: ModelActions }) {
  const { t } = useTranslation()
  const [open, setOpen] = useState(false)
  const providerEnabled = (id: number) => providers.find((candidate) => candidate.id === id)?.enabled ?? false
  const activeSources = model.sources.filter((source) => source.enabled && providerEnabled(source.provider_id)).length
  const effective = model.enabled && activeSources > 0
  const providerName = (id: number) => {
    const provider = providers.find((candidate) => candidate.id === id)
    return provider?.label ?? provider?.name ?? `#${id}`
  }

  return (
    <Card className={cn(!effective && "opacity-80")}>
      <CardHeader className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <button type="button" className="flex min-w-0 flex-1 items-center gap-2 text-left" onClick={() => setOpen((value) => !value)} aria-expanded={open}>
          {open ? <ChevronDownIcon className="size-4 shrink-0" /> : <ChevronRightIcon className="size-4 shrink-0" />}
          <span className="flex min-w-0 flex-1 flex-wrap items-center gap-x-2 gap-y-0.5">
            <span className="truncate font-medium">{model.display_name ?? model.name}</span>
            <span className="truncate font-mono text-xs text-muted-foreground">{model.name}</span>
            {effective
              ? <Badge variant="secondary">{t("portal.providers.activeSourceCount", { count: activeSources })}</Badge>
              : <Badge variant="destructive">{t("portal.providers.noActiveSource")}</Badge>}
          </span>
        </button>
        <div className="flex items-center gap-2">
          <Switch checked={effective} onChange={(next) => { if (activeSources > 0) actions.onToggle(model, next) }} disabled={activeSources === 0} label={t("portal.providers.enabled")} />
          <Button size="icon-sm" variant="ghost" onClick={() => actions.onEdit(model)} aria-label={t("portal.providers.editModel")}><PencilIcon aria-hidden /></Button>
          <DeleteButton title={t("portal.providers.deleteModel")} description={t("portal.providers.deleteModelBody", { name: model.display_name ?? model.name })} onConfirm={() => actions.onDelete(model)} />
        </div>
      </CardHeader>
      {open ? (
        <CardContent className="flex flex-col gap-3">
          <div className="flex items-center justify-between">
            <h3 className="text-sm font-medium">{t("portal.providers.sources")}</h3>
            <Button size="sm" variant="outline" onClick={() => actions.onAddSource(model)}><PlusIcon aria-hidden />{t("portal.providers.addSource")}</Button>
          </div>
          {model.sources.length === 0 ? <p className="text-sm text-muted-foreground">{t("portal.providers.noSources")}</p> : null}
          <div className="flex flex-col gap-2">
            {model.sources.map((source) => (
              <div key={source.id} className="flex flex-col gap-2 rounded-md border px-3 py-2 sm:flex-row sm:items-center sm:justify-between">
                <div className="flex min-w-0 flex-wrap items-center gap-x-3 gap-y-1">
                  <span className="truncate font-medium">{providerName(source.provider_id)}</span>
                  <span className="truncate font-mono text-xs text-muted-foreground">{source.upstream_model}</span>
                </div>
                <div className="flex items-center gap-2 sm:justify-end">
                  <Badge variant="secondary">{t("portal.providers.tier")} {source.tier}</Badge>
                  <Badge variant="secondary">{t("portal.providers.weight")} {source.weight}</Badge>
                  <Switch checked={source.enabled} onChange={(next) => actions.onToggleSource(model, source, next)} label={t("portal.providers.enabled")} />
                  <Button size="icon-sm" variant="ghost" onClick={() => actions.onEditSource(model, source)} aria-label={t("portal.providers.editSource")}><PencilIcon aria-hidden /></Button>
                  <DeleteButton title={t("portal.providers.deleteSource")} description={source.upstream_model} onConfirm={() => actions.onDeleteSource(model, source)} />
                </div>
              </div>
            ))}
          </div>
        </CardContent>
      ) : null}
    </Card>
  )
}
