import { useEffect, useMemo, useRef, useState } from "react"
import { useQuery } from "@tanstack/react-query"
import { useTranslation } from "react-i18next"
import { portalActivity } from "@/api/portal"
import type { PortalActivityRequestDto } from "@/generated/PortalActivityRequestDto"
import type { PortalActivitySessionDto } from "@/generated/PortalActivitySessionDto"
import { QueryState } from "@/components/query-state"
import { CountUp, EqBars, Sparkline } from "@/components/portal/live-bits"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import { formatCount } from "@/lib/format"
import { cn } from "@/lib/utils"

/// How often the panel refreshes. Fast enough to feel live, slow enough that a
/// room full of open overviews does not become the gateway's busiest client.
const REFRESH_MS = 3_000

/// A session counts as online until it has been quiet for this long. Shorter
/// than the aggregate window, so a pause between turns does not flash offline.
const ONLINE_SECONDS = 60

/// How many throughput samples the per-session sparkline keeps (3s each).
const SAMPLE_CAP = 20

type SessionRow = PortalActivitySessionDto & { inFlight: number; live: boolean }

/// The live view, one row per session: which keys are talking, on which
/// models, how fast they generate — with motion that says so: counters tick,
/// streaming rows shimmer, sparklines grow a point per poll.
export function Activity() {
  const { t, i18n } = useTranslation()
  const activity = useQuery({
    queryKey: ["portal", "activity"],
    queryFn: ({ signal }) => portalActivity(signal),
    refetchInterval: REFRESH_MS,
    staleTime: 0,
  })
  // Elapsed-time labels tick every second even between polls.
  const [, setTick] = useState(0)
  useEffect(() => {
    const timer = window.setInterval(() => setTick((value) => value + 1), 1_000)
    return () => window.clearInterval(timer)
  }, [])

  // Rolling per-session throughput samples for the sparklines.
  const [samples, setSamples] = useState<Map<number, Array<number>>>(new Map())
  const activityData = activity.data
  useEffect(() => {
    if (!activityData) return
    setSamples((current) => {
      const next = new Map(current)
      for (const session of activityData.sessions) {
        const history = next.get(session.key_id) ?? []
        next.set(session.key_id, [...history, session.tokens_per_second].slice(-SAMPLE_CAP))
      }
      return next
    })
  }, [activityData])

  // In-flight requests grouped by key, so a session row can show live progress
  // — including keys whose first request has not settled into usage yet.
  const inFlightByKey = useMemo(() => {
    const map = new Map<number, { count: number; sample: PortalActivityRequestDto }>()
    for (const request of activityData?.requests ?? []) {
      const known = map.get(request.key_id)
      map.set(request.key_id, { count: (known?.count ?? 0) + 1, sample: known?.sample ?? request })
    }
    return map
  }, [activityData])

  const rows = useMemo(() => {
    const sessions = activityData?.sessions ?? []
    const withLive = sessions.map(
      (session): SessionRow => ({ ...session, inFlight: inFlightByKey.get(session.key_id)?.count ?? 0, live: false }),
    )
    for (const [keyId, info] of inFlightByKey) {
      if (sessions.some((session) => session.key_id === keyId)) continue
      const request = info.sample
      withLive.push({
        key_id: keyId,
        prefix: request.prefix,
        label: request.label,
        requests: 0,
        input_tokens: 0,
        output_tokens: 0,
        last_at: Math.floor(Date.now() / 1000),
        models: request.model ? [request.model] : [],
        tokens_per_second: 0,
        inFlight: info.count,
        live: true,
      })
    }
    return withLive
  }, [activityData, inFlightByKey])

  const now = activityData?.now
  const onlineCount = rows.filter((row) => isOnline(row, now)).length
  const live = (activityData?.in_flight ?? 0) > 0
  const tiles: Array<[string, number, (value: number) => string]> = [
    [t("portal.activity.inFlight"), activityData?.in_flight ?? 0, (value) => String(Math.round(value))],
    [t("portal.activity.requestsPerMinute"), activityData?.last_minute_requests ?? 0, (value) => formatCount(Math.round(value), i18n.language)],
    [t("portal.activity.tokensPerMinute"), activityData?.last_minute_tokens ?? 0, (value) => formatCount(Math.round(value), i18n.language)],
    [t("portal.activity.activeSessions"), onlineCount, (value) => String(Math.round(value))],
  ]

  return (
    <Card>
      <CardHeader className="flex flex-row items-center justify-between gap-3">
        <CardTitle>{t("portal.activity.title")}</CardTitle>
        <span className="flex items-center gap-2 text-xs text-muted-foreground">
          <EqBars active={live} />
          {t("portal.activity.refresh", { seconds: Math.round(REFRESH_MS / 1000) })}
        </span>
      </CardHeader>
      <CardContent className="flex flex-col gap-4">
        <QueryState loading={activity.isLoading} error={activity.isError ? t("portal.activity.loadError") : ""}>
          <dl className="grid gap-px overflow-hidden rounded-xl border bg-border grid-cols-2 lg:grid-cols-4">
            {tiles.map(([label, value, format]) => (
              <div key={label} className="flex flex-col gap-1 bg-card px-4 py-3">
                <dt className="text-xs text-muted-foreground">{label}</dt>
                <CountUp value={value} format={format} className="font-mono text-lg tabular-nums" />
              </div>
            ))}
          </dl>

          <section className="mt-4 flex flex-col gap-2">
            <h3 className="text-sm font-medium">{t("portal.activity.sessionsTitle")}</h3>
            <SessionList rows={rows} samples={samples} now={now} />
          </section>

          <section className="flex flex-col gap-2">
            <h3 className="text-sm font-medium">{t("portal.activity.modelsTitle")}</h3>
            <ModelList models={activityData?.models ?? []} now={now} />
          </section>
        </QueryState>
      </CardContent>
    </Card>
  )
}

function isOnline(session: { last_at: number; inFlight?: number }, now: number | undefined): boolean {
  if ((session.inFlight ?? 0) > 0) return true
  return now != null && now - session.last_at < ONLINE_SECONDS
}

/// One row per session: identity, the models it touched, what it did in the
/// window, and its generating speed while online. A session stays online for
/// {@link ONLINE_SECONDS} after its last activity, and rows that own in-flight
/// requests shimmer while they stream.
function SessionList({ rows, samples, now }: {
  rows: Array<SessionRow>
  samples: Map<number, Array<number>>
  now: number | undefined
}) {
  const { t, i18n } = useTranslation()
  // Keep departed sessions around just long enough to fade out in place.
  const [leaving, setLeaving] = useState<Array<SessionRow>>([])
  const known = useRef<Map<number, SessionRow>>(new Map())
  useEffect(() => {
    const ids = new Set(rows.map((row) => row.key_id))
    const departed = [...known.current.values()].filter(
      (row) => !ids.has(row.key_id) && !leaving.some((row) => row.key_id === row.key_id),
    )
    known.current = new Map(rows.map((row) => [row.key_id, row]))
    if (departed.length === 0) return
    setLeaving((current) => [...current, ...departed])
    const timer = window.setTimeout(() => {
      setLeaving((current) => current.filter((row) => !departed.some((departed_) => departed_.key_id === row.key_id)))
    }, 400)
    return () => window.clearTimeout(timer)
  }, [rows])

  const ordered = useMemo(() => {
    const all = [...rows, ...leaving.filter((row) => !rows.some((row_) => row_.key_id === row.key_id))]
    return all.sort((left, right) => {
      const leftOnline = isOnline(left, now)
      const rightOnline = isOnline(right, now)
      return leftOnline === rightOnline ? right.last_at - left.last_at : leftOnline ? -1 : 1
    })
  }, [rows, leaving, now])

  if (ordered.length === 0) return <p className="text-sm text-muted-foreground">{t("portal.activity.sessionsEmpty")}</p>
  return (
    <div className="flex flex-col gap-2">
      {ordered.map((session) => {
        const online = isOnline(session, now)
        const streaming = (session.inFlight ?? 0) > 0
        const tokens = session.input_tokens + session.output_tokens
        return (
          <div
            key={session.key_id}
            className={cn(
              "activity-row relative overflow-hidden flex flex-col gap-1 rounded-md border px-3 py-2",
              leaving.some((row) => row.key_id === session.key_id) && "activity-row-out",
            )}
          >
            {streaming ? <span className="activity-scan rounded-md" aria-hidden /> : null}
            <div className="flex flex-wrap items-center justify-between gap-x-3 gap-y-1">
              <span className="flex min-w-0 items-center gap-2">
                <span
                  className={cn("size-2 shrink-0 rounded-full", online ? "animate-pulse bg-state-healthy" : "bg-muted-foreground/40")}
                  title={online ? t("portal.activity.online") : t("portal.activity.offline")}
                />
                <span className="min-w-0 truncate text-sm">{session.label ?? session.prefix ?? `#${session.key_id}`}</span>
                {session.prefix ? <span className="truncate font-mono text-xs text-muted-foreground">{session.prefix}</span> : null}
                {streaming ? (
                  <span className="rounded bg-state-healthy/10 px-1.5 py-0.5 text-[0.65rem] text-state-healthy">
                    {t("portal.activity.inFlightOnSession", { count: session.inFlight })}
                  </span>
                ) : null}
              </span>
              <span className="font-mono text-xs text-muted-foreground">
                {online
                  ? (session.tokens_per_second > 0 ? t("portal.activity.sessionSpeed", { speed: session.tokens_per_second.toFixed(1) }) : t("portal.activity.online"))
                  : t("portal.activity.offline")}
                {" · "}{agoLabel(session.last_at, now, i18n.language, "")}
              </span>
            </div>
            <div className="flex flex-wrap items-center justify-between gap-x-3 gap-y-1">
              <span className="flex flex-wrap gap-1">
                {session.models.map((model) => (
                  <span key={model} className="rounded bg-muted px-1.5 py-0.5 font-mono text-[0.65rem] text-muted-foreground">{model}</span>
                ))}
              </span>
              <span className="flex items-center gap-2">
                {online && samples.get(session.key_id)?.length ? (
                  <Sparkline samples={samples.get(session.key_id) ?? []} online={online} />
                ) : null}
                <span className="font-mono text-xs text-muted-foreground">
                  {t("portal.activity.sessionSummary", { requests: session.requests, tokens: formatCount(tokens, i18n.language) })}
                </span>
              </span>
            </div>
          </div>
        )
      })}
    </div>
  )
}

function ModelList({ models, now }: { models: Array<{ model: string; provider: string | null; requests: number; input_tokens: number; output_tokens: number; last_at: number }>; now: number | undefined }) {
  const { t, i18n } = useTranslation()
  if (models.length === 0) return <p className="text-sm text-muted-foreground">{t("portal.activity.modelsEmpty")}</p>
  const busiest = Math.max(1, ...models.map((model) => model.requests))
  return (
    <div className="flex flex-col gap-2">
      {models.map((model) => (
        <div key={`${model.provider ?? ""}/${model.model}`} className="activity-row flex flex-col gap-1">
          <div className="flex flex-wrap items-center justify-between gap-x-3 gap-y-1 text-sm">
            <span className="min-w-0 truncate font-mono text-xs">{model.model}</span>
            <span className="font-mono text-xs text-muted-foreground">
              {t("portal.activity.sessionRequests", { count: model.requests })} · {agoLabel(model.last_at, now, i18n.language, "")}
            </span>
          </div>
          <div className="flex items-center gap-2">
            <div className="h-1.5 min-w-0 flex-1 overflow-hidden rounded-full bg-muted">
              <div className="h-full rounded-full bg-primary transition-[width] duration-500" style={{ width: `${Math.max(2, (model.requests / busiest) * 100)}%` }} />
            </div>
            <span className="shrink-0 font-mono text-xs text-muted-foreground">{formatCount(model.input_tokens + model.output_tokens, i18n.language)}</span>
          </div>
        </div>
      ))}
    </div>
  )
}

function agoLabel(lastAt: number | undefined, now: number | undefined, language: string, fallback: string): string {
  if (!lastAt || !now) return fallback
  const seconds = Math.max(0, Math.round(now - lastAt))
  return new Intl.RelativeTimeFormat(language, { numeric: "auto" }).format(-seconds, "second")
}
