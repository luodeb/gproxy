import { useState } from "react"
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import { useTranslation } from "react-i18next"
import { createPortalKey, deletePortalKey, portalKeys, revealPortalKey } from "@/api/portal"
import { DataTable, type DataTableColumn } from "@/components/data-table"
import { KeySecretCell } from "@/components/keys/key-secret-cell"
import { KeyPermissions } from "@/components/portal/key-permissions"
import type { UserKeyDto } from "@/generated/UserKeyDto"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Input } from "@/components/ui/input"

export function KeyManagement() {
  const { t } = useTranslation()
  const queryClient = useQueryClient()
  const keys = useQuery({ queryKey: ["portal", "keys"], queryFn: ({ signal }) => portalKeys(signal) })
  const [label, setLabel] = useState("")
  const [prefix, setPrefix] = useState<"sk" | "at">("sk")
  const [created, setCreated] = useState<string | null>(null)
  const [expandedKey, setExpandedKey] = useState<number | null>(null)

  const createKey = useMutation({
    mutationFn: () => createPortalKey({ prefix, label: label.trim() || null, expires_at: null }),
    onSuccess: async (value) => {
      setCreated(value.api_key)
      setLabel("")
      await queryClient.invalidateQueries({ queryKey: ["portal", "keys"] })
    },
  })
  const removeKey = useMutation({
    mutationFn: deletePortalKey,
    onSuccess: async () => queryClient.invalidateQueries({ queryKey: ["portal", "keys"] }),
  })

  const columns: Array<DataTableColumn<UserKeyDto>> = [
    {
      key: "label",
      label: t("portal.keys.columnLabel"),
      header: t("portal.keys.columnLabel"),
      cell: (key) => (
        <div className="min-w-0">
          <p className="truncate text-sm font-medium">{key.label ?? t("portal.keys.unnamed")}</p>
          <KeySecretCell record={key} reveal={() => revealPortalKey(key.id)} />
        </div>
      ),
    },
    {
      key: "actions",
      label: t("portal.keys.columnActions"),
      header: "",
      className: "w-px",
      cell: (key) => (
        <Button
          type="button"
          size="sm"
          variant="outline"
          disabled={removeKey.isPending}
          onClick={(event) => { event.stopPropagation(); removeKey.mutate(key.id) }}
        >
          {t("portal.keys.revoke")}
        </Button>
      ),
    },
  ]

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t("portal.keys.title")}</CardTitle>
        <CardDescription>{t("portal.keys.description")}</CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-4">
        {created ? (
          <Alert>
            <AlertTitle>{t("portal.keys.created")}</AlertTitle>
            <AlertDescription><code className="break-all">{created}</code></AlertDescription>
          </Alert>
        ) : null}
        <form className="flex flex-col gap-3 sm:flex-row" onSubmit={(event) => { event.preventDefault(); createKey.mutate() }}>
          <select className="h-9 rounded-md border bg-background px-3 text-sm" value={prefix} onChange={(event) => setPrefix(event.target.value as "sk" | "at")}>
            <option value="sk">sk</option>
            <option value="at">at</option>
          </select>
          <Input value={label} placeholder={t("portal.keys.label")} onChange={(event) => setLabel(event.target.value)} />
          <Button type="submit" disabled={createKey.isPending}>{t("portal.keys.create")}</Button>
        </form>
        <DataTable<UserKeyDto>
          columns={columns}
          rows={keys.data ?? []}
          rowKey={(key) => key.id}
          searchText={(key) => `${key.label ?? ""} ${key.prefix ?? ""}`}
          renderCard={(key) => (
            <div className="flex min-w-0 flex-col gap-2">
              <p className="truncate text-sm font-medium">{key.label ?? t("portal.keys.unnamed")}</p>
              <KeySecretCell record={key} reveal={() => revealPortalKey(key.id)} />
            </div>
          )}
          renderExpandedRow={(key) => <KeyPermissions keyRecord={key} />}
          activeRowKey={expandedKey}
          onRowClick={(key) => setExpandedKey((current) => (current === key.id ? null : key.id))}
          empty={t("portal.keys.empty")}
          storageKey="portal-keys"
        />
      </CardContent>
    </Card>
  )
}
