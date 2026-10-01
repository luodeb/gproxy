import { useState } from "react"
import { useQuery } from "@tanstack/react-query"
import { useTranslation } from "react-i18next"
import { portalRecentRequests, portalUsageOverview } from "@/api/portal"
import { QueryState } from "@/components/query-state"
import { UsageChart, type UsageMetric } from "@/components/portal/usage-chart"
import { RecentRequests } from "@/components/portal/recent-requests"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group"
import { usePortalSession } from "@/lib/portal-session-context"

export function PortalOverviewPage() {
  const { t } = useTranslation()
  const { context } = usePortalSession()
  const recentEnabled = context.recent_requests_enabled
  const [metric, setMetric] = useState<UsageMetric>("requests")

  const usage = useQuery({
    queryKey: ["portal", "usage-overview", 7, "all"],
    queryFn: ({ signal }) => {
      const to = Math.floor(Date.now() / 1_000) + 1
      return portalUsageOverview({ from: to - 7 * 86_400, to }, signal)
    },
  })
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
      <div className="mt-6 flex flex-col gap-6">
        <Card>
          <CardHeader className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
            <CardTitle>{t("portal.usage.trend")}</CardTitle>
            <ToggleGroup type="single" variant="outline" size="sm" value={metric} aria-label={t("portal.usage.metricLabel")} onValueChange={(value) => { if (value === "requests" || value === "tokens") setMetric(value) }}>
              <ToggleGroupItem value="requests">{t("portal.usage.requests")}</ToggleGroupItem>
              <ToggleGroupItem value="tokens">{t("portal.usage.tokens")}</ToggleGroupItem>
            </ToggleGroup>
          </CardHeader>
          <CardContent>
            <QueryState loading={usage.isLoading} error={usage.isError ? t("portal.usage.loadError") : ""}>
              {usage.data ? <UsageChart points={usage.data.series} metric={metric} /> : null}
            </QueryState>
          </CardContent>
        </Card>
        {recentEnabled ? (
          <RecentRequests requests={recent.data ?? []} loading={recent.isLoading} error={recent.isError} />
        ) : null}
      </div>
    </>
  )
}
