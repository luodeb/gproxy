import { useState } from "react"
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import { useTranslation } from "react-i18next"
import { portalKeyMcp, portalKeyModels, setPortalKeyMcp, setPortalKeyModels } from "@/api/portal"
import type { PortalKeyMcpGrantDto } from "@/generated/PortalKeyMcpGrantDto"
import type { UserKeyDto } from "@/generated/UserKeyDto"
import { KeyMcpEditor } from "@/components/portal/key-mcp-editor"
import { QueryState } from "@/components/query-state"
import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"

/// What one key may reach, shown inside its expanded row: models first, then
/// MCP tools. Both are allow-lists, but they differ in one way the copy states
/// outright — no models checked means "no own rows" (the account's access
/// applies), while no MCP servers checked means this key calls no MCP tools.
export function KeyPermissions({ keyRecord }: { keyRecord: UserKeyDto }) {
  const { t } = useTranslation()
  const queryClient = useQueryClient()
  const models = useQuery({
    queryKey: ["portal", "key-models", keyRecord.id],
    queryFn: ({ signal }) => portalKeyModels(keyRecord.id, signal),
  })
  const mcp = useQuery({
    queryKey: ["portal", "key-mcp", keyRecord.id],
    queryFn: ({ signal }) => portalKeyMcp(keyRecord.id, signal),
  })
  const [pendingModels, setPendingModels] = useState<Set<string> | null>(null)
  const [pendingGrants, setPendingGrants] = useState<Array<PortalKeyMcpGrantDto> | null>(null)
  const selected = pendingModels ?? new Set(models.data?.allowed ?? [])
  const grants = pendingGrants ?? mcp.data?.allowed ?? []
  const toggleModel = (name: string) => setPendingModels(() => {
    const next = new Set(selected)
    if (next.has(name)) next.delete(name)
    else next.add(name)
    return next
  })
  const save = useMutation({
    mutationFn: async () => {
      await setPortalKeyModels(keyRecord.id, [...selected])
      await setPortalKeyMcp(keyRecord.id, grants)
    },
    onSuccess: async () => {
      setPendingModels(null)
      setPendingGrants(null)
      await queryClient.invalidateQueries({ queryKey: ["portal", "key-models", keyRecord.id] })
      await queryClient.invalidateQueries({ queryKey: ["portal", "key-mcp", keyRecord.id] })
    },
  })

  return (
    <div className="flex flex-col gap-6 py-1">
      <section className="flex flex-col gap-2">
        <h3 className="text-sm font-medium">{t("portal.keys.modelsTitle")}</h3>
        <p className="text-xs text-muted-foreground">{t("portal.keys.modelsHint")}</p>
        <QueryState loading={models.isLoading} error={models.isError ? t("portal.keys.modelsError") : ""}>
          <div className="grid gap-1 sm:grid-cols-2 xl:grid-cols-3">
            {(models.data?.available ?? []).map((name) => (
              <label key={name} className="flex items-center gap-3 rounded-md border px-3 py-2">
                <Checkbox checked={selected.has(name)} onCheckedChange={() => toggleModel(name)} />
                <span className="min-w-0 truncate font-mono text-xs">{name}</span>
              </label>
            ))}
          </div>
          {models.data && models.data.available.length === 0 ? <p className="text-sm text-muted-foreground">{t("portal.keys.modelsEmpty")}</p> : null}
        </QueryState>
      </section>

      <section className="flex flex-col gap-2">
        <h3 className="text-sm font-medium">{t("portal.keys.mcpTitle")}</h3>
        <p className="text-xs text-muted-foreground">{t("portal.keys.mcpHint")}</p>
        <QueryState loading={mcp.isLoading} error={mcp.isError ? t("portal.keys.mcpError") : ""}>
          <KeyMcpEditor
            servers={mcp.data?.available ?? []}
            grants={grants}
            onChange={(next) => setPendingGrants(next)}
          />
        </QueryState>
      </section>

      <div className="flex justify-end">
        <Button size="sm" disabled={save.isPending || models.isLoading || mcp.isLoading} onClick={() => save.mutate()}>
          {t("portal.common.save")}
        </Button>
      </div>
    </div>
  )
}
