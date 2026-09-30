import { useTranslation } from "react-i18next"
import { KeyManagement } from "@/components/portal/key-management"

export function PortalKeysPage() {
  const { t } = useTranslation()

  return (
    <>
      <section className="flex flex-col gap-2">
        <h1 className="text-2xl font-semibold tracking-tight">{t("portal.keys.title")}</h1>
        <p className="max-w-3xl text-sm leading-6 text-muted-foreground">{t("portal.keys.description")}</p>
      </section>
      <div className="mt-6">
        <KeyManagement />
      </div>
    </>
  )
}