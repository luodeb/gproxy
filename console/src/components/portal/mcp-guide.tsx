import { useTranslation } from "react-i18next"
import { toast } from "sonner"
import { Button } from "@/components/ui/button"
import { Card, CardAction, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { mcpSnippets } from "@/components/portal/connection-snippets"
import { CopySnippet } from "@/components/portal/copy-snippet"
import { copyText } from "@/lib/copy-text"

/// How to reach the gateway's MCP endpoint. Sits beside the model-API guide
/// because it is the same credential: one key, one origin, two protocols.
export function McpGuide({ origin, apiKey }: { origin: string; apiKey: string }) {
  const { t } = useTranslation()
  const endpoint = `${origin}/mcp`
  const snippets = mcpSnippets({ origin, key: apiKey })

  async function copyEndpoint() {
    try {
      await copyText(endpoint)
      toast.success(t("portal.connect.copied"))
    } catch {
      toast.error(t("portal.connect.copyError"))
    }
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t("portal.connect.mcpTitle")}</CardTitle>
        <CardDescription>{t("portal.connect.mcpDescription")}</CardDescription>
        <CardAction>
          <Button size="sm" variant="outline" onClick={() => void copyEndpoint()}>
            {t("portal.connect.mcpCopyEndpoint")}
          </Button>
        </CardAction>
      </CardHeader>
      <CardContent className="flex flex-col gap-5">
        <dl className="grid gap-1">
          <dt className="text-xs text-muted-foreground">{t("portal.connect.mcpEndpoint")}</dt>
          <dd className="break-all font-mono text-sm">{endpoint}</dd>
        </dl>
        <Tabs defaultValue={snippets[0].method}>
          <TabsList className="max-w-full overflow-x-auto overflow-y-hidden">
            {snippets.map((snippet) => (
              <TabsTrigger key={snippet.method} value={snippet.method}>
                {t(`portal.connect.mcpMethods.${snippet.method}`)}
              </TabsTrigger>
            ))}
          </TabsList>
          {snippets.map((snippet) => (
            <TabsContent key={snippet.method} value={snippet.method} className="pt-3">
              <CopySnippet
                title={t(`portal.connect.mcpMethods.${snippet.method}`)}
                display={snippet.display}
                copyPayload={snippet.copy}
              />
            </TabsContent>
          ))}
        </Tabs>
        <p className="text-xs text-muted-foreground">{t("portal.connect.mcpHint")}</p>
      </CardContent>
    </Card>
  )
}
