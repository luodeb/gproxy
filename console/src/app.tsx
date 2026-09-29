import { lazy, Suspense } from "react"
import { QueryState } from "@/components/query-state"

const AdminSurface = lazy(() => import("@/surfaces/admin-surface").then((module) => ({ default: module.AdminSurface })))
const PortalSurface = lazy(() => import("@/surfaces/portal-surface").then((module) => ({ default: module.PortalSurface })))

type Surface = "portal" | "admin"

// 根路径直接进入登录页（admin surface 未登录时渲染 AuthPanel），不再有公共首页。
function surfaceForPath(pathname: string): Surface {
  if (pathname === "/portal" || pathname.startsWith("/portal/")) return "portal"
  return "admin"
}

function SurfaceLoading() {
  return <main className="mx-auto max-w-2xl px-5 py-16"><QueryState loading error="">{null}</QueryState></main>
}

export function App() {
  const surface = surfaceForPath(window.location.pathname)
  if (surface === "portal") return <Suspense fallback={<SurfaceLoading />}><PortalSurface /></Suspense>
  return <Suspense fallback={<SurfaceLoading />}><AdminSurface /></Suspense>
}
