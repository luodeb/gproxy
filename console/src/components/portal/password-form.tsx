import { useState } from "react"
import { useMutation } from "@tanstack/react-query"
import { useTranslation } from "react-i18next"
import { portalChangePassword } from "@/api/portal"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Input } from "@/components/ui/input"

export function PasswordForm() {
  const { t } = useTranslation()
  const [currentPassword, setCurrentPassword] = useState("")
  const [newPassword, setNewPassword] = useState("")
  const [passwordDone, setPasswordDone] = useState(false)
  const password = useMutation({
    mutationFn: () => portalChangePassword({ current_password: currentPassword, new_password: newPassword }),
    onSuccess: () => {
      setCurrentPassword("")
      setNewPassword("")
      setPasswordDone(true)
    },
  })

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t("portal.password.title")}</CardTitle>
        <CardDescription>{t("portal.password.description")}</CardDescription>
      </CardHeader>
      <CardContent>
        <form className="flex flex-col gap-3" onSubmit={(event) => { event.preventDefault(); setPasswordDone(false); password.mutate() }}>
          <Input type="password" autoComplete="current-password" value={currentPassword} placeholder={t("portal.password.current")} required onChange={(event) => setCurrentPassword(event.target.value)} />
          <Input type="password" autoComplete="new-password" value={newPassword} placeholder={t("portal.password.new")} required onChange={(event) => setNewPassword(event.target.value)} />
          {passwordDone ? <p className="text-sm text-muted-foreground">{t("portal.password.saved")}</p> : null}
          <Button type="submit" disabled={password.isPending}>{t("portal.password.action")}</Button>
        </form>
      </CardContent>
    </Card>
  )
}