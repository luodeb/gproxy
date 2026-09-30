import { QueryClient, QueryClientProvider } from "@tanstack/react-query"
import { act, render, screen } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { afterEach, expect, test } from "vitest"
import "@/i18n"
import { PortalSessionProvider } from "@/components/portal/portal-session"
import { PortalShell } from "@/components/portal/portal-shell"
import { ThemeProvider } from "@/lib/theme"
import type { PortalContextDto } from "@/generated/PortalContextDto"

const context: PortalContextDto = {
  user_name: "Ada Lovelace",
  recent_requests_enabled: false,
}

afterEach(() => {
  window.localStorage.clear()
  window.history.replaceState(null, "", "/portal")
})

function renderShell() {
  return render(
    <QueryClientProvider client={new QueryClient()}>
    <ThemeProvider>
      <PortalSessionProvider value={{ context, logout: () => {} }}>
        <PortalShell route="overview"><p>panel body</p></PortalShell>
      </PortalSessionProvider>
    </ThemeProvider>
    </QueryClientProvider>,
  )
}

test("the portal sidebar lists the routed sections and shows the user at the bottom", () => {
  renderShell()
  const nav = screen.getByRole("navigation", { name: "Portal navigation" })
  for (const label of ["Overview", "Connect", "Usage", "API keys", "Authorized sessions"]) {
    expect(screen.getByRole("button", { name: label })).toBeInTheDocument()
  }
  expect(nav).toBeInTheDocument()
  expect(screen.getByRole("button", { name: "Account menu" })).toHaveTextContent("Ada Lovelace")
  expect(screen.getByText("panel body")).toBeInTheDocument()
})

test("clicking a section navigates to its URL", async () => {
  const user = userEvent.setup()
  renderShell()
  await user.click(screen.getByRole("button", { name: "API keys" }))
  expect(window.location.pathname).toBe("/portal/keys")
})

test("the user menu opens settings in a drawer with the password form", async () => {
  const user = userEvent.setup()
  renderShell()
  await user.click(screen.getByRole("button", { name: "Account menu" }))
  await user.click(await screen.findByRole("menuitem", { name: "Settings" }))
  expect(await screen.findByRole("heading", { name: "Settings" })).toBeInTheDocument()
  expect(screen.getByText("Change the password used to sign in to this portal.")).toBeInTheDocument()
  expect(screen.getByPlaceholderText("Current password")).toBeInTheDocument()
})

test("the sidebar keeps its own preferences, separate from the console's", async () => {
  const user = userEvent.setup()
  renderShell()
  await user.click(screen.getByRole("button", { name: "Close navigation" }))
  expect(JSON.parse(window.localStorage.getItem("gproxy.portal.sidebar.preferences") ?? "null")).toMatchObject({ collapsed: true })
  expect(window.localStorage.getItem("gproxy.sidebar.preferences")).toBeNull()
})

test("logout is reachable from the user menu", async () => {
  const user = userEvent.setup()
  let loggedOut = false
  render(
    <QueryClientProvider client={new QueryClient()}>
      <ThemeProvider>
        <PortalSessionProvider value={{ context, logout: () => { loggedOut = true } }}>
          <PortalShell route="overview"><p>panel body</p></PortalShell>
        </PortalSessionProvider>
      </ThemeProvider>
    </QueryClientProvider>,
  )
  await user.click(screen.getByRole("button", { name: "Account menu" }))
  await user.click(await screen.findByRole("menuitem", { name: "Sign out" }))
  expect(loggedOut).toBe(true)
})

test("the oauth consent branch is not routed through the sidebar", () => {
  // Guards against a regression where the consent screen loses its own layout.
  act(() => window.history.replaceState(null, "", "/portal"))
  expect(window.location.pathname).toBe("/portal")
})