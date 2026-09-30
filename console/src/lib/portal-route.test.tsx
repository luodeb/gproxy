import { act, render, screen } from "@testing-library/react"
import { afterEach, expect, test } from "vitest"
import { portalPath, usePortalLocation } from "./portal-route"

function Probe() {
  const { route } = usePortalLocation()
  return <span data-testid="route">{route}</span>
}

afterEach(() => window.history.replaceState(null, "", "/"))

function go(path: string) {
  act(() => window.history.pushState(null, "", path))
  act(() => window.dispatchEvent(new PopStateEvent("popstate")))
}

test("portal paths address the overview at the surface root", () => {
  expect(portalPath("overview")).toBe("/portal")
  expect(portalPath("connect")).toBe("/portal/connect")
  expect(portalPath("keys")).toBe("/portal/keys")
  expect(portalPath("sessions")).toBe("/portal/sessions")
})

test("the portal reads its section from the real URL", () => {
  window.history.replaceState(null, "", "/portal/usage")
  render(<Probe />)
  expect(screen.getByTestId("route")).toHaveTextContent("usage")
  go("/portal/sessions")
  expect(screen.getByTestId("route")).toHaveTextContent("sessions")
  go("/portal")
  expect(screen.getByTestId("route")).toHaveTextContent("overview")
})

test("unknown, admin and non-portal paths fall back to the overview", () => {
  window.history.replaceState(null, "", "/portal/not-a-section")
  render(<Probe />)
  expect(screen.getByTestId("route")).toHaveTextContent("overview")
  go("/admin/providers")
  expect(screen.getByTestId("route")).toHaveTextContent("overview")
  go("/v1/chat/completions")
  expect(screen.getByTestId("route")).toHaveTextContent("overview")
})