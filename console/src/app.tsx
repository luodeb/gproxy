import { lazy, Suspense } from "react"
import { QueryState } from "@/components/query-state"

const PortalSurface = lazy(() => import("@/surfaces/portal-surface").then((module) => ({ default: module.PortalSurface })))

function SurfaceLoading() {
  return <main className="mx-auto max-w-2xl px-5 py-16"><QueryState loading error="">{null}</QueryState></main>
}

// 本 fork 只有用户门户这一个界面：根路径直接进入门户（未登录时渲染登录页），
// 管理能力通过 MCP / admin API 提供，不再有 Web 控制台。
export function App() {
  return <Suspense fallback={<SurfaceLoading />}><PortalSurface /></Suspense>
}
