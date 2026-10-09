import { useState } from "react"
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import { useTranslation } from "react-i18next"
import { ChevronDownIcon, ChevronRightIcon, PencilIcon, PlusIcon, SearchIcon } from "lucide-react"
import {
  createPortalMcpServer, deletePortalMcpServer, discoverPortalMcpServer,
  portalMcpServers, updatePortalMcpServer,
} from "@/api/portal"
import type { PortalMcpServerDto } from "@/generated/PortalMcpServerDto"
import type { PortalMcpServerWriteRequest } from "@/generated/PortalMcpServerWriteRequest"
import { DeleteButton } from "@/components/portal/providers/delete-button"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Dialog, DialogBody, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { formatInstant } from "@/lib/format"
import { cn } from "@/lib/utils"

const emptyDraft: PortalMcpServerWriteRequest = {
  name: "",
  url: "",
  auth_kind: "bearer",
  auth_header_name: null,
  token: null,
  enabled: true,
}

/// The upstream MCP servers gproxy aggregates, and the credential each uses.
/// Tools come from the last discovery — the card shows how old that is, because
/// an upstream that renamed its tools only shows up here.
export function McpServers() {
  const { t, i18n } = useTranslation()
  const queryClient = useQueryClient()
  const servers = useQuery({ queryKey: ["portal", "mcp-servers"], queryFn: ({ signal }) => portalMcpServers(signal) })
  const [editing, setEditing] = useState<{ server: PortalMcpServerDto | null } | null>(null)
  const [expanded, setExpanded] = useState<number | null>(null)
  const [draft, setDraft] = useState<PortalMcpServerWriteRequest>(emptyDraft)
  const [discoverResult, setDiscoverResult] = useState<{ name: string; tools: number; message: string | null } | null>(null)

  const invalidate = () => queryClient.invalidateQueries({ queryKey: ["portal", "mcp-servers"] })
  const save = useMutation({
    mutationFn: () => editing?.server
      ? updatePortalMcpServer(editing.server.id, draft)
      : createPortalMcpServer(draft),
    onSuccess: async () => { await invalidate(); setEditing(null) },
  })
  const remove = useMutation({ mutationFn: deletePortalMcpServer, onSuccess: invalidate })
  const discover = useMutation({
    mutationFn: (server: PortalMcpServerDto) => discoverPortalMcpServer(server.id),
    onSuccess: async (result, server) => {
      await invalidate()
      setDiscoverResult({ name: server.name, tools: result.tools.length, message: result.message })
    },
  })

  const openEditor = (server: PortalMcpServerDto | null) => {
    setDraft(server
      ? { name: server.name, url: server.url, auth_kind: server.auth_kind, auth_header_name: server.auth_header_name, token: null, enabled: server.enabled }
      : emptyDraft)
    setEditing({ server })
  }

  return (
    <>
      <div className="mt-4 flex flex-col gap-3">
        <div className="flex items-center justify-between gap-3">
          <p className="max-w-3xl text-sm text-muted-foreground">{t("portal.mcp.description")}</p>
          <Button onClick={() => openEditor(null)}><PlusIcon aria-hidden />{t("portal.mcp.addServer")}</Button>
        </div>
        {servers.isLoading ? <p className="text-sm text-muted-foreground">{t("portal.common.loading")}</p> : null}
        {servers.isError ? <p className="text-sm text-destructive">{t("portal.mcp.loadError")}</p> : null}
        {discoverResult ? (
          <p className={discoverResult.message ? "text-sm text-destructive" : "text-sm text-muted-foreground"}>
            {discoverResult.message ?? t("portal.mcp.discovered", { name: discoverResult.name, count: discoverResult.tools })}
          </p>
        ) : null}
        {(servers.data ?? []).length === 0 && !servers.isLoading ? <p className="text-sm text-muted-foreground">{t("portal.mcp.empty")}</p> : null}
        {(servers.data ?? []).map((server) => {
          const open = expanded === server.id
          return (
            <div key={server.id} className="flex flex-col gap-2 rounded-md border px-3 py-2">
              <div className="flex flex-wrap items-center justify-between gap-3">
                <button
                  type="button"
                  className="flex min-w-0 flex-1 items-center gap-2 text-left"
                  aria-expanded={open}
                  aria-label={t("portal.mcp.showTools", { name: server.name })}
                  onClick={() => setExpanded(open ? null : server.id)}
                >
                  {open ? <ChevronDownIcon className="size-4 shrink-0" aria-hidden /> : <ChevronRightIcon className="size-4 shrink-0" aria-hidden />}
                  <span className="flex min-w-0 flex-col gap-0.5">
                    <span className="flex min-w-0 flex-wrap items-center gap-x-2 gap-y-0.5">
                      <span className="truncate font-medium">{server.name}</span>
                      <Badge variant="secondary">{server.auth_kind}</Badge>
                      {server.enabled ? null : <Badge variant="outline">{t("portal.common.disabled")}</Badge>}
                      <Badge variant={server.tools.length ? "outline" : "destructive"}>{t("portal.mcp.toolsCount", { count: server.tools.length })}</Badge>
                    </span>
                    <span className="truncate font-mono text-xs text-muted-foreground">{server.url}</span>
                    <span className="text-xs text-muted-foreground">
                      {server.tools_fetched_at ? t("portal.mcp.fetchedAt", { time: formatInstant(server.tools_fetched_at / 1000, i18n.language) }) : t("portal.mcp.neverFetched")}
                    </span>
                  </span>
                </button>
                <div className="flex items-center gap-1">
                  <Button size="sm" variant="outline" disabled={discover.isPending} onClick={() => discover.mutate(server)}>
                    <SearchIcon aria-hidden />{t("portal.mcp.discover")}
                  </Button>
                  <Button size="icon-sm" variant="ghost" aria-label={t("portal.mcp.editServer")} onClick={() => openEditor(server)}><PencilIcon aria-hidden /></Button>
                  <DeleteButton
                    title={t("portal.mcp.deleteServer")}
                    description={t("portal.mcp.deleteServerBody", { name: server.name })}
                    onConfirm={() => remove.mutate(server.id)}
                  />
                </div>
              </div>
              {open ? (
                <div className={cn("flex flex-wrap items-center gap-1 rounded-md border border-dashed px-3 py-2", server.tools.length === 0 && "text-sm text-muted-foreground")}>
                  {server.tools.length === 0
                    ? t("portal.mcp.noTools")
                    : server.tools.map((tool) => (
                      <span key={tool} className="rounded bg-muted px-1.5 py-0.5 font-mono text-xs">{tool}</span>
                    ))}
                </div>
              ) : null}
            </div>
          )
        })}
      </div>

      <Dialog open={editing != null} onOpenChange={(open) => { if (!open) setEditing(null) }}>
        <DialogContent closeLabel={t("portal.common.close")}>
          <DialogHeader>
            <DialogTitle>{editing?.server ? t("portal.mcp.editServer") : t("portal.mcp.addServer")}</DialogTitle>
          </DialogHeader>
          <DialogBody className="flex flex-col gap-4">
            <div className="flex flex-col gap-2">
              <Label htmlFor="mcp-name">{t("portal.mcp.name")}</Label>
              <Input id="mcp-name" value={draft.name} onChange={(event) => setDraft({ ...draft, name: event.target.value })} />
            </div>
            <div className="flex flex-col gap-2">
              <Label htmlFor="mcp-url">{t("portal.mcp.url")}</Label>
              <Input id="mcp-url" className="font-mono text-xs" value={draft.url} onChange={(event) => setDraft({ ...draft, url: event.target.value })} />
            </div>
            <div className="flex flex-col gap-2">
              <Label htmlFor="mcp-auth">{t("portal.mcp.authKind")}</Label>
              <select id="mcp-auth" className="h-9 rounded-md border bg-background px-3 text-sm" value={draft.auth_kind} onChange={(event) => setDraft({ ...draft, auth_kind: event.target.value })}>
                <option value="bearer">{t("portal.mcp.authBearer")}</option>
                <option value="header">{t("portal.mcp.authHeader")}</option>
                <option value="none">{t("portal.mcp.authNone")}</option>
              </select>
            </div>
            {draft.auth_kind === "header" ? (
              <div className="flex flex-col gap-2">
                <Label htmlFor="mcp-header">{t("portal.mcp.headerName")}</Label>
                <Input id="mcp-header" className="font-mono text-xs" value={draft.auth_header_name ?? ""} onChange={(event) => setDraft({ ...draft, auth_header_name: event.target.value || null })} />
              </div>
            ) : null}
            {draft.auth_kind === "none" ? null : (
              <div className="flex flex-col gap-2">
                <Label htmlFor="mcp-token">{t("portal.mcp.token")}</Label>
                <Input id="mcp-token" type="password" value={draft.token ?? ""} placeholder={editing?.server ? t("portal.mcp.tokenKeep") : ""} onChange={(event) => setDraft({ ...draft, token: event.target.value || null })} />
              </div>
            )}
          </DialogBody>
          <DialogFooter>
            <Button variant="outline" onClick={() => setEditing(null)}>{t("portal.common.cancel")}</Button>
            <Button disabled={save.isPending || !draft.name || !draft.url} onClick={() => save.mutate()}>{t("portal.common.save")}</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  )
}
