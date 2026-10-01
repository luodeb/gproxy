import { useState } from "react"
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import { useTranslation } from "react-i18next"
import { portalKeyModels, setPortalKeyModels } from "@/api/portal"
import type { UserKeyDto } from "@/generated/UserKeyDto"
import { QueryState } from "@/components/query-state"
import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"
import { Dialog, DialogBody, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog"

export function KeyModelsDialog({ keyRecord, onClose }: { keyRecord: UserKeyDto | null; onClose: () => void }) {
  const { t } = useTranslation()
  const queryClient = useQueryClient()
  const open = keyRecord != null
  const models = useQuery({
    queryKey: ["portal", "key-models", keyRecord?.id],
    queryFn: ({ signal }) => portalKeyModels(keyRecord!.id, signal),
    enabled: open,
  })
  const [pending, setPending] = useState<Set<string> | null>(null)
  const selected = pending ?? new Set(models.data?.allowed ?? [])
  const toggle = (name: string) => setPending(() => {
    const next = new Set(selected)
    if (next.has(name)) next.delete(name)
    else next.add(name)
    return next
  })
  const save = useMutation({
    mutationFn: () => setPortalKeyModels(keyRecord!.id, [...selected]),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["portal", "key-models", keyRecord!.id] })
      onClose()
    },
  })

  return (
    <Dialog open={open} onOpenChange={(value) => { if (!value) onClose() }}>
      <DialogContent closeLabel={t("portal.common.close")}>
        <DialogHeader>
          <DialogTitle>{t("portal.keys.modelsTitle")}</DialogTitle>
          <DialogDescription>{keyRecord?.label ?? t("portal.keys.unnamed")} · {t("portal.keys.modelsHint")}</DialogDescription>
        </DialogHeader>
        <DialogBody>
          <QueryState loading={models.isLoading} error={models.isError ? t("portal.keys.modelsError") : ""}>
            <div className="flex flex-col gap-1">
              {(models.data?.available ?? []).map((name) => (
                <label key={name} className="flex items-center gap-3 rounded-md border px-3 py-2">
                  <Checkbox checked={selected.has(name)} onCheckedChange={() => toggle(name)} />
                  <span className="min-w-0 truncate font-mono text-xs">{name}</span>
                </label>
              ))}
              {models.data && models.data.available.length === 0 ? <p className="text-sm text-muted-foreground">{t("portal.keys.modelsEmpty")}</p> : null}
            </div>
          </QueryState>
        </DialogBody>
        <DialogFooter>
          <Button variant="outline" onClick={onClose}>{t("portal.common.cancel")}</Button>
          <Button disabled={save.isPending || models.isLoading} onClick={() => save.mutate()}>{t("portal.common.save")}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
