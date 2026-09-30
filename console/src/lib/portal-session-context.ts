import { createContext, useContext } from "react"
import type { PortalContextDto } from "@/generated/PortalContextDto"

export type PortalSessionValue = {
  context: PortalContextDto
  logout: () => void
}

export const PortalSessionContext = createContext<PortalSessionValue | null>(null)

export function usePortalSession() {
  const value = useContext(PortalSessionContext)
  if (!value) throw new Error("usePortalSession must be used within PortalShell")
  return value
}