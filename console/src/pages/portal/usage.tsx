import { useState } from "react"
import { useQuery } from "@tanstack/react-query"
import { useTranslation } from "react-i18next"
import { portalKeys, portalUsageOverview } from "@/api/portal"
import type { PortalUsageModelDto, PortalUsageOverviewDto } from "@/api/portal"
import { QueryState } from "@/components/query-state"
import { UsageChart, type UsageMetric } from "@/components/portal/usage-chart"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group"
import { formatCost, formatCount } from "@/lib/format"

export type UsageDays = 1 | 7 | 30

const ranges: Array<UsageDays> = [1, 7, 30]

export function PortalUsagePage() {
  const { t } = useTranslation()
  const [days, setDays] = useState<UsageDays>(7)
  const [keyId, setKeyId] = useState("all")
  const [metric, setMetric] = useState<UsageMetric>("requests")

  const keys = useQuery({ queryKey: ["portal", "keys"], queryFn: ({ signal }) => portalKeys(signal) })
  const usage = useQuery({
    queryKey: ["portal", "usage-overview", days, keyId],
    queryFn: ({ signal }) => {
      const to = Math.floor(Date.now() / 1_000) + 1
      return portalUsageOverview({ from: to - days * 86_400, to, user_key_id: keyId === "all" ? undefined : Number(keyId) }, signal)
    },
  })

  const keyLabel = (key: { prefix: string | null; label: string | null; id: number }) => key.label ?? key.prefix ?? `#${key.id}`

  return (
    <>
      <section className="flex flex-col gap-2">
        <h1 className="text-2xl font-semibold tracking-tight">{t("portal.usage.title")}</h1>
        <p className="max-w-3xl text-sm leading-6 text-muted-foreground">{t("portal.usage.description")}</p>
      </section>

      <div className="mt-6 flex flex-wrap items-center gap-3">
        <Select value={keyId} onValueChange={setKeyId}>
          <SelectTrigger className="w-52" aria-label={t("portal.usage.keyLabel")}><SelectValue /></SelectTrigger>
          <SelectContent>
            <SelectItem value="all">{t("portal.usage.allKeys")}</SelectItem>
            {(keys.data ?? []).map((key) => <SelectItem key={key.id} value={String(key.id)}>{keyLabel(key)}</SelectItem>)}
          </SelectContent>
        </Select>
        <ToggleGroup type="single" variant="outline" size="sm" value={String(days)} aria-label={t("portal.usage.rangeLabel")} onValueChange={(value) => { const next = Number(value); if (value && ranges.includes(next as UsageDays)) setDays(next as UsageDays) }}>
          {ranges.map((range) => <ToggleGroupItem key={range} value={String(range)}>{t(`portal.usage.ranges.${range}`)}</ToggleGroupItem>)}
        </ToggleGroup>
      </div>

      <QueryState loading={usage.isLoading} error={usage.isError ? t("portal.usage.loadError") : ""}>
        {usage.data ? <UsageBody usage={usage.data} metric={metric} onMetric={setMetric} /> : null}
      </QueryState>
    </>
  )
}

function UsageBody({ usage, metric, onMetric }: { usage: PortalUsageOverviewDto; metric: UsageMetric; onMetric: (metric: UsageMetric) => void }) {
  const { t, i18n } = useTranslation()
  const maxRequests = Math.max(1, ...usage.models.map((model) => model.requests))

  const stats: Array<[string, string]> = [
    [t("portal.usage.requests"), formatCount(usage.requests, i18n.language)],
    [t("portal.usage.inputTokens"), formatCount(usage.input_tokens, i18n.language)],
    [t("portal.usage.outputTokens"), formatCount(usage.output_tokens, i18n.language)],
    [t("portal.usage.cachedTokens"), formatCount(usage.cached_input_tokens, i18n.language)],
    [t("portal.usage.cost"), formatCost(usage.cost, i18n.language)],
  ]

  return (
    <div className="mt-6 flex flex-col gap-6">
      <dl className="grid gap-px overflow-hidden rounded-xl border bg-border sm:grid-cols-3 lg:grid-cols-5">
        {stats.map(([label, value]) => (
          <div key={label} className="flex flex-col gap-1 bg-card px-4 py-3">
            <dt className="text-xs text-muted-foreground">{label}</dt>
            <dd className="font-mono text-lg">{value}</dd>
          </div>
        ))}
      </dl>

      <Card>
        <CardHeader className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
          <div>
            <CardTitle>{t("portal.usage.trend")}</CardTitle>
            <CardDescription>{t("portal.usage.trendDescription")}</CardDescription>
          </div>
          <ToggleGroup type="single" variant="outline" size="sm" value={metric} aria-label={t("portal.usage.metricLabel")} onValueChange={(value) => { if (value === "requests" || value === "tokens") onMetric(value) }}>
            <ToggleGroupItem value="requests">{t("portal.usage.requests")}</ToggleGroupItem>
            <ToggleGroupItem value="tokens">{t("portal.usage.tokens")}</ToggleGroupItem>
          </ToggleGroup>
        </CardHeader>
        <CardContent>
          <UsageChart points={usage.series} metric={metric} />
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>{t("portal.usage.byModel")}</CardTitle>
          <CardDescription>{t("portal.usage.byModelDescription")}</CardDescription>
        </CardHeader>
        <CardContent>
          {usage.models.length === 0 ? <p className="py-6 text-center text-sm text-muted-foreground">{t("portal.usage.noData")}</p> : (
            <div className="flex flex-col gap-3">
              {usage.models.map((model) => <ModelRow key={`${model.provider ?? ""}/${model.model}`} model={model} maxRequests={maxRequests} />)}
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  )
}

function ModelRow({ model, maxRequests }: { model: PortalUsageModelDto; maxRequests: number }) {
  const { t, i18n } = useTranslation()
  return (
    <div className="flex flex-col gap-1">
      <div className="flex flex-wrap items-center justify-between gap-x-4 gap-y-1 text-sm">
        <span className="flex min-w-0 items-center gap-2">
          <span className="min-w-0 truncate font-mono text-xs">{model.model}</span>
          {model.provider ? <span className="shrink-0 truncate text-xs text-muted-foreground">{model.provider}</span> : null}
        </span>
        <span className="flex items-center gap-4 text-xs text-muted-foreground">
          <span>{formatCount(model.requests, i18n.language)} {t("portal.usage.requests")}</span>
          <span>{formatCount(model.input_tokens + model.output_tokens, i18n.language)} {t("portal.usage.tokens")}</span>
          <span className="font-mono">{formatCost(model.cost, i18n.language)}</span>
        </span>
      </div>
      <div className="h-1.5 w-full overflow-hidden rounded-full bg-muted">
        <div className="h-full rounded-full bg-primary" style={{ width: `${Math.max(2, (model.requests / maxRequests) * 100)}%` }} />
      </div>
    </div>
  )
}
