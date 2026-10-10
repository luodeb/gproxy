import { useState } from "react"
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import { useTranslation } from "react-i18next"
import { ChevronDownIcon, ChevronRightIcon, EyeIcon, GaugeIcon, KeyRoundIcon, PencilIcon, PlusIcon, RotateCcwIcon, SearchIcon } from "lucide-react"
import type { ChannelDto, CredentialDto, ProviderDto, ProviderModelDto } from "@/api/control"
import { providerProtocol, setProviderProtocol } from "@/api/control"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardHeader } from "@/components/ui/card"
import { DeleteButton } from "@/components/portal/providers/delete-button"
import { Switch } from "@/components/portal/providers/switch"
import { cn } from "@/lib/utils"

export type ProviderActions = {
  onEdit: (provider: ProviderDto) => void
  onToggle: (provider: ProviderDto, next: boolean) => void
  onDelete: (provider: ProviderDto) => void
  onAddCredential: (provider: ProviderDto) => void
  onEditCredential: (provider: ProviderDto, credential: CredentialDto) => void
  onToggleCredential: (credential: CredentialDto, next: boolean) => void
  onDeleteCredential: (credential: CredentialDto) => void
  onReveal: (credential: CredentialDto) => Promise<string>
  onResetHealth: (credential: CredentialDto) => void
  onAddModel: (provider: ProviderDto) => void
  onDiscover: (provider: ProviderDto) => void
  onToggleExpose: (provider: ProviderDto, next: boolean) => void
  onTest: (provider: ProviderDto, model: ProviderModelDto) => void
  onEditModel: (provider: ProviderDto, model: ProviderModelDto) => void
  onDeleteModel: (model: ProviderModelDto) => void
}

export function ProviderCard({ provider, channel, credentials, models, actions }: {
  provider: ProviderDto
  channel: ChannelDto | undefined
  credentials: Array<CredentialDto>
  models: Array<ProviderModelDto>
  actions: ProviderActions
}) {
  const { t } = useTranslation()
  const [open, setOpen] = useState(false)
  const [revealed, setRevealed] = useState<Record<number, string>>({})
  const url = providerUrl(provider, channel)

  const reveal = async (credential: CredentialDto) => {
    const value = await actions.onReveal(credential)
    setRevealed((current) => ({ ...current, [credential.id]: value }))
  }

  return (
    <Card>
      <CardHeader className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <button type="button" className="flex min-w-0 flex-1 items-center gap-2 text-left" onClick={() => setOpen((value) => !value)} aria-expanded={open}>
          {open ? <ChevronDownIcon className="size-4 shrink-0" /> : <ChevronRightIcon className="size-4 shrink-0" />}
          <span className="flex min-w-0 flex-1 flex-wrap items-center gap-x-2 gap-y-0.5">
            <span className="truncate font-medium">{provider.label ?? provider.name}</span>
            <Badge variant="secondary">{channel?.display_name ?? provider.channel}</Badge>
            {url ? <span className="hidden truncate font-mono text-xs text-muted-foreground md:inline">{url}</span> : null}
            <Badge variant={credentials.length ? "outline" : "destructive"}>{t("portal.providers.keyCount", { count: credentials.length })}</Badge>
          </span>
        </button>
        <div className="flex items-center gap-2">
          <Switch checked={provider.enabled} onChange={(next) => actions.onToggle(provider, next)} label={t("portal.providers.enabled")} />
          <Button size="icon-sm" variant="ghost" onClick={() => actions.onEdit(provider)} aria-label={t("portal.providers.editProvider")}><PencilIcon aria-hidden /></Button>
          <DeleteButton title={t("portal.providers.deleteProvider")} description={t("portal.providers.deleteProviderBody", { name: provider.label ?? provider.name })} onConfirm={() => actions.onDelete(provider)} />
        </div>
      </CardHeader>
      {open ? (
        <CardContent className="flex flex-col gap-6">
          <ProtocolSection providerId={provider.id} />
          <section className="flex flex-col gap-2">
            <div className="flex items-center justify-between">
              <h3 className="text-sm font-medium">{t("portal.providers.keys")}</h3>
              <Button size="sm" variant="outline" onClick={() => actions.onAddCredential(provider)}><PlusIcon aria-hidden />{t("portal.providers.addKey")}</Button>
            </div>
            {credentials.length === 0 ? <p className="text-sm text-muted-foreground">{t("portal.providers.noKeys")}</p> : null}
            <div className="flex flex-col gap-2">
              {credentials.map((credential) => (
                <div key={credential.id} className="flex flex-col gap-2 rounded-md border px-3 py-2">
                  <div className="flex flex-wrap items-center gap-x-3 gap-y-1">
                    <span className="flex min-w-0 items-center gap-2">
                      <KeyRoundIcon className="size-4 shrink-0 text-muted-foreground" aria-hidden />
                      <span className="truncate">{credential.label ?? t("portal.providers.unnamedKey")}</span>
                    </span>
                    <span className="font-mono text-xs text-muted-foreground">{credential.kind}</span>
                    <Badge variant="secondary">{t("portal.providers.weight")} {credential.weight}</Badge>
                    {credential.rpm_limit != null ? <span className="font-mono text-xs text-muted-foreground">{credential.rpm_limit} rpm</span> : null}
                    <HealthBadge health={credential.health} />
                  </div>
                  <div className="flex items-center gap-1 sm:justify-end">
                    <Switch checked={credential.enabled} onChange={(next) => actions.onToggleCredential(credential, next)} label={t("portal.providers.enabled")} />
                    <Button size="icon-sm" variant="ghost" onClick={() => void reveal(credential)} aria-label={t("portal.providers.reveal")}><EyeIcon aria-hidden /></Button>
                    <Button size="icon-sm" variant="ghost" onClick={() => actions.onResetHealth(credential)} aria-label={t("portal.providers.resetHealth")}><RotateCcwIcon aria-hidden /></Button>
                    <Button size="icon-sm" variant="ghost" onClick={() => actions.onEditCredential(provider, credential)} aria-label={t("portal.providers.editKey")}><PencilIcon aria-hidden /></Button>
                    <DeleteButton title={t("portal.providers.deleteKey")} description={t("portal.providers.deleteKeyBody")} onConfirm={() => actions.onDeleteCredential(credential)} />
                  </div>
                  {revealed[credential.id] ? <code className="break-all rounded bg-muted px-2 py-1 text-xs">{revealed[credential.id]}</code> : null}
                </div>
              ))}
            </div>
          </section>
          <section className="flex flex-col gap-2">
            <div className="flex flex-wrap items-center justify-between gap-2">
              <h3 className="text-sm font-medium">{t("portal.providers.models")}</h3>
              <div className="flex items-center gap-3">
                <label className="flex items-center gap-2 text-xs text-muted-foreground">{t("portal.providers.exposeModels")}<Switch checked={provider.settings.expose_provider_models !== false} onChange={(next) => actions.onToggleExpose(provider, next)} /></label>
                <Button size="sm" variant="outline" onClick={() => actions.onDiscover(provider)}><SearchIcon aria-hidden />{t("portal.providers.discover")}</Button>
                <Button size="sm" variant="outline" onClick={() => actions.onAddModel(provider)}><PlusIcon aria-hidden />{t("portal.providers.addUpstreamModel")}</Button>
              </div>
            </div>
            {models.length === 0 ? <p className="text-sm text-muted-foreground">{t("portal.providers.noUpstreamModels")}</p> : null}
            <div className="flex flex-col gap-2">
              {models.map((model) => (
                <div key={model.id} className={cn("flex flex-wrap items-center justify-between gap-2 rounded-md border px-3 py-2", !model.enabled && "opacity-60")}>
                  <span className="flex min-w-0 flex-wrap items-center gap-x-2 gap-y-0.5">
                    <span className="font-mono text-xs">{model.model_id}</span>
                    {model.display_name ? <span className="text-xs text-muted-foreground">{model.display_name}</span> : null}
                  </span>
                  <span className="flex items-center gap-1">
                    <Button size="sm" variant="outline" onClick={() => actions.onTest(provider, model)}><GaugeIcon aria-hidden />{t("portal.providers.test")}</Button>
                    <Button size="icon-sm" variant="ghost" onClick={() => actions.onEditModel(provider, model)} aria-label={t("portal.providers.editUpstreamModel")}><PencilIcon aria-hidden /></Button>
                    <DeleteButton title={t("portal.providers.deleteUpstreamModel")} description={model.model_id} onConfirm={() => actions.onDeleteModel(model)} />
                  </span>
                </div>
              ))}
            </div>
          </section>
        </CardContent>
      ) : null}
    </Card>
  )
}

/// Per-provider Responses handling: passthrough to upstreams that serve
/// `/v1/responses` themselves, or translate to Chat for the many
/// OpenAI-compatible upstreams that do not (Zhipu's coding endpoint among
/// them) — the case that otherwise fails upstream with a bare 404.
function ProtocolSection({ providerId }: { providerId: number }) {
  const { t } = useTranslation()
  const queryClient = useQueryClient()
  const protocol = useQuery({
    queryKey: ["portal", "provider-protocol", providerId],
    queryFn: ({ signal }) => providerProtocol(providerId, signal),
  })
  const save = useMutation({
    mutationFn: (translate: boolean) => setProviderProtocol(providerId, translate),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["portal", "provider-protocol", providerId] }),
  })
  return (
    <section className="flex flex-col gap-2">
      <h3 className="text-sm font-medium">{t("portal.providers.protocolTitle")}</h3>
      <div className="flex flex-wrap items-center justify-between gap-2 rounded-md border px-3 py-2">
        <span className="flex min-w-0 flex-col gap-0.5">
          <span className="text-sm">{t("portal.providers.translateLabel")}</span>
          <span className="text-xs text-muted-foreground">{t("portal.providers.translateHint")}</span>
        </span>
        <Switch
          checked={protocol.data?.translate_responses ?? false}
          disabled={protocol.isLoading || save.isPending}
          onChange={(next) => save.mutate(next)}
          label={t("portal.providers.translateLabel")}
        />
      </div>
    </section>
  )
}

function HealthBadge({ health }: { health: CredentialDto["health"] }) {
  const { t } = useTranslation()
  const label = health === "healthy" ? t("portal.health.healthy")
    : health === "degraded" ? t("portal.health.degraded")
    : health === "dead" ? t("portal.health.dead")
    : health === "disabled" ? t("portal.health.disabled")
    : t("portal.health.unknown")
  return <Badge variant={health === "healthy" ? "default" : health === "dead" ? "destructive" : "secondary"}>{label}</Badge>
}

function providerUrl(provider: ProviderDto, channel: ChannelDto | undefined): string {
  const urlField = channel?.provider_fields.find((field) => field.control === "url")
  if (urlField) {
    const value = provider.settings[urlField.key]
    if (typeof value === "string") return value
  }
  const base = provider.settings.base_url
  return typeof base === "string" ? base : ""
}
