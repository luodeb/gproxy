import type { ReactNode } from "react"
import { PortalSessionContext, type PortalSessionValue } from "@/lib/portal-session-context"

export function PortalSessionProvider({ value, children }: { value: PortalSessionValue; children: ReactNode }) {
  return <PortalSessionContext.Provider value={value}>{children}</PortalSessionContext.Provider>
}