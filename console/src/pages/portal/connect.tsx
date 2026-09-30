import { useState } from "react"
import { useQuery } from "@tanstack/react-query"
import { useTranslation } from "react-i18next"
import { portalModels } from "@/api/portal"
import { QueryState } from "@/components/query-state"
import { ConnectionGuide } from "@/components/portal/connection-guide"
import { ModelCatalog } from "@/components/portal/model-catalog"

export function PortalConnectPage() {
  const { t } = useTranslation()
  const models = useQuery({ queryKey: ["portal", "models"], queryFn: ({ signal }) => portalModels(signal) })
  const [requestedModel, setRequestedModel] = useState<string | null>(null)
  const list = models.data ?? []
  const selectedModel = list.some((model) => model.name === requestedModel)
    ? requestedModel
    : list[0]?.name ?? null

  return (
    <>
      <section className="flex flex-col gap-2">
        <h1 className="text-2xl font-semibold tracking-tight">{t("portal.connect.title")}</h1>
        <p className="max-w-3xl text-sm leading-6 text-muted-foreground">{t("portal.connect.description")}</p>
      </section>
      <QueryState loading={models.isLoading} error={models.isError ? t("portal.models.loadError") : ""}>
        <div className="mt-6 flex flex-col gap-6">
          <ConnectionGuide
            origin={window.location.origin}
            apiKey={t("portal.connect.keyPlaceholder")}
            models={list}
            selectedModel={selectedModel}
            onModelChange={setRequestedModel}
          />
          <ModelCatalog models={list} />
        </div>
      </QueryState>
    </>
  )
}