import { useEffect, useState } from "react"
import { CopyIcon, EyeIcon, EyeOffIcon, LoaderCircleIcon } from "lucide-react"
import { useTranslation } from "react-i18next"
import { toast } from "sonner"
import type { UserKeyDto } from "@/generated/UserKeyDto"
import type { UserKeyRevealResponse } from "@/generated/UserKeyRevealResponse"
import { Button } from "@/components/ui/button"

export function KeySecretCell({ record, reveal, remaskMs = 15_000 }: { record: UserKeyDto; reveal: () => Promise<UserKeyRevealResponse>; remaskMs?: number }) {
  const { t } = useTranslation()
  const [secret, setSecret] = useState<string | null>(null)
  const [pending, setPending] = useState(false)

  useEffect(() => {
    if (secret == null) return
    const timer = window.setTimeout(() => setSecret(null), remaskMs)
    return () => window.clearTimeout(timer)
  }, [remaskMs, secret])

  const onReveal = async () => {
    setPending(true)
    try {
      setSecret((await reveal()).api_key)
      toast.success(t("portal.keys.revealed"))
    } catch {
      toast.error(t("portal.keys.revealError"))
    } finally {
      setPending(false)
    }
  }
  const copy = async () => {
    if (secret == null) return
    try {
      await navigator.clipboard.writeText(secret)
      toast.success(t("portal.keys.copied"))
    } catch {
      toast.error(t("portal.keys.copyError"))
    }
  }

  return (
    <div className="flex min-w-56 items-center gap-1.5">
      <code className="min-w-0 flex-1 truncate text-xs">{secret ?? t("portal.keys.masked", { prefix: record.prefix ?? "" })}</code>
      {secret ? (
        <>
          <Button size="icon-xs" variant="ghost" aria-label={t("portal.keys.copy")} onClick={() => void copy()}><CopyIcon /></Button>
          <Button size="icon-xs" variant="ghost" aria-label={t("portal.keys.remask")} onClick={() => setSecret(null)}><EyeOffIcon /></Button>
        </>
      ) : (
        <Button size="icon-xs" variant="ghost" aria-label={t(record.revealable ? "portal.keys.reveal" : "portal.keys.notRevealable")} disabled={!record.revealable || pending} onClick={() => void onReveal()}>
          {pending ? <LoaderCircleIcon className="animate-spin" /> : <EyeIcon />}
        </Button>
      )}
    </div>
  )
}
