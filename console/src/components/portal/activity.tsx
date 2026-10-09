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

/// The live view: requests in flight right now (id, key, model, how long they
/// have been running), the last minute's traffic, and per-key throughput.
/// Polls on an interval — numbers move on their own while the page sits open.
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

  const live = (activity.data?.in_flight ?? 0) > 0
  const tiles: Array<[string, string]> = [
    [t("portal.activity.inFlight"), String(activity.data?.in_flight ?? 0)],
    [t("portal.activity.requestsPerMinute"), formatCount(activity.data?.last_minute_requests ?? 0, i18n.language)],
    [t("portal.activity.tokensPerMinute"), formatCount(activity.data?.last_minute_tokens ?? 0, i18n.language)],
    [t("portal.activity.activeSessions"), String(activity.data?.sessions.length ?? 0)],
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
            <h3 className="text-sm font-medium">{t("portal.activity.requestsTitle")}</h3>
            <RequestList requests={activity.data?.requests ?? []} recent={activity.data?.recent ?? []} />
          </section>

          <div className="grid gap-6 lg:grid-cols-2">
            <section className="flex flex-col gap-2">
              <h3 className="text-sm font-medium">{t("portal.activity.sessionsTitle")}</h3>
              <SessionList sessions={activity.data?.sessions ?? []} now={activity.data?.now} />
            </section>
            <section className="flex flex-col gap-2">
              <h3 className="text-sm font-medium">{t("portal.activity.modelsTitle")}</h3>
              <ModelList models={activity.data?.models ?? []} now={activity.data?.now} />
            </section>
          </div>
        </QueryState>
      </CardContent>
    </Card>
  )
}

type LiveRequest = { request_id: string; prefix: string | null; label: string | null; model: string | null; started_at: number }
type FinishedRequest = { request_id: string; prefix: string | null; label: string | null; model: string; provider: string | null; input_tokens: number; output_tokens: number; at: number; latency_ms: number }

/// One list, two states: what is running now (pulsing, counting up) and what
/// just finished (dimmed, with how long it took). The finished rows are what
/// keep the panel informative between polls — most requests are over in a
/// second or two, so an in-flight-only list is usually empty.
function RequestList({ requests, recent }: { requests: Array<LiveRequest>; recent: Array<FinishedRequest> }) {
  const { t, i18n } = useTranslation()
  if (requests.length === 0 && recent.length === 0) return <p className="text-sm text-muted-foreground">{t("portal.activity.requestsEmpty")}</p>
  const now = Date.now()
  return (
    <div className="flex flex-col gap-1">
      {requests.map((request) => {
        const tail = request.request_id.split("-").pop() ?? request.request_id
        const elapsed = Math.max(0, Math.round((now - request.started_at) / 1000))
        return (
          <div key={`live-${request.request_id}`} className="flex flex-wrap items-center gap-x-3 gap-y-1 rounded-md border px-3 py-2">
            <span className="flex size-2 shrink-0 animate-pulse rounded-full bg-state-healthy" aria-hidden />
            <span className="font-mono text-xs text-muted-foreground" title={request.request_id}>…{tail}</span>
            <span className="min-w-0 truncate text-sm">{request.label ?? request.prefix ?? t("portal.keys.unnamed")}</span>
            <span className="min-w-0 truncate font-mono text-xs">{request.model ?? "—"}</span>
            <span className="ml-auto font-mono text-xs text-muted-foreground">{t("portal.activity.elapsedSeconds", { count: elapsed })}</span>
          </div>
        )
      })}
      {recent.map((request) => {
        const tail = request.request_id.split("-").pop() ?? request.request_id
        const tokens = request.input_tokens + request.output_tokens
        return (
          <div key={`done-${request.request_id}`} className="flex flex-wrap items-center gap-x-3 gap-y-1 rounded-md border px-3 py-2 opacity-70">
            <span className="size-2 shrink-0 rounded-full bg-muted-foreground/40" aria-hidden />
            <span className="font-mono text-xs text-muted-foreground" title={request.request_id}>…{tail}</span>
            <span className="min-w-0 truncate text-sm">{request.label ?? request.prefix ?? t("portal.keys.unnamed")}</span>
            <span className="min-w-0 truncate font-mono text-xs">{request.model}</span>
            {request.provider ? <span className="truncate text-xs text-muted-foreground">{request.provider}</span> : null}
            <span className="ml-auto font-mono text-xs text-muted-foreground">
              {t("portal.activity.finishedMeta", { latency: request.latency_ms, tokens: formatCount(tokens, i18n.language) })} · {agoLabel(request.at, Math.floor(now / 1000), i18n.language, "")}
            </span>
          </div>
        )
      })}
    </div>
  )
}

function SessionList({ sessions, now }: { sessions: Array<PortalActivitySessionDto>; now: number | undefined }) {
  const { t, i18n } = useTranslation()
  if (sessions.length === 0) return <p className="text-sm text-muted-foreground">{t("portal.activity.sessionsEmpty")}</p>
  return (
    <div className="flex flex-col gap-2">
      {sessions.map((session) => (
        <div key={session.key_id} className="flex flex-col gap-1 rounded-md border px-3 py-2">
          <div className="flex flex-wrap items-center justify-between gap-x-3 gap-y-1">
            <span className="flex min-w-0 items-center gap-2">
              <span className="min-w-0 truncate text-sm">{session.label ?? session.prefix ?? `#${session.key_id}`}</span>
              {session.prefix ? <span className="truncate font-mono text-xs text-muted-foreground">{session.prefix}</span> : null}
            </span>
            <span className="font-mono text-xs text-muted-foreground">
              {t("portal.activity.sessionRequests", { count: session.requests })} · {agoLabel(session.last_at, now, i18n.language, "")}
            </span>
          </div>
          <div className="flex flex-wrap items-center justify-between gap-x-3 gap-y-1">
            <span className="flex flex-wrap gap-1">
              {session.models.map((model) => (
                <span key={model} className="rounded bg-muted px-1.5 py-0.5 font-mono text-[0.65rem] text-muted-foreground">{model}</span>
              ))}
            </span>
            <span className="font-mono text-xs text-muted-foreground">{t("portal.activity.sessionSpeed", { speed: session.tokens_per_second.toFixed(1) })}</span>
          </div>
        </div>
      ))}
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
