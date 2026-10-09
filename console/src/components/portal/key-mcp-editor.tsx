import { useTranslation } from "react-i18next"
import type { PortalKeyMcpGrantDto } from "@/generated/PortalKeyMcpGrantDto"
import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"
import { Input } from "@/components/ui/input"

export type McpServerTools = { name: string; enabled: boolean; tools: Array<string> }

/// Editor for one key's MCP grants: a server is on or off, and an enabled server
/// carries the tool-name patterns the key may call. Tool names from the last
/// discovery are offered as one-click chips, so the common case (grant these two
/// tools) never requires typing a pattern by hand.
export function KeyMcpEditor({ servers, grants, onChange }: {
  servers: Array<McpServerTools>
  grants: Array<PortalKeyMcpGrantDto>
  onChange: (next: Array<PortalKeyMcpGrantDto>) => void
}) {
  const { t } = useTranslation()
  const grantOf = (server: string) => grants.find((grant) => grant.server === server)

  const toggleServer = (server: string, on: boolean) => onChange(
    on
      ? [...grants, { server, patterns: ["*"] }]
      : grants.filter((grant) => grant.server !== server),
  )
  const setPatterns = (server: string, patterns: Array<string>) => onChange(
    grants.map((grant) => (grant.server === server ? { ...grant, patterns } : grant)),
  )
  const addPattern = (server: string, pattern: string) => {
    const grant = grantOf(server)
    if (!grant || grant.patterns.includes(pattern)) return
    setPatterns(server, [...grant.patterns, pattern])
  }
  const removePattern = (server: string, index: number) => {
    const grant = grantOf(server)
    if (!grant) return
    setPatterns(server, grant.patterns.filter((_, position) => position !== index))
  }

  if (servers.length === 0) {
    return <p className="text-sm text-muted-foreground">{t("portal.keys.mcpEmpty")}</p>
  }

  return (
    <div className="flex flex-col gap-2">
      {servers.map((server) => {
        const grant = grantOf(server.name)
        return (
          <div key={server.name} className="flex flex-col gap-3 rounded-md border px-3 py-2">
            <label className="flex items-center gap-3">
              <Checkbox checked={grant != null} onCheckedChange={(value) => toggleServer(server.name, value === true)} />
              <span className="min-w-0 truncate text-sm">{server.name}</span>
              {server.enabled ? null : <span className="text-xs text-muted-foreground">{t("portal.keys.mcpServerDisabled")}</span>}
              <span className="ml-auto font-mono text-xs text-muted-foreground">{t("portal.keys.mcpToolCount", { count: server.tools.length })}</span>
            </label>
            {grant ? (
              <div className="flex flex-col gap-2">
                {grant.patterns.map((pattern, index) => (
                  <div key={`${pattern}-${index}`} className="flex items-center gap-2">
                    <Input
                      className="h-8 font-mono text-xs"
                      value={pattern}
                      aria-label={t("portal.keys.mcpPatternLabel", { server: server.name })}
                      placeholder={t("portal.keys.mcpPatternPlaceholder")}
                      onChange={(event) => setPatterns(server.name, grant.patterns.map((value, position) => (position === index ? event.target.value : value)))}
                    />
                    <Button
                      type="button"
                      size="sm"
                      variant="ghost"
                      aria-label={t("portal.keys.mcpRemovePattern", { pattern })}
                      onClick={() => removePattern(server.name, index)}
                    >
                      {t("portal.keys.mcpRemove")}
                    </Button>
                  </div>
                ))}
                <div className="flex flex-wrap items-center gap-1">
                  <Button type="button" size="xs" variant="outline" onClick={() => addPattern(server.name, "")}>
                    {t("portal.keys.mcpAddPattern")}
                  </Button>
                  {server.tools
                    .filter((tool) => !grant.patterns.includes(tool))
                    .map((tool) => (
                      <Button key={tool} type="button" size="xs" variant="ghost" className="font-mono" onClick={() => addPattern(server.name, tool)}>
                        {tool}
                      </Button>
                    ))}
                </div>
              </div>
            ) : null}
          </div>
        )
      })}
      <p className="text-xs text-muted-foreground">{t("portal.keys.mcpPatternHint")}</p>
    </div>
  )
}
