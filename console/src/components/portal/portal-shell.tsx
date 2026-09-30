import { useState, type ReactNode } from "react"
import { useTranslation } from "react-i18next"
import {
  CableIcon,
  ChartNoAxesCombinedIcon,
  ChevronsUpDownIcon,
  KeyRoundIcon,
  LayoutDashboardIcon,
  LogOutIcon,
  PanelLeftCloseIcon,
  PanelLeftOpenIcon,
  SettingsIcon,
  ShieldCheckIcon,
  UserIcon,
} from "lucide-react"
import { Button } from "@/components/ui/button"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { Sheet, SheetContent, SheetDescription, SheetHeader, SheetTitle } from "@/components/ui/sheet"
import { LocaleControls } from "@/components/locale-controls"
import { PasswordForm } from "@/components/portal/password-form"
import { usePortalSession } from "@/lib/portal-session-context"
import { SidebarResizeHandle } from "@/components/sidebar-resize-handle"
import { useSidebarPreferences } from "@/components/use-sidebar-preferences"
import { navigatePortalPath, portalPath, type PortalRoute } from "@/lib/portal-route"
import { cn } from "@/lib/utils"

const PORTAL_SIDEBAR_KEY = "gproxy.portal.sidebar.preferences"

const items: Array<{ route: PortalRoute; icon: typeof LayoutDashboardIcon }> = [
  { route: "overview", icon: LayoutDashboardIcon },
  { route: "connect", icon: CableIcon },
  { route: "usage", icon: ChartNoAxesCombinedIcon },
  { route: "keys", icon: KeyRoundIcon },
  { route: "sessions", icon: ShieldCheckIcon },
]

export function PortalShell({ route, children }: { route: PortalRoute; children: ReactNode }) {
  const { t } = useTranslation()
  const { context, logout } = usePortalSession()
  const sidebar = useSidebarPreferences(PORTAL_SIDEBAR_KEY)
  const [settingsOpen, setSettingsOpen] = useState(false)

  return (
    <div className="min-h-screen lg:grid" style={{ gridTemplateColumns: `${sidebar.collapsed ? 64 : sidebar.width}px minmax(0, 1fr)` }}>
      <aside className="relative border-b bg-card lg:sticky lg:top-0 lg:h-screen lg:border-r lg:border-b-0">
        <div className="flex h-full flex-col">
          <header className="flex items-center justify-between gap-2 px-3 py-4">
            <div className={cn("flex min-w-0 items-center gap-2", sidebar.collapsed && "lg:sr-only")}>
              {/* Brand mark: the GPROXY globe, the same asset the favicon serves. */}
              <img src="/favicon-96x96.png" className="size-7 shrink-0 rounded" width={28} height={28} alt="" />
              <div className="min-w-0">
                <p className="text-base font-bold tracking-wide">{t("portal.brand")}</p>
                <p className="text-xs text-muted-foreground">{t("portal.surface")}</p>
              </div>
            </div>
            <Button className="hidden lg:inline-flex" size="icon-sm" variant="ghost" aria-label={t(sidebar.collapsed ? "portal.nav.open" : "portal.nav.close")} onClick={sidebar.toggle}>{sidebar.collapsed ? <PanelLeftOpenIcon aria-hidden /> : <PanelLeftCloseIcon aria-hidden />}</Button>
          </header>
          <nav className="flex gap-1 overflow-x-auto overflow-y-hidden p-2 lg:flex-1 lg:flex-col" aria-label={t("portal.nav.label")}>
            {items.map(({ route: itemRoute, icon: Icon }) => (
              <Button key={itemRoute} variant={route === itemRoute ? "secondary" : "ghost"} aria-current={route === itemRoute ? "page" : undefined} aria-label={t(`portal.nav.${itemRoute}`)} className={cn("h-9 justify-start gap-3 px-3", sidebar.collapsed && "lg:justify-center", route === itemRoute && "font-medium")} onClick={() => navigatePortalPath(portalPath(itemRoute))}>
                <Icon className="size-4 shrink-0" aria-hidden />
                <span className={cn(sidebar.collapsed && "lg:sr-only")}>{t(`portal.nav.${itemRoute}`)}</span>
              </Button>
            ))}
          </nav>
          <UserMenu collapsed={sidebar.collapsed} username={context.user_name} onLogout={logout} onSettings={() => setSettingsOpen(true)} />
        </div>
        {!sidebar.collapsed ? <SidebarResizeHandle label={t("portal.nav.resize")} width={sidebar.width} minWidth={sidebar.minWidth} maxWidth={sidebar.maxWidth} onWidth={sidebar.setWidth} onReset={sidebar.resetWidth} /> : null}
      </aside>
      <main className="min-w-0 px-4 py-6 sm:px-6 lg:px-8 lg:py-8">{children}</main>
      <Sheet open={settingsOpen} onOpenChange={setSettingsOpen}>
        <SheetContent className="w-full gap-6 overflow-y-auto sm:max-w-md">
          <SheetHeader>
            <SheetTitle>{t("portal.settings.title")}</SheetTitle>
            <SheetDescription>{t("portal.settings.description")}</SheetDescription>
          </SheetHeader>
          <p className="text-sm text-muted-foreground">{t("portal.settings.account", { name: context.user_name })}</p>
          <div className="flex flex-col gap-3">
            <div>
              <p className="text-sm font-medium">{t("portal.settings.appearance")}</p>
              <p className="text-xs text-muted-foreground">{t("portal.settings.appearanceDescription")}</p>
            </div>
            <LocaleControls />
          </div>
          <PasswordForm />
        </SheetContent>
      </Sheet>
    </div>
  )
}

function UserMenu({ collapsed, username, onLogout, onSettings }: { collapsed: boolean; username: string; onLogout: () => void; onSettings: () => void }) {
  const { t } = useTranslation()
  const initial = username.trim().charAt(0).toUpperCase() || "?"
  return (
    <div className="border-t p-2">
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button variant="ghost" className={cn("h-11 w-full justify-start gap-3 px-2", collapsed && "lg:justify-center")} aria-label={t("portal.account.openMenu")}>
            <span className="grid size-7 shrink-0 place-items-center rounded-full bg-muted font-mono text-xs font-semibold" aria-hidden>{initial}</span>
            <span className={cn("min-w-0 flex-1 truncate text-left text-sm", collapsed && "lg:sr-only")}>{username}</span>
            <ChevronsUpDownIcon className={cn("size-4 shrink-0 text-muted-foreground", collapsed && "lg:sr-only")} aria-hidden />
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="start" side="top" className="w-56">
          <DropdownMenuLabel className="flex items-center gap-2 font-normal">
            <UserIcon className="size-4 text-muted-foreground" aria-hidden />
            <span className="truncate font-mono text-xs">{username}</span>
          </DropdownMenuLabel>
          <DropdownMenuSeparator />
          <DropdownMenuItem onSelect={onSettings}>
            <SettingsIcon aria-hidden />
            {t("portal.account.settings")}
          </DropdownMenuItem>
          <DropdownMenuItem variant="destructive" onSelect={onLogout}>
            <LogOutIcon aria-hidden />
            {t("portal.account.logout")}
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
      <p className={cn("hidden px-2 pt-1 font-mono text-[0.65rem] text-muted-foreground lg:block", collapsed && "lg:hidden")}>{buildIdentity()}</p>
    </div>
  )
}

function buildIdentity() {
  const build = window.__GPROXY_BUILD_INFO__
  return build
    ? `${build.version} · ${build.buildHash.slice(0, 12)}`
    : `${__GPROXY_VERSION__} · ${__GPROXY_BUILD_HASH__}`
}