import { useEffect, useState } from "react"
import { useQuery } from "@tanstack/react-query"
import { useTranslation } from "react-i18next"
import { portalActivity } from "@/api/portal"
import type { PortalActivitySessionDto } from "@/generated/PortalActivitySessionDto"
import { QueryState } from "@/components/query-state"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import { formatCount } from "@/lib/format"

/// How often the panel refreshes. Fast enough to feel live, slow enough that a
/// room full of open overviews does not become the gateway's busiest client.
const REFRESH_MS = 3_000

/// A session counts as online until it has been quiet for this long. Shorter
/// than the aggregate window, so a pause between turns does not flash offline.
const ONLINE_SECONDS = 60

/// The live view, one row per session: which keys are talking, on which
/// models, how fast they generate, online until quiet for a minute. Polls on
/// an interval — numbers move on their own while the page sits open.
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

  const now = activity.data?.now
  const sessions = activity.data?.sessions ?? []
  const onlineCount = sessions.filter((session) => now != null && now - session.last_at < ONLINE_SECONDS).length
  const live = (activity.data?.in_flight ?? 0) > 0
  const tiles: Array<[string, string]> = [
    [t("portal.activity.inFlight"), String(activity.data?.in_flight ?? 0)],
    [t("portal.activity.requestsPerMinute"), formatCount(activity.data?.last_minute_requests ?? 0, i18n.language)],
    [t("portal.activity.tokensPerMinute"), formatCount(activity.data?.last_minute_tokens ?? 0, i18n.language)],
    [t("portal.activity.activeSessions"), String(onlineCount)],
  ]

  return (
    <Card>
      <CardHeader className="flex flex-row items-center justify-between gap-3">
        <CardTitle>{t("portal.activity.title")}</CardTitle>
        <span className="flex items-center gap-2 text-xs text-muted-foreground">
          <span className={`size-2 rounded-full ${live ? "animate-pulse bg-state-healthy" : "bg-muted-foreground/40"}`} aria-hidden />
          {t("portal.activity.refresh", { seconds: Math.round(REFRESH_MS / 1000) })}
        </span>
      </CardHeader>
      <CardContent className="flex flex-col gap-4">
        <QueryState loading={activity.isLoading} error={activity.isError ? t("portal.activity.loadError") : ""}>
          <dl className="grid gap-px overflow-hidden rounded-xl border bg-border grid-cols-2 lg:grid-cols-4">
            {tiles.map(([label, value]) => (
              <div key={label} className="flex flex-col gap-1 bg-card px-4 py-3">
                <dt className="text-xs text-muted-foreground">{label}</dt>
                <dd className="font-mono text-lg">{value}</dd>
              </div>
            ))}
          </dl>

          <section className="mt-4 flex flex-col gap-2">
            <h3 className="text-sm font-medium">{t("portal.activity.sessionsTitle")}</h3>
            <SessionList sessions={sessions} now={now} />
          </section>

          <section className="flex flex-col gap-2">
            <h3 className="text-sm font-medium">{t("portal.activity.modelsTitle")}</h3>
            <ModelList models={activity.data?.models ?? []} now={now} />
          </section>
        </QueryState>
      </CardContent>
    </Card>
  )
}

/// One row per session: identity, the models it touched, what it did in the
/// window, and its generating speed while online. A session stays online for
/// {@link ONLINE_SECONDS} after its last activity — the in-flight registry
/// keeps it online mid-request even before any usage row has landed.
function SessionList({ sessions, now }: { sessions: Array<PortalActivitySessionDto>; now: number | undefined }) {
  const { t, i18n } = useTranslation()
  if (sessions.length === 0) return <p className="text-sm text-muted-foreground">{t("portal.activity.sessionsEmpty")}</p>
  const sorted = [...sessions].sort((left, right) => {
    const leftOnline = now != null && now - left.last_at < ONLINE_SECONDS
    const rightOnline = now != null && now - right.last_at < ONLINE_SECONDS
    return leftOnline === rightOnline ? right.last_at - left.last_at : leftOnline ? -1 : 1
  })
  return (
    <div className="flex flex-col gap-2">
      {sorted.map((session) => {
        const online = now != null && now - session.last_at < ONLINE_SECONDS
        const tokens = session.input_tokens + session.output_tokens
        return (
          <div key={session.key_id} className="flex flex-col gap-1 rounded-md border px-3 py-2">
            <div className="flex flex-wrap items-center justify-between gap-x-3 gap-y-1">
              <span className="flex min-w-0 items-center gap-2">
                <span
                  className={`size-2 shrink-0 rounded-full ${online ? "animate-pulse bg-state-healthy" : "bg-muted-foreground/40"}`}
                  aria-label={online ? t("portal.activity.online") : t("portal.activity.offline")}
                  title={online ? t("portal.activity.online") : t("portal.activity.offline")}
                />
                <span className="min-w-0 truncate text-sm">{session.label ?? session.prefix ?? `#${session.key_id}`}</span>
                {session.prefix ? <span className="truncate font-mono text-xs text-muted-foreground">{session.prefix}</span> : null}
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
              <span className="font-mono text-xs text-muted-foreground">
                {t("portal.activity.sessionSummary", { requests: session.requests, tokens: formatCount(tokens, i18n.language) })}
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
        <div key={`${model.provider ?? ""}/${model.model}`} className="flex flex-col gap-1">
          <div className="flex flex-wrap items-center justify-between gap-x-3 gap-y-1 text-sm">
            <span className="min-w-0 truncate font-mono text-xs">{model.model}</span>
            <span className="font-mono text-xs text-muted-foreground">
              {t("portal.activity.sessionRequests", { count: model.requests })} · {agoLabel(model.last_at, now, i18n.language, "")}
            </span>
          </div>
          <div className="flex items-center gap-2">
            <div className="h-1.5 min-w-0 flex-1 overflow-hidden rounded-full bg-muted">
              <div className="h-full rounded-full bg-primary" style={{ width: `${Math.max(2, (model.requests / busiest) * 100)}%` }} />
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
