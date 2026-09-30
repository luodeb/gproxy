import { useTranslation } from "react-i18next"
import { OAuthSessions } from "@/components/portal/oauth-sessions"

export function PortalSessionsPage() {
  const { t } = useTranslation()

  return (
    <>
      <section className="flex flex-col gap-2">
        <h1 className="text-2xl font-semibold tracking-tight">{t("portal.sessions.title")}</h1>
        <p className="max-w-3xl text-sm leading-6 text-muted-foreground">{t("portal.sessions.description")}</p>
      </section>
      <div className="mt-6">
        <OAuthSessions />
      </div>
    </>
  )
}