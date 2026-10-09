import { QueryClient, QueryClientProvider } from "@tanstack/react-query"
import { render, screen } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { expect, test, vi } from "vitest"
import "@/i18n"
import { KeyPermissions } from "@/components/portal/key-permissions"
import type { UserKeyDto } from "@/generated/UserKeyDto"
import * as portal from "@/api/portal"

vi.mock("@/api/portal", async (importOriginal) => ({
  ...(await importOriginal<object>()),
  portalKeyModels: vi.fn(),
  setPortalKeyModels: vi.fn(),
  portalKeyMcp: vi.fn(),
  setPortalKeyMcp: vi.fn(),
}))

const keyRecord: UserKeyDto = {
  id: 2,
  user_id: 1,
  prefix: "sk-gp-test",
  label: "mcp-test",
  revealable: true,
  expires_at: null,
  enabled: true,
}

function renderPanel() {
  return render(
    <QueryClientProvider client={new QueryClient()}>
      <KeyPermissions keyRecord={keyRecord} />
    </QueryClientProvider>,
  )
}

test("the expanded key row shows model and MCP access and saves both", async () => {
  const user = userEvent.setup()
  vi.mocked(portal.portalKeyModels).mockResolvedValue({ available: ["glm-4.7", "glm-5"], allowed: ["glm-4.7"] })
  vi.mocked(portal.portalKeyMcp).mockResolvedValue({
    available: [{ name: "zhipu-reader", enabled: true, tools: ["webReader"] }],
    allowed: [{ server: "zhipu-reader", patterns: ["web*"] }],
  })
  vi.mocked(portal.setPortalKeyModels).mockResolvedValue({ available: [], allowed: [] })
  vi.mocked(portal.setPortalKeyMcp).mockResolvedValue({ available: [], allowed: [] })

  renderPanel()

  // Model access: the stored allow-list is checked, the rest are not.
  expect(await screen.findByText("glm-4.7")).toBeInTheDocument()
  expect(screen.getByRole("checkbox", { name: "glm-4.7" })).toBeChecked()
  expect(screen.getByRole("checkbox", { name: "glm-5" })).not.toBeChecked()

  // MCP access: the granted server is checked and carries its pattern.
  expect(screen.getByRole("checkbox", { name: /zhipu-reader/ })).toBeChecked()
  const pattern = screen.getByLabelText("Tool pattern for zhipu-reader") as HTMLInputElement
  expect(pattern.value).toBe("web*")

  // Adding a model and the upstream tool, then saving, sends both allow-lists.
  await user.click(screen.getByRole("checkbox", { name: "glm-5" }))
  await user.click(screen.getByRole("button", { name: "webReader" }))
  await user.click(screen.getByRole("button", { name: "Save" }))

  await vi.waitFor(() => {
    expect(portal.setPortalKeyModels).toHaveBeenCalledWith(2, ["glm-4.7", "glm-5"])
    expect(portal.setPortalKeyMcp).toHaveBeenCalledWith(2, [
      { server: "zhipu-reader", patterns: ["web*", "webReader"] },
    ])
  })
})
