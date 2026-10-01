import { useState } from "react"
import { useQuery } from "@tanstack/react-query"
import { useTranslation } from "react-i18next"
import type { DiscoveredModelDto, ProviderDto } from "@/api/control"
import { discoverProviderModels } from "@/api/control"
import { Button } from "@/components/ui/button"
import { Badge } from "@/components/ui/badge"
import { Checkbox } from "@/components/ui/checkbox"
import { Dialog, DialogBody, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { QueryState } from "@/components/query-state"

export function DiscoverDialog({ open, onOpenChange, provider, pending, onSubmit }: {
  open: boolean
  onOpenChange: (open: boolean) => void
  provider: ProviderDto | null
  pending: boolean
  onSubmit: (models: Array<DiscoveredModelDto>) => void
}) {
  const { t } = useTranslation()
  const [selected, setSelected] = useState<Set<string>>(new Set())
  const discovery = useQuery({
    queryKey: ["portal", "discover", provider?.id],
    queryFn: ({ signal }) => discoverProviderModels(provider!.id, signal),
    enabled: open && provider != null,
    staleTime: 0,
    retry: 0,
  })

  const models = discovery.data?.models ?? []
  const toggle = (id: string) => setSelected((current) => {
    const next = new Set(current)
    if (next.has(id)) next.delete(id)
    else next.add(id)
    return next
  })
  const chosen = models.filter((model) => selected.has(model.model_id))

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-xl" closeLabel={t("portal.common.close")}>
        <DialogHeader>
          <DialogTitle>{t("portal.providers.discoverTitle")}</DialogTitle>
          <DialogDescription>{provider?.label ?? provider?.name}</DialogDescription>
        </DialogHeader>
        <DialogBody>
          <QueryState loading={discovery.isLoading} error={discovery.isError ? t("portal.providers.discoverError") : ""}>
            {discovery.data && !discovery.data.ok ? <p className="text-sm text-destructive">{discovery.data.message ?? t("portal.providers.discoverError")}</p> : null}
            {discovery.data?.ok && models.length === 0 ? <p className="text-sm text-muted-foreground">{t("portal.providers.discoverEmpty")}</p> : null}
            <div className="flex flex-col gap-1">
              {models.map((model) => (
                <label key={model.model_id} className="flex items-center gap-3 rounded-md border px-3 py-2">
                  <Checkbox checked={selected.has(model.model_id)} disabled={model.known} onCheckedChange={() => toggle(model.model_id)} />
                  <span className="min-w-0 flex-1">
                    <span className="block truncate font-mono text-xs">{model.model_id}</span>
                    {model.display_name ? <span className="block truncate text-xs text-muted-foreground">{model.display_name}</span> : null}
                  </span>
                  {model.known ? <Badge variant="outline">{t("portal.providers.discoverKnown")}</Badge> : null}
                </label>
              ))}
            </div>
          </QueryState>
        </DialogBody>
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>{t("portal.common.cancel")}</Button>
          <Button disabled={pending || chosen.length === 0} onClick={() => onSubmit(chosen)}>{t("portal.providers.discoverAdd", { count: chosen.length })}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
