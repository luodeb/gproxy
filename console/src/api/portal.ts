import type { UserKeyRevealResponse } from "@/generated/UserKeyRevealResponse"
import type { ErrorEnvelope } from "@/generated/ErrorEnvelope"
import type { PortalContextDto } from "@/generated/PortalContextDto"
import type { OAuthSessionPageDto } from "@/generated/OAuthSessionPageDto"
import type { PortalKeyCreateRequest } from "@/generated/PortalKeyCreateRequest"
import type { PortalLoginRequest } from "@/generated/PortalLoginRequest"
import type { PortalPasswordChangeRequest } from "@/generated/PortalPasswordChangeRequest"
import type { PortalSessionStatusDto } from "@/generated/PortalSessionStatusDto"
import type { PortalModelDto } from "@/generated/PortalModelDto"
import type { PortalRecentQueryDto } from "@/generated/PortalRecentQueryDto"
import type { PortalRecentRequestDto } from "@/generated/PortalRecentRequestDto"
import type { UserKeyCreateResponse } from "@/generated/UserKeyCreateResponse"
import type { PortalMcpServerDto } from "@/generated/PortalMcpServerDto"
import type { PortalMcpServerWriteRequest } from "@/generated/PortalMcpServerWriteRequest"
import type { PortalMcpDiscoverResponse } from "@/generated/PortalMcpDiscoverResponse"
import type { PortalKeyMcpDto } from "@/generated/PortalKeyMcpDto"
import type { PortalKeyMcpGrantDto } from "@/generated/PortalKeyMcpGrantDto"
import type { PortalActivityDto } from "@/generated/PortalActivityDto"
import type { UserKeyDto } from "@/generated/UserKeyDto"
import { ApiError, json } from "@/api/client"

async function portalApi<T>(path: string, signal?: AbortSignal, init?: RequestInit): Promise<T> {
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

export const portalLogin = (value: PortalLoginRequest) =>
  portalApi<PortalContextDto>("/portal/api/login", undefined, json("POST", value))

export const portalSession = () => portalApi<PortalSessionStatusDto>("/portal/api/session")

export const portalLogout = () => portalApi<void>("/portal/api/logout", undefined, json("POST", {}))

export const portalChangePassword = (value: PortalPasswordChangeRequest) =>
  portalApi<void>("/portal/api/password", undefined, json("POST", value))


export const portalModels = (signal?: AbortSignal) =>
  portalApi<Array<PortalModelDto>>("/portal/api/models", signal)

export type PortalUsagePointDto = {
  bucket_start: number
  requests: number
  input_tokens: number
  output_tokens: number
  cached_input_tokens: number
  cost: string
}

export type PortalUsageModelDto = {
  model: string
  /** The upstream that served it — same-named models on different providers stay apart. */
  provider: string | null
  requests: number
  input_tokens: number
  output_tokens: number
  cached_input_tokens: number
  cost: string
}

export type PortalUsageModelSeriesDto = {
  model: string
  series: Array<PortalUsagePointDto>
}

export type PortalUsageOverviewDto = {
  from: number
  to: number
  requests: number
  input_tokens: number
  output_tokens: number
  cached_input_tokens: number
  cost: string
  series: Array<PortalUsagePointDto>
  models: Array<PortalUsageModelDto>
  model_series: Array<PortalUsageModelSeriesDto>
}

export function portalUsageOverview(query: { from: number; to: number; user_key_id?: number; model?: string }, signal?: AbortSignal) {
  const params = new URLSearchParams({ from: String(query.from), to: String(query.to) })
  if (query.user_key_id != null) params.set("user_key_id", String(query.user_key_id))
  if (query.model) params.set("model", query.model)
  return portalApi<PortalUsageOverviewDto>(`/portal/api/usage/overview?${params}`, signal)
}

export function portalRecentRequests(query: PortalRecentQueryDto, signal?: AbortSignal) {
  const params = new URLSearchParams()
  if (query.limit != null) params.set("limit", String(query.limit))
  const suffix = params.size === 0 ? "" : `?${params}`
  return portalApi<Array<PortalRecentRequestDto>>(`/portal/api/recent-requests${suffix}`, signal)
}

export const portalKeys = (signal?: AbortSignal) => portalApi<Array<UserKeyDto>>("/portal/api/keys", signal)

export type PortalKeyModelsDto = { available: Array<string>; allowed: Array<string> }

export const portalKeyModels = (id: number, signal?: AbortSignal) =>
  portalApi<PortalKeyModelsDto>(`/portal/api/keys/${id}/models`, signal)

export const setPortalKeyModels = (id: number, models: Array<string>) =>
  portalApi<PortalKeyModelsDto>(`/portal/api/keys/${id}/models`, undefined, json("PUT", { models }))

export const portalOAuthSessions = (activeOnly: boolean, limit: number, offset: number, signal?: AbortSignal) =>
  portalApi<OAuthSessionPageDto>(`/portal/api/oauth-sessions?${new URLSearchParams({ active_only: String(activeOnly), limit: String(limit), offset: String(offset) })}`, signal)

export const revokePortalOAuthSession = (id: number) => portalApi<void>(`/portal/api/oauth-sessions/${id}`, undefined, { method: "DELETE" })

export const createPortalKey = (value: PortalKeyCreateRequest) =>
  portalApi<UserKeyCreateResponse>("/portal/api/keys", undefined, json("POST", value))


export const deletePortalKey = (id: number) =>
  portalApi<void>(`/portal/api/keys/${id}`, undefined, { method: "DELETE" })



export const revealPortalKey = (id: number) =>
  portalApi<UserKeyRevealResponse>(`/portal/api/keys/${id}/reveal`, undefined, json("POST", {}))

/** The MCP servers the operator registered, with their last discovered tools. */
export const portalMcpServers = (signal?: AbortSignal) =>
  portalApi<Array<PortalMcpServerDto>>("/portal/api/mcp-servers", signal)

export const createPortalMcpServer = (value: PortalMcpServerWriteRequest) =>
  portalApi<PortalMcpServerDto>("/portal/api/mcp-servers", undefined, json("POST", value))

export const updatePortalMcpServer = (id: number, value: PortalMcpServerWriteRequest) =>
  portalApi<{ id: number }>(`/portal/api/mcp-servers/${id}`, undefined, json("PATCH", value))

export const deletePortalMcpServer = (id: number) =>
  portalApi<void>(`/portal/api/mcp-servers/${id}`, undefined, { method: "DELETE" })

/** Ask the server what it serves; a failed pull still answers `ok: false`. */
export const discoverPortalMcpServer = (id: number) =>
  portalApi<PortalMcpDiscoverResponse>(`/portal/api/mcp-servers/${id}/discover`, undefined, json("POST", {}))

/** The MCP tools one key may call. Empty `allowed` means none — no fallback. */
export const portalKeyMcp = (id: number, signal?: AbortSignal) =>
  portalApi<PortalKeyMcpDto>(`/portal/api/keys/${id}/mcp`, signal)

export const setPortalKeyMcp = (id: number, grants: Array<PortalKeyMcpGrantDto>) =>
  portalApi<PortalKeyMcpDto>(`/portal/api/keys/${id}/mcp`, undefined, json("PUT", { grants }))

/** Live activity: what this account's keys are doing right now. */
export const portalActivity = (signal?: AbortSignal) =>
  portalApi<PortalActivityDto>("/portal/api/activity", signal)
