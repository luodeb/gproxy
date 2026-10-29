import { Area, AreaChart, CartesianGrid, Legend, Line, LineChart, ResponsiveContainer, Tooltip, XAxis, YAxis } from "recharts"
import { useTranslation } from "react-i18next"
import type { PortalUsageModelSeriesDto, PortalUsagePointDto } from "@/api/portal"
import { formatCount } from "@/lib/format"

export type UsageMetric = "requests" | "tokens"

const MODEL_COLORS = ["var(--primary)", "#f59e0b", "#10b981", "#3b82f6", "#a855f7", "#ef4444", "#14b8a6", "#f97316", "#8b5cf6", "#ec4899", "#84cc16", "#64748b"]

export function UsageChart({ points, metric, height = 256 }: { points: Array<PortalUsagePointDto>; metric: UsageMetric; height?: number }) {
  const { t, i18n } = useTranslation()
  const data = points.map((point) => ({
    time: point.bucket_start * 1_000,
    requests: point.requests,
    tokens: point.input_tokens + point.output_tokens,
  }))

  if (data.length === 0) {
    return <p className="py-16 text-center text-sm text-muted-foreground">{t("portal.usage.noData")}</p>
  }

  return (
    <div className="w-full outline-none [&_*]:outline-none" style={{ height }}>
      <ResponsiveContainer width="100%" height="100%">
        <AreaChart accessibilityLayer={false} data={data} margin={{ top: 8, right: 8, bottom: 0, left: 0 }}>
          <defs>
            <linearGradient id="usage-fill" x1="0" y1="0" x2="0" y2="1">
              <stop offset="5%" stopColor="var(--primary)" stopOpacity={0.35} />
              <stop offset="95%" stopColor="var(--primary)" stopOpacity={0.02} />
            </linearGradient>
          </defs>
          <CartesianGrid strokeDasharray="3 3" stroke="var(--border)" vertical={false} />
          <XAxis
            dataKey="time"
            tickFormatter={(value) => new Intl.DateTimeFormat(i18n.language, { month: "short", day: "numeric", hour: "2-digit" }).format(new Date(value))}
            tick={{ fontSize: 11 }}
            minTickGap={24}
            stroke="var(--muted-foreground)"
          />
          <YAxis tickFormatter={(value) => formatCount(Number(value), i18n.language)} tick={{ fontSize: 11 }} width={48} stroke="var(--muted-foreground)" />
          <Tooltip
            cursor={{ stroke: "var(--border)", strokeWidth: 1 }}
            labelFormatter={(value) => new Intl.DateTimeFormat(i18n.language, { dateStyle: "medium", timeStyle: "short" }).format(new Date(Number(value)))}
            formatter={(value) => [formatCount(Number(value), i18n.language), metric === "tokens" ? t("portal.usage.tokens") : t("portal.usage.requests")]}
            contentStyle={{ background: "var(--popover)", border: "1px solid var(--border)", borderRadius: 8, fontSize: 12 }}
          />
          <Area type="monotone" dataKey={metric} stroke="var(--primary)" strokeWidth={2} fill="url(#usage-fill)" />
        </AreaChart>
      </ResponsiveContainer>
    </div>
  )
}

function modelValue(point: PortalUsagePointDto, metric: UsageMetric): number {
  return metric === "tokens" ? point.input_tokens + point.output_tokens : point.requests
}

export function UsageModelChart({ modelSeries, metric, height = 256 }: { modelSeries: Array<PortalUsageModelSeriesDto>; metric: UsageMetric; height?: number }) {
  const { t, i18n } = useTranslation()
  const times = [...new Set(modelSeries.flatMap((entry) => entry.series.map((point) => point.bucket_start)))].sort((left, right) => left - right)
  const byModel = new Map(modelSeries.map((entry) => [entry.model, new Map(entry.series.map((point) => [point.bucket_start, modelValue(point, metric)]))]))
  const data = times.map((bucket) => {
    const row: Record<string, number | number[]> = { time: bucket * 1_000 }
    for (const [model, points] of byModel) row[model] = points.get(bucket) ?? 0
    return row
  })

  if (data.length === 0) {
    return <p className="py-16 text-center text-sm text-muted-foreground">{t("portal.usage.noData")}</p>
  }

  return (
    <div className="w-full outline-none [&_*]:outline-none" style={{ height }}>
      <ResponsiveContainer width="100%" height="100%">
        <LineChart accessibilityLayer={false} data={data} margin={{ top: 8, right: 8, bottom: 0, left: 0 }}>
          <CartesianGrid strokeDasharray="3 3" stroke="var(--border)" vertical={false} />
          <XAxis
            dataKey="time"
            tickFormatter={(value) => new Intl.DateTimeFormat(i18n.language, { month: "short", day: "numeric", hour: "2-digit" }).format(new Date(value))}
            tick={{ fontSize: 11 }}
            minTickGap={24}
            stroke="var(--muted-foreground)"
          />
          <YAxis tickFormatter={(value) => formatCount(Number(value), i18n.language)} tick={{ fontSize: 11 }} width={48} stroke="var(--muted-foreground)" />
          <Tooltip
            cursor={{ stroke: "var(--border)", strokeWidth: 1 }}
            labelFormatter={(value) => new Intl.DateTimeFormat(i18n.language, { dateStyle: "medium", timeStyle: "short" }).format(new Date(Number(value)))}
            formatter={(value, name) => [formatCount(Number(value), i18n.language), String(name)]}
            contentStyle={{ background: "var(--popover)", border: "1px solid var(--border)", borderRadius: 8, fontSize: 12 }}
          />
          <Legend wrapperStyle={{ fontSize: 11 }} />
          {modelSeries.map((entry, index) => (
            <Line key={entry.model} type="monotone" dataKey={entry.model} stroke={MODEL_COLORS[index % MODEL_COLORS.length]} strokeWidth={2} dot={false} connectNulls />
          ))}
        </LineChart>
      </ResponsiveContainer>
    </div>
  )
}
