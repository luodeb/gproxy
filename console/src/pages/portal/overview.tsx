import { useQuery } from "@tanstack/react-query"
import { useTranslation } from "react-i18next"
import { portalQuotaWindows, portalRecentRequests } from "@/api/portal"
import { QueryState } from "@/components/query-state"
import { QuotaWindows } from "@/components/portal/quota-windows"
import { RecentRequests } from "@/components/portal/recent-requests"
import { usePortalSession } from "@/lib/portal-session-context"

export function PortalOverviewPage() {
  const { t } = useTranslation()
  const { context } = usePortalSession()
  const recentEnabled = context.recent_requests_enabled
  const quota = useQuery({ queryKey: ["portal", "quota-windows"], queryFn: ({ signal }) => portalQuotaWindows(signal) })
  const recent = useQuery({
    queryKey: ["portal", "recent-requests"],
    queryFn: ({ signal }) => portalRecentRequests({ limit: 20 }, signal),
    enabled: recentEnabled,
  })

  return (
    <>
      <section className="flex flex-col gap-2">
        <h1 className="text-2xl font-semibold tracking-tight">{t("portal.overview.title", { name: context.user_name })}</h1>
        <p className="max-w-3xl text-sm leading-6 text-muted-foreground">{t("portal.overview.description")}</p>
      </section>
      <QueryState loading={quota.isLoading} error={quota.isError ? t("portal.quota.loadError") : ""}>
        <div className="mt-6 flex flex-col gap-6">
          <QuotaWindows windows={quota.data ?? []} loading={false} error={false} />
          {recentEnabled ? (
            <RecentRequests requests={recent.data ?? []} loading={recent.isLoading} error={recent.isError} />
          ) : null}
        </div>
      </QueryState>
    </>
  )
}