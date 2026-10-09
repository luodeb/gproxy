import { useMemo, useState } from "react"
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import { useTranslation } from "react-i18next"
import { PlusIcon, SearchIcon } from "lucide-react"
import { toast } from "sonner"
import * as control from "@/api/control"
import type { CredentialDto, CredentialWriteRequest, DiscoveredModelDto, ManagedModelDto, ManagedModelWriteRequest, ModelSourceDto, ModelSourceWriteRequest, ProviderDto, ProviderModelDto, ProviderModelWriteRequest, ProviderWriteRequest } from "@/api/control"
import { QueryState } from "@/components/query-state"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { ProviderCard, type ProviderActions } from "@/components/portal/providers/provider-card"
import { ModelCard, type ModelActions } from "@/components/portal/providers/model-card"
import { ProviderDialog } from "@/components/portal/providers/provider-dialog"
import { CredentialDialog } from "@/components/portal/providers/credential-dialog"
import { ProviderModelDialog } from "@/components/portal/providers/provider-model-dialog"
import { ModelDialog } from "@/components/portal/providers/model-dialog"
import { SourceDialog } from "@/components/portal/providers/source-dialog"
import { DiscoverDialog } from "@/components/portal/providers/discover-dialog"
import { ModelTestDialog } from "@/components/portal/providers/model-test-dialog"
import { McpServers } from "@/components/portal/mcp-servers"
import type { NewModelSource } from "@/components/portal/providers/model-dialog"

type Editor =
  | { kind: "provider"; provider: ProviderDto | null }
  | { kind: "credential"; provider: ProviderDto; credential: CredentialDto | null }
  | { kind: "upstream"; provider: ProviderDto; model: ProviderModelDto | null }
  | { kind: "discover"; provider: ProviderDto }
  | { kind: "model"; model: ManagedModelDto | null }
  | { kind: "source"; model: ManagedModelDto; source: ModelSourceDto | null }
  | null

function editorKey(editor: Editor): string {
  if (!editor) return "closed"
  switch (editor.kind) {
    case "provider": return `provider:${editor.provider?.id ?? "new"}`
    case "credential": return `credential:${editor.credential?.id ?? "new"}:${editor.provider.id}`
    case "upstream": return `upstream:${editor.model?.id ?? "new"}:${editor.provider.id}`
    case "discover": return `discover:${editor.provider.id}`
    case "model": return `model:${editor.model?.id ?? "new"}`
    case "source": return `source:${editor.source?.id ?? "new"}:${editor.model.id}`
  }
}

export function PortalProvidersPage() {
  const { t } = useTranslation()
  const qc = useQueryClient()
  const [tab, setTab] = useState<"providers" | "models" | "mcp">("providers")
  const [search, setSearch] = useState("")
  const [editor, setEditor] = useState<Editor>(null)
  const [testTarget, setTestTarget] = useState<{ provider: ProviderDto; model: ProviderModelDto } | null>(null)

  const channels = useQuery({ queryKey: ["portal", "channels"], queryFn: ({ signal }) => control.listChannels(signal), staleTime: Infinity })
  const providers = useQuery({ queryKey: ["portal", "providers"], queryFn: ({ signal }) => control.listProviders(signal) })
  const credentials = useQuery({ queryKey: ["portal", "credentials"], queryFn: ({ signal }) => control.listCredentials(signal) })
  const providerModels = useQuery({ queryKey: ["portal", "provider-models"], queryFn: ({ signal }) => control.listProviderModels(signal) })
  const managedModels = useQuery({ queryKey: ["portal", "managed-models"], queryFn: ({ signal }) => control.listManagedModels(signal) })

  const invalidate = () => {
    for (const key of ["providers", "credentials", "provider-models", "managed-models"]) qc.invalidateQueries({ queryKey: ["portal", key] })
    qc.invalidateQueries({ queryKey: ["portal", "models"] })
  }
  const fail = (error: unknown) => toast.error(error instanceof Error ? error.message : String(error))
  const saved = () => { invalidate(); toast.success(t("portal.common.saved")) }
  const removed = () => { invalidate(); toast.success(t("portal.common.deleted")) }

  const channelOf = (provider: ProviderDto | null) => channels.data?.find((channel) => channel.id === provider?.channel)

  const saveProvider = useMutation({ mutationFn: ({ id, value }: { id: number | null; value: ProviderWriteRequest }) => id == null ? control.createProvider(value) : control.updateProvider(id, value), onSuccess: () => { saved(); setEditor(null) }, onError: fail })
  const toggleProvider = useMutation({ mutationFn: ({ provider, next }: { provider: ProviderDto; next: boolean }) => control.updateProvider(provider.id, providerWrite(provider, { enabled: next })), onSuccess: invalidate, onError: fail })
  const toggleExpose = useMutation({ mutationFn: ({ provider, next }: { provider: ProviderDto; next: boolean }) => control.updateProvider(provider.id, providerWrite(provider, { settings: { ...provider.settings, expose_provider_models: next } })), onSuccess: invalidate, onError: fail })
  const removeProvider = useMutation({ mutationFn: control.deleteProvider, onSuccess: removed, onError: fail })

  const saveCredential = useMutation({ mutationFn: ({ id, value }: { id: number | null; value: CredentialWriteRequest }) => id == null ? control.createCredential(value) : control.updateCredential(id, value), onSuccess: () => { saved(); setEditor(null) }, onError: fail })
  const toggleCredential = useMutation({ mutationFn: ({ credential, next }: { credential: CredentialDto; next: boolean }) => control.updateCredential(credential.id, credentialWrite(credential, next)), onSuccess: invalidate, onError: fail })
  const removeCredential = useMutation({ mutationFn: control.deleteCredential, onSuccess: removed, onError: fail })
  const resetHealth = useMutation({ mutationFn: control.resetCredentialHealth, onSuccess: () => { invalidate(); toast.success(t("portal.providers.resetHealth")) }, onError: fail })
  const reveal = async (credential: CredentialDto) => {
    try {
      const { secret } = await control.revealCredential(credential.id)
      return JSON.stringify(secret)
    } catch (error) {
      fail(error)
      return ""
    }
  }

  const saveUpstream = useMutation({ mutationFn: ({ id, value }: { id: number | null; value: ProviderModelWriteRequest }) => id == null ? control.createProviderModel(value) : control.updateProviderModel(id, value), onSuccess: () => { saved(); setEditor(null) }, onError: fail })
  const removeUpstream = useMutation({ mutationFn: control.deleteProviderModel, onSuccess: removed, onError: fail })
  const discoverAdd = useMutation({
    mutationFn: async ({ providerId, models }: { providerId: number; models: Array<DiscoveredModelDto> }) => {
      for (const model of models) {
        await control.createProviderModel({
          provider_id: providerId,
          model_id: model.model_id,
          display_name: model.display_name,
          variants: null,
          context_window: model.context_window,
          max_output_tokens: model.max_output_tokens,
          thinking_supported: model.thinking_supported,
          thinking_adaptive_supported: model.thinking_adaptive_supported,
          thinking_enabled_supported: model.thinking_enabled_supported,
          metadata: model.metadata,
          enabled: true,
        })
      }
    },
    onSuccess: () => { saved(); setEditor(null) },
    onError: fail,
  })

  const saveModel = useMutation({
    mutationFn: async ({ id, value, source }: { id: number | null; value: ManagedModelWriteRequest; source: NewModelSource | null }) => {
      if (id != null) return control.updateManagedModel(id, value)
      const created = await control.createManagedModel(value)
      if (source) {
        await control.createModelSource(created.id, { provider_id: source.provider_id, upstream_model: source.upstream_model, tier: 0, weight: 100, enabled: true })
      }
      return created
    },
    onSuccess: () => { saved(); setEditor(null) },
    onError: fail,
  })
  const toggleModel = useMutation({ mutationFn: ({ model, next }: { model: ManagedModelDto; next: boolean }) => control.updateManagedModel(model.id, { name: model.name, display_name: model.display_name, enabled: next }), onSuccess: invalidate, onError: fail })
  const removeModel = useMutation({ mutationFn: control.deleteManagedModel, onSuccess: removed, onError: fail })

  const saveSource = useMutation({ mutationFn: ({ modelId, sourceId, value }: { modelId: number; sourceId: number | null; value: ModelSourceWriteRequest }) => sourceId == null ? control.createModelSource(modelId, value) : control.updateModelSource(modelId, sourceId, value), onSuccess: () => { saved(); setEditor(null) }, onError: fail })
  const toggleSource = useMutation({ mutationFn: ({ model, source, next }: { model: ManagedModelDto; source: ModelSourceDto; next: boolean }) => control.updateModelSource(model.id, source.id, { provider_id: source.provider_id, upstream_model: source.upstream_model, tier: source.tier, weight: source.weight, enabled: next }), onSuccess: invalidate, onError: fail })
  const removeSource = useMutation({ mutationFn: ({ model, source }: { model: ManagedModelDto; source: ModelSourceDto }) => control.deleteModelSource(model.id, source.id), onSuccess: removed, onError: fail })

  const providerActions: ProviderActions = {
    onEdit: (provider) => setEditor({ kind: "provider", provider }),
    onToggle: (provider, next) => toggleProvider.mutate({ provider, next }),
    onDelete: (provider) => removeProvider.mutate(provider.id),
    onAddCredential: (provider) => setEditor({ kind: "credential", provider, credential: null }),
    onEditCredential: (provider, credential) => setEditor({ kind: "credential", provider, credential }),
    onToggleCredential: (credential, next) => toggleCredential.mutate({ credential, next }),
    onDeleteCredential: (credential) => removeCredential.mutate(credential.id),
    onReveal: reveal,
    onResetHealth: (credential) => resetHealth.mutate(credential.id),
    onAddModel: (provider) => setEditor({ kind: "upstream", provider, model: null }),
    onDiscover: (provider) => setEditor({ kind: "discover", provider }),
    onToggleExpose: (provider, next) => toggleExpose.mutate({ provider, next }),
    onTest: (provider, model) => setTestTarget({ provider, model }),
    onEditModel: (provider, model) => setEditor({ kind: "upstream", provider, model }),
    onDeleteModel: (model) => removeUpstream.mutate(model.id),
  }

  const modelActions: ModelActions = {
    onEdit: (model) => setEditor({ kind: "model", model }),
    onToggle: (model, next) => toggleModel.mutate({ model, next }),
    onDelete: (model) => removeModel.mutate(model.id),
    onAddSource: (model) => setEditor({ kind: "source", model, source: null }),
    onEditSource: (model, source) => setEditor({ kind: "source", model, source }),
    onToggleSource: (model, source, next) => toggleSource.mutate({ model, source, next }),
    onDeleteSource: (model, source) => removeSource.mutate({ model, source }),
  }

  const query = search.trim().toLowerCase()
  const visibleProviders = useMemo(() => (providers.data ?? []).filter((provider) => !query || `${provider.name} ${provider.label ?? ""} ${provider.channel}`.toLowerCase().includes(query)), [providers.data, query])
  const visibleModels = useMemo(() => (managedModels.data ?? []).filter((model) => !query || `${model.name} ${model.display_name ?? ""}`.toLowerCase().includes(query)), [managedModels.data, query])

  const loading = providers.isLoading || managedModels.isLoading
  const errored = providers.isError || managedModels.isError

  return (
    <>
      <section className="flex flex-col gap-2">
        <h1 className="text-2xl font-semibold tracking-tight">{t("portal.providers.pageTitle")}</h1>
        <p className="max-w-3xl text-sm leading-6 text-muted-foreground">{t("portal.providers.pageDescription")}</p>
      </section>

      <div className="mt-6 flex flex-wrap items-center justify-between gap-3">
        <div className="flex gap-2">
          <Button variant={tab === "providers" ? "secondary" : "ghost"} onClick={() => setTab("providers")}>{t("portal.providers.tabProviders")}</Button>
          <Button variant={tab === "models" ? "secondary" : "ghost"} onClick={() => setTab("models")}>{t("portal.providers.tabModels")}</Button>
          <Button variant={tab === "mcp" ? "secondary" : "ghost"} onClick={() => setTab("mcp")}>{t("portal.providers.tabMcp")}</Button>
        </div>
        <div className="flex items-center gap-2">
          {tab === "mcp" ? null : (
            <div className="relative">
              <SearchIcon className="pointer-events-none absolute top-1/2 left-2 size-4 -translate-y-1/2 text-muted-foreground" aria-hidden />
              <Input className="pl-8" value={search} onChange={(event) => setSearch(event.target.value)} placeholder={t("portal.providers.search")} />
            </div>
          )}
          {tab === "providers" ? <Button onClick={() => setEditor({ kind: "provider", provider: null })}><PlusIcon aria-hidden />{t("portal.providers.addProvider")}</Button> : null}
          {tab === "models" ? <Button onClick={() => setEditor({ kind: "model", model: null })}><PlusIcon aria-hidden />{t("portal.providers.addModel")}</Button> : null}
        </div>
      </div>

      {tab === "mcp" ? (
        <>
          <h2 className="mt-4 text-lg font-medium">{t("portal.mcp.title")}</h2>
          <McpServers />
        </>
      ) : (
      <QueryState loading={loading} error={errored ? t("portal.providers.loadError") : ""}>
        {tab === "providers" ? (
          <div className="mt-4 flex flex-col gap-4">
            {visibleProviders.length === 0 ? <p className="text-sm text-muted-foreground">{t("portal.providers.emptyProviders")}</p> : null}
            {visibleProviders.map((provider) => (
              <ProviderCard
                key={provider.id}
                provider={provider}
                channel={channelOf(provider)}
                credentials={(credentials.data ?? []).filter((credential) => credential.provider_id === provider.id)}
                models={(providerModels.data ?? []).filter((model) => model.provider_id === provider.id)}
                actions={providerActions}
              />
            ))}
          </div>
        ) : (
          <div className="mt-4 flex flex-col gap-4">
            {visibleModels.length === 0 ? <p className="text-sm text-muted-foreground">{t("portal.providers.emptyModels")}</p> : null}
            {visibleModels.map((model) => (
              <ModelCard key={model.id} model={model} providers={providers.data ?? []} actions={modelActions} />
            ))}
          </div>
        )}
      </QueryState>
      )}

      {editor?.kind === "provider" ? (
        <ProviderDialog key={editorKey(editor)} open onOpenChange={(open) => { if (!open) setEditor(null) }} provider={editor.provider} channels={channels.data ?? []} pending={saveProvider.isPending} onSubmit={(value) => saveProvider.mutate({ id: editor.provider?.id ?? null, value })} />
      ) : null}
      {editor?.kind === "credential" ? (
        <CredentialDialog key={editorKey(editor)} open onOpenChange={(open) => { if (!open) setEditor(null) }} provider={editor.provider} credential={editor.credential} channel={channelOf(editor.provider)} pending={saveCredential.isPending} onSubmit={(value) => saveCredential.mutate({ id: editor.credential?.id ?? null, value })} />
      ) : null}
      {editor?.kind === "upstream" ? (
        <ProviderModelDialog key={editorKey(editor)} open onOpenChange={(open) => { if (!open) setEditor(null) }} providerId={editor.provider.id} model={editor.model} pending={saveUpstream.isPending} onSubmit={(value) => saveUpstream.mutate({ id: editor.model?.id ?? null, value })} />
      ) : null}
      {editor?.kind === "discover" ? (
        <DiscoverDialog key={editorKey(editor)} open onOpenChange={(open) => { if (!open) setEditor(null) }} provider={editor.provider} pending={discoverAdd.isPending} onSubmit={(models) => discoverAdd.mutate({ providerId: editor.provider.id, models })} />
      ) : null}
      {editor?.kind === "model" ? (
        <ModelDialog key={editorKey(editor)} open onOpenChange={(open) => { if (!open) setEditor(null) }} model={editor.model} providers={providers.data ?? []} providerModels={providerModels.data ?? []} pending={saveModel.isPending} onSubmit={(value, source) => saveModel.mutate({ id: editor.model?.id ?? null, value, source })} />
      ) : null}
      {editor?.kind === "source" ? (
        <SourceDialog key={editorKey(editor)} open onOpenChange={(open) => { if (!open) setEditor(null) }} source={editor.source} providers={providers.data ?? []} providerModels={providerModels.data ?? []} pending={saveSource.isPending} onSubmit={(value) => saveSource.mutate({ modelId: editor.model.id, sourceId: editor.source?.id ?? null, value })} />
      ) : null}
      <ModelTestDialog
        open={testTarget != null}
        onOpenChange={(open) => { if (!open) setTestTarget(null) }}
        providerId={testTarget?.provider.id ?? 0}
        providerName={testTarget ? testTarget.provider.label ?? testTarget.provider.name : ""}
        model={testTarget?.model ?? null}
      />
    </>
  )
}

function providerWrite(provider: ProviderDto, patch: Partial<ProviderWriteRequest>): ProviderWriteRequest {
  return {
    name: provider.name,
    label: provider.label,
    channel: provider.channel,
    settings: provider.settings,
    traffic_policy: provider.traffic_policy,
    credential_strategy: provider.credential_strategy,
    proxy_url: provider.proxy_url,
    tls_fingerprint: provider.tls_fingerprint,
    enabled: provider.enabled,
    ...patch,
  }
}

function credentialWrite(credential: CredentialDto, enabled: boolean): CredentialWriteRequest {
  return {
    provider_id: credential.provider_id,
    label: credential.label,
    kind: credential.kind,
    secret: null,
    quota_secret: null,
    enabled,
    weight: credential.weight,
    rpm_limit: credential.rpm_limit,
    tpm_limit: credential.tpm_limit,
    proxy_url: credential.proxy_url,
    tls_fingerprint: credential.tls_fingerprint,
  }
}
