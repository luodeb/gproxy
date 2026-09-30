import { lazy, Suspense, useEffect, useState } from "react"
import { useQueryClient } from "@tanstack/react-query"
import type { PortalContextDto } from "@/generated/PortalContextDto"
import { portalLogin, portalLogout, portalSession } from "@/api/portal"
import { AuthPanel } from "@/components/auth/auth-panel"
import { PortalShell } from "@/components/portal/portal-shell"
import { PortalSessionProvider } from "@/components/portal/portal-session"
import { OAuthConsent } from "@/components/portal/oauth-consent"
import { QueryState } from "@/components/query-state"
import { oauthReturnUrl } from "@/lib/oauth-callback"
import { usePortalLocation } from "@/lib/portal-route"

const OverviewPage = lazy(() => import("@/pages/portal/overview").then((module) => ({ default: module.PortalOverviewPage })))
const ConnectPage = lazy(() => import("@/pages/portal/connect").then((module) => ({ default: module.PortalConnectPage })))
const UsagePage = lazy(() => import("@/pages/portal/usage").then((module) => ({ default: module.PortalUsagePage })))
const KeysPage = lazy(() => import("@/pages/portal/keys").then((module) => ({ default: module.PortalKeysPage })))
const SessionsPage = lazy(() => import("@/pages/portal/sessions").then((module) => ({ default: module.PortalSessionsPage })))

type PortalSession = { context: PortalContextDto }

function continueOAuth() {
  const value = new URLSearchParams(window.location.search).get("oauth_return")
  const target = oauthReturnUrl(value, window.location.origin)
  if (target) window.location.assign(target)
}

export function PortalPage() {
  const queryClient = useQueryClient()
  const { route } = usePortalLocation()
  const [session, setSession] = useState<PortalSession | null>(null)
  const [sessionLoading, setSessionLoading] = useState(true)
  const [loginPending, setLoginPending] = useState(false)
  const [loginFailed, setLoginFailed] = useState(false)
  const params = new URLSearchParams(window.location.search)
  const authorization = params.get("oauth_authorize")
  const deviceCode = params.get("oauth_device")
  const authorizing = authorization != null || deviceCode != null

  useEffect(() => {
    void portalSession()
      .then((status) => {
        if (status.user) {
          setSession({ context: status.user })
          continueOAuth()
        }
      })
      .finally(() => setSessionLoading(false))
  }, [])

  async function login(username: string, password: string) {
    setLoginPending(true)
    setLoginFailed(false)
    try {
      const context = await portalLogin({ username, password })
      setSession({ context })
      continueOAuth()
    } catch {
      setLoginFailed(true)
    } finally {
      setLoginPending(false)
    }
  }

  async function logout() {
    try {
      await portalLogout()
      queryClient.clear()
      window.location.assign("/portal")
    } catch {
      return
    }
  }

  if (sessionLoading) {
    return <main className="mx-auto max-w-2xl px-5 py-16"><QueryState loading error="">{null}</QueryState></main>
  }

  if (!session) {
    return (
      <AuthPanel
        setup={false}
        audience="portal"
        pending={loginPending}
        failed={loginFailed}
        onSubmit={(username, password) => void login(username, password)}
      />
    )
  }

  if (authorizing) {
    return <main className="mx-auto max-w-7xl px-4 py-8 sm:px-6 lg:px-8"><OAuthConsent authorization={authorization} deviceCode={deviceCode} /></main>
  }

  const page = { overview: <OverviewPage />, connect: <ConnectPage />, usage: <UsagePage />, keys: <KeysPage />, sessions: <SessionsPage /> }[route]

  return (
    <PortalSessionProvider value={{ context: session.context, logout: () => void logout() }}>
      <PortalShell route={route}>
        <Suspense fallback={<QueryState loading error="">{null}</QueryState>}>{page}</Suspense>
      </PortalShell>
    </PortalSessionProvider>
  )
}