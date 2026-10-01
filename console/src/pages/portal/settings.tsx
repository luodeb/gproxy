import { useTranslation } from "react-i18next"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { LocaleControls } from "@/components/locale-controls"
import { PasswordForm } from "@/components/portal/password-form"
import { usePortalSession } from "@/lib/portal-session-context"

export function PortalSettingsPage() {
  const { t } = useTranslation()
  const { context } = usePortalSession()

  return (
    <>
      <section className="flex flex-col gap-2">
        <h1 className="text-2xl font-semibold tracking-tight">{t("portal.settings.title")}</h1>
        <p className="max-w-3xl text-sm leading-6 text-muted-foreground">{t("portal.settings.description")}</p>
      </section>
      <div className="mt-6 flex max-w-2xl flex-col gap-6">
        <Card>
          <CardHeader>
            <CardTitle>{t("portal.settings.accountTitle")}</CardTitle>
            <CardDescription>{t("portal.settings.account", { name: context.user_name })}</CardDescription>
          </CardHeader>
        </Card>
        <Card>
          <CardHeader>
            <CardTitle>{t("portal.settings.appearance")}</CardTitle>
            <CardDescription>{t("portal.settings.appearanceDescription")}</CardDescription>
          </CardHeader>
          <CardContent>
            <LocaleControls />
          </CardContent>
        </Card>
        <PasswordForm />
      </div>
    </>
  )
}
