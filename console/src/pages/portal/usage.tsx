import { useState } from "react"
import { useQuery } from "@tanstack/react-query"
import { useTranslation } from "react-i18next"
import { portalQuotaWindows, portalUsage } from "@/api/portal"
import { QuotaWindows } from "@/components/portal/quota-windows"
import { UsagePanel, type UsageDays } from "@/components/portal/usage-panel"

export function PortalUsagePage() {
  const { t } = useTranslation()
  const [days, setDays] = useState<UsageDays>(7)
  const usage = useQuery({
    queryKey: ["portal", "usage", days],
    queryFn: ({ signal }) => {
      const to = Math.floor(Date.now() / 1_000) + 1
      return portalUsage({ from: to - days * 86_400, to }, signal)
    },
  })
  const quota = useQuery({ queryKey: ["portal", "quota-windows"], queryFn: ({ signal }) => portalQuotaWindows(signal) })

  return (
    <>
      <section className="flex flex-col gap-2">
        <h1 className="text-2xl font-semibold tracking-tight">{t("portal.usage.title")}</h1>
        <p className="max-w-3xl text-sm leading-6 text-muted-foreground">{t("portal.usage.description")}</p>
      </section>
      <div className="mt-6 grid gap-6 lg:grid-cols-2">
        <UsagePanel usage={usage.data} days={days} loading={usage.isLoading} error={usage.isError} onDaysChange={setDays} />
        <QuotaWindows windows={quota.data ?? []} loading={quota.isLoading} error={quota.isError} />
      </div>
    </>
  )
}