import { useSyncExternalStore } from "react"

export type PortalRoute = "overview" | "connect" | "usage" | "keys" | "sessions"
export type PortalLocation = { route: PortalRoute; segments: Array<string> }

const routes = new Set<PortalRoute>(["overview", "connect", "usage", "keys", "sessions"])
const serverLocation: PortalLocation = { route: "overview", segments: [] }
let cachedPath = ""
let cachedLocation = serverLocation

function readLocation(): PortalLocation {
  const pathname = window.location.pathname
  if (pathname === cachedPath) return cachedLocation
  const parts = pathname.split("/").filter(Boolean)
  cachedPath = pathname
  if (parts[0] !== "portal") {
    cachedLocation = serverLocation
    return cachedLocation
  }
  const candidate = parts[1] as PortalRoute | undefined
  cachedLocation = !candidate || !routes.has(candidate)
    ? serverLocation
    : { route: candidate, segments: parts.slice(2).map(decodeURIComponent) }
  return cachedLocation
}

function subscribe(listener: () => void) {
  window.addEventListener("popstate", listener)
  return () => window.removeEventListener("popstate", listener)
}

export function usePortalLocation() {
  return useSyncExternalStore(subscribe, readLocation, () => serverLocation)
}

export function portalPath(route: PortalRoute) {
  return route === "overview" ? "/portal" : `/portal/${route}`
}

export function navigatePortalPath(path: string, replace = false) {
  if (replace) window.history.replaceState(null, "", path)
  else window.history.pushState(null, "", path)
  window.dispatchEvent(new PopStateEvent("popstate"))
}