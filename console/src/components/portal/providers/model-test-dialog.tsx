import { useQuery } from "@tanstack/react-query"
import { LoaderCircleIcon } from "lucide-react"
import { useTranslation } from "react-i18next"
import type { ProviderModelDto } from "@/api/control"
import { testProviderModel } from "@/api/control"
import { Alert, AlertTitle } from "@/components/ui/alert"
import { Dialog, DialogBody, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { formatNumber } from "@/lib/format"

export function ModelTestDialog({ open, onOpenChange, providerId, providerName, model }: {
  open: boolean
  onOpenChange: (open: boolean) => void
  providerId: number
  providerName: string
  model: ProviderModelDto | null
}) {
  const { t } = useTranslation()
  const test = useQuery({
    queryKey: ["portal", "model-test", providerId, model?.model_id],
    queryFn: () => testProviderModel(providerId, model!.model_id),
    enabled: open && model != null,
    retry: 0,
    staleTime: 0,
  })
  const result = test.data

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent closeLabel={t("portal.common.close")}>
        <DialogHeader>
          <DialogTitle>{t("portal.providers.testTitle")}</DialogTitle>
          <DialogDescription>{providerName} · <span className="font-mono">{model?.model_id}</span></DialogDescription>
        </DialogHeader>
        <DialogBody>
          {test.isLoading ? (
            <div className="flex flex-col items-center gap-3 py-10 text-sm text-muted-foreground" aria-busy="true">
              <LoaderCircleIcon className="size-6 animate-spin" aria-hidden />
              <p>{t("portal.providers.testRunning")}</p>
            </div>
          ) : test.isError ? (
            <Alert variant="destructive"><AlertTitle>{t("portal.providers.testError")}</AlertTitle></Alert>
          ) : result ? (
            <div className="flex flex-col gap-4">
              {!result.ok ? <p className="text-sm text-destructive">{result.message ?? `HTTP ${result.status}`}</p> : null}
              <dl className="grid grid-cols-2 gap-px overflow-hidden rounded-lg border bg-border">
                <Metric label={t("portal.providers.ttft")} value={result.ttft_ms == null ? "—" : `${result.ttft_ms} ms`} />
                <Metric label={t("portal.providers.tokensPerSecond")} value={result.tokens_per_second == null ? "—" : `${formatNumber(result.tokens_per_second, "en")} tok/s`} />
                <Metric label={t("portal.providers.totalLatency")} value={`${result.latency_ms} ms`} />
                <Metric label={t("portal.providers.outputTokens")} value={String(result.output_tokens)} />
              </dl>
              {result.reply ? <p className="whitespace-pre-wrap break-words rounded-md bg-muted px-3 py-2 text-sm">{result.reply}</p> : null}
            </div>
          ) : null}
        </DialogBody>
      </DialogContent>
    </Dialog>
  )
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex flex-col gap-1 bg-card px-3 py-2">
      <dt className="text-xs text-muted-foreground">{label}</dt>
      <dd className="font-mono text-base">{value}</dd>
    </div>
  )
}
