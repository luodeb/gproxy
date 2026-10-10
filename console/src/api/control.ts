import type { ErrorEnvelope } from "@/generated/ErrorEnvelope"
import { ApiError, json } from "@/api/client"

export type ChannelFieldControl = "text" | "secret" | "url" | "integer" | "boolean" | "string_list" | "select"

export type ChannelFieldDto = {
  key: string
  i18n_key: string
  control: ChannelFieldControl
  required: boolean
  advanced: boolean
  default_value: string | null
  options: Array<string>
}

export type ChannelDto = {
  id: string
  display_name: string
  provider_fields: Array<ChannelFieldDto>
  credential_fields: Array<ChannelFieldDto>
}

export type ProviderDto = {
  id: number
  name: string
  label: string | null
  channel: string
  settings: Record<string, unknown>
  traffic_policy: unknown | null
  credential_strategy: string
  proxy_url: string | null
  tls_fingerprint: unknown | null
  invalid_tls_fingerprint: unknown | null
  tls_fingerprint_error: string | null
  enabled: boolean
}

export type ProviderWriteRequest = {
  name: string
  label: string | null
  channel: string
  settings: Record<string, unknown>
  traffic_policy: unknown | null
  credential_strategy: string
  proxy_url: string | null
  tls_fingerprint: unknown | null
  enabled: boolean
}

export type CredentialHealth = "disabled" | "unknown" | "healthy" | "degraded" | "dead"

export type CredentialModelHealthDto = {
  model: string
  health: CredentialHealth
  observed_at: number
  response_status: number | null
  detail: string | null
}

export type CredentialDto = {
  id: number
  provider_id: number
  label: string | null
  kind: string
  version: number
  enabled: boolean
  weight: number
  rpm_limit: number | null
  tpm_limit: number | null
  proxy_url: string | null
  tls_fingerprint: unknown | null
  health: CredentialHealth
  health_observed_at: number | null
  health_response_status: number | null
  health_detail: string | null
  model_health: Array<CredentialModelHealthDto>
}

export type CredentialWriteRequest = {
  provider_id: number
  label: string | null
  kind: string
  secret: Record<string, unknown> | null
  quota_secret: Record<string, unknown> | null
  enabled: boolean
  weight: number
  rpm_limit: number | null
  tpm_limit: number | null
  proxy_url: string | null
  tls_fingerprint: unknown | null
}

export type ProviderModelDto = {
  id: number
  provider_id: number
  model_id: string
  display_name: string | null
  variants: unknown | null
  context_window: number | null
  max_output_tokens: number | null
  thinking_supported: boolean | null
  thinking_adaptive_supported: boolean | null
  thinking_enabled_supported: boolean | null
  metadata: Record<string, unknown>
  enabled: boolean
}

export type ProviderModelWriteRequest = {
  provider_id: number
  model_id: string
  display_name: string | null
  variants: unknown | null
  context_window: number | null
  max_output_tokens: number | null
  thinking_supported: boolean | null
  thinking_adaptive_supported: boolean | null
  thinking_enabled_supported: boolean | null
  metadata: Record<string, unknown>
  enabled: boolean
}

export type ModelSourceDto = {
  id: number
  provider_id: number
  upstream_model: string
  tier: number
  weight: number
  enabled: boolean
}

export type ModelSourceWriteRequest = {
  provider_id: number
  upstream_model: string
  tier: number
  weight: number
  enabled: boolean
}

export type ManagedModelDto = {
  id: number
  name: string
  display_name: string | null
  enabled: boolean
  sources: Array<ModelSourceDto>
}

export type ManagedModelWriteRequest = {
  name: string
  display_name: string | null
  enabled: boolean
}

export type DiscoveredModelDto = {
  model_id: string
  display_name: string | null
  context_window: number | null
  max_output_tokens: number | null
  thinking_supported: boolean | null
  thinking_adaptive_supported: boolean | null
  thinking_enabled_supported: boolean | null
  metadata: Record<string, unknown>
  default_price_available: boolean
  known: boolean
}

export type ModelDiscoverResponse = {
  ok: boolean
  status: number
  latency_ms: number
  key_prefix: string
  models: Array<DiscoveredModelDto>
  message: string | null
}

async function api<T>(path: string, signal?: AbortSignal, init?: RequestInit): Promise<T> {
  const response = await fetch(path, {
    ...init,
    cache: "no-store",
    credentials: "same-origin",
    signal,
    headers: {
      ...Object.fromEntries(new Headers(init?.headers).entries()),
      accept: "application/json",
    },
  })
  if (!response.ok) {
    const body = await response.json().catch(() => null) as ErrorEnvelope | null
    throw new ApiError(response.status, body?.error.message ?? response.statusText)
  }
  if (response.status === 204) return undefined as T
  return response.json() as Promise<T>
}

export const listChannels = (signal?: AbortSignal) => api<Array<ChannelDto>>("/portal/api/channels", signal)

export const listProviders = (signal?: AbortSignal) => api<Array<ProviderDto>>("/portal/api/providers", signal)
export const createProvider = (value: ProviderWriteRequest) => api<ProviderDto>("/portal/api/providers", undefined, json("POST", value))
export const updateProvider = (id: number, value: ProviderWriteRequest) => api<ProviderDto>(`/portal/api/providers/${id}`, undefined, json("PATCH", value))
export const deleteProvider = (id: number) => api<void>(`/portal/api/providers/${id}`, undefined, { method: "DELETE" })
export const discoverProviderModels = (id: number, signal?: AbortSignal) => api<ModelDiscoverResponse>(`/portal/api/providers/${id}/discover`, signal, json("POST", {}))

/** Per-provider wire-protocol handling for the OpenAI Responses API. */
export type ProviderProtocolDto = { translate_responses: boolean }

export const providerProtocol = (id: number, signal?: AbortSignal) =>
  api<ProviderProtocolDto>(`/portal/api/providers/${id}/protocol`, signal)

export const setProviderProtocol = (id: number, translate_responses: boolean) =>
  api<ProviderProtocolDto>(`/portal/api/providers/${id}/protocol`, undefined, json("PUT", { translate_responses }))

export type ModelTestResult = {
  ok: boolean
  status: number
  latency_ms: number
  ttft_ms: number | null
  output_tokens: number
  tokens_per_second: number | null
  key_prefix: string
  reply: string | null
  message: string | null
}

export const testProviderModel = (providerId: number, modelId: string) =>
  api<ModelTestResult>(`/portal/api/providers/${providerId}/model-test`, undefined, json("POST", { provider_id: providerId, model_id: modelId }))

export const listCredentials = (signal?: AbortSignal) => api<Array<CredentialDto>>("/portal/api/credentials", signal)
export const createCredential = (value: CredentialWriteRequest) => api<CredentialDto>("/portal/api/credentials", undefined, json("POST", value))
export const updateCredential = (id: number, value: CredentialWriteRequest) => api<CredentialDto>(`/portal/api/credentials/${id}`, undefined, json("PATCH", value))
export const deleteCredential = (id: number) => api<void>(`/portal/api/credentials/${id}`, undefined, { method: "DELETE" })
export const revealCredential = (id: number) => api<{ secret: Record<string, unknown> }>(`/portal/api/credentials/${id}/reveal`, undefined, json("POST", {}))
export const resetCredentialHealth = (id: number) => api<void>(`/portal/api/credentials/${id}/health-reset`, undefined, json("POST", {}))

export const listProviderModels = (signal?: AbortSignal) => api<Array<ProviderModelDto>>("/portal/api/provider-models", signal)
export const createProviderModel = (value: ProviderModelWriteRequest) => api<ProviderModelDto>("/portal/api/provider-models", undefined, json("POST", value))
export const updateProviderModel = (id: number, value: ProviderModelWriteRequest) => api<ProviderModelDto>(`/portal/api/provider-models/${id}`, undefined, json("PATCH", value))
export const deleteProviderModel = (id: number) => api<void>(`/portal/api/provider-models/${id}`, undefined, { method: "DELETE" })

export const listManagedModels = (signal?: AbortSignal) => api<Array<ManagedModelDto>>("/portal/api/managed-models", signal)
export const createManagedModel = (value: ManagedModelWriteRequest) => api<ManagedModelDto>("/portal/api/managed-models", undefined, json("POST", value))
export const updateManagedModel = (id: number, value: ManagedModelWriteRequest) => api<ManagedModelDto>(`/portal/api/managed-models/${id}`, undefined, json("PATCH", value))
export const deleteManagedModel = (id: number) => api<void>(`/portal/api/managed-models/${id}`, undefined, { method: "DELETE" })

export const createModelSource = (modelId: number, value: ModelSourceWriteRequest) => api<ModelSourceDto>(`/portal/api/managed-models/${modelId}/sources`, undefined, json("POST", value))
export const updateModelSource = (modelId: number, sourceId: number, value: ModelSourceWriteRequest) => api<ModelSourceDto>(`/portal/api/managed-models/${modelId}/sources/${sourceId}`, undefined, json("PATCH", value))
export const deleteModelSource = (modelId: number, sourceId: number) => api<void>(`/portal/api/managed-models/${modelId}/sources/${sourceId}`, undefined, { method: "DELETE" })
