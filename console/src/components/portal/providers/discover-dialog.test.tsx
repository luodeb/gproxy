import { QueryClient, QueryClientProvider } from "@tanstack/react-query"
import { render, screen, waitFor } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { expect, test, vi } from "vitest"
import "@/i18n"
import { PortalProvidersPage } from "@/pages/portal/providers"
import * as control from "@/api/control"
import type { ChannelDto, CredentialDto, DiscoveredModelDto, ManagedModelDto, ModelDiscoverResponse, ProviderDto, ProviderModelDto } from "@/api/control"

vi.mock("@/api/control", async (importOriginal) => ({
  ...(await importOriginal<object>()),
  listChannels: vi.fn(),
  listProviders: vi.fn(),
  listCredentials: vi.fn(),
  listProviderModels: vi.fn(),
  listManagedModels: vi.fn(),
  discoverProviderModels: vi.fn(),
}))

const provider: ProviderDto = {
  id: 1,
  name: "zhipu",
  label: "Zhipu",
  channel: "zhipu",
  settings: {},
  traffic_policy: null,
  credential_strategy: "rotate",
  proxy_url: null,
  tls_fingerprint: null,
  invalid_tls_fingerprint: null,
  tls_fingerprint_error: null,
  enabled: true,
}

const channel: ChannelDto = {
  id: "zhipu",
  display_name: "Zhipu",
  provider_fields: [],
  credential_fields: [],
}

const credential: CredentialDto = {
  id: 1,
  provider_id: 1,
  label: "main",
  kind: "api_key",
  version: 1,
  enabled: true,
  weight: 100,
  rpm_limit: null,
  tpm_limit: null,
  proxy_url: null,
  tls_fingerprint: null,
  health: "healthy",
  health_observed_at: null,
  health_response_status: null,
  health_detail: null,
  model_health: [],
}

const upstreamModel: ProviderModelDto = {
  id: 1,
  provider_id: 1,
  model_id: "glm-4.7",
  display_name: null,
  variants: null,
  context_window: null,
  max_output_tokens: null,
  thinking_supported: null,
  thinking_adaptive_supported: null,
  thinking_enabled_supported: null,
  metadata: {},
  enabled: true,
}

const managedModel: ManagedModelDto = {
  id: 1,
  name: "glm-4.7",
  display_name: null,
  enabled: true,
  sources: [],
}

const discovered: DiscoveredModelDto = {
  model_id: "glm-5",
  display_name: "GLM-5",
  known: false,
  context_window: null,
  max_output_tokens: null,
  thinking_supported: null,
  thinking_adaptive_supported: null,
  thinking_enabled_supported: null,
  metadata: {},
  default_price_available: false,
}

function mockApi() {
  vi.mocked(control.listChannels).mockResolvedValue([channel])
  vi.mocked(control.listProviders).mockResolvedValue([provider])
  vi.mocked(control.listCredentials).mockResolvedValue([credential])
  vi.mocked(control.listProviderModels).mockResolvedValue([upstreamModel])
  vi.mocked(control.listManagedModels).mockResolvedValue([managedModel])
}

function renderPage() {
  return render(
    <QueryClientProvider client={new QueryClient()}>
      <PortalProvidersPage />
    </QueryClientProvider>,
  )
}

test("clicking discover opens the dialog and lists upstream models", async () => {
  const user = userEvent.setup()
  mockApi()
  const response: ModelDiscoverResponse = { ok: true, status: 200, latency_ms: 5, key_prefix: "sk-gp-test", models: [discovered], message: null }
  vi.mocked(control.discoverProviderModels).mockResolvedValue(response)

  renderPage()
  // The provider name appears in both the expand button and the channel badge; the
  // expand toggle is the <button> ancestor of the first match.
  const expand = (await screen.findAllByText("Zhipu"))[0].closest("button")
  expect(expand).not.toBeNull()
  await user.click(expand!)
  await user.click(await screen.findByRole("button", { name: "Discover models" }))

  await waitFor(() => {
    expect(screen.getByRole("dialog")).toBeInTheDocument()
  })
  expect(await screen.findByText("glm-5")).toBeInTheDocument()
  expect(control.discoverProviderModels).toHaveBeenCalledWith(1, expect.anything())
})
