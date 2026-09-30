---
title: 用户门户与 Web 界面
description: "gproxy 二进制提供的唯一 Web 界面、门户各板块，以及它如何构建并嵌入二进制"
---

`gproxy` 二进制只提供**一个** React 应用——**用户门户**，位于 `/portal`
及其子路径。构建产物嵌入在二进制里，无需额外部署任何东西。

| 路径 | 界面 | API | 面向 |
| --- | --- | --- | --- |
| `/portal` 与 `/portal/*` | 用户门户 | `/portal/api/**` | 设有密码的用户 |
| `/` | 重定向到 `/portal` | 无 | 任何能访问该端口的人 |
| `/admin/*`（HTML） | 已移除 | 无 | 重定向到 `/portal` |
| `/admin/api/**` | 管理 API | `/admin/api/**` | 管理员、脚本、MCP 客户端 |

端口上的其余流量都是以 API 密钥认证的网关流量（见
[路由与端点](/zh-cn/reference/routing-table/)）。

> **本 fork 没有操作员控制台。** 管理 API 与其进程内的
> [MCP 服务端](/zh-cn/reference/admin-api/) 取代了它，因此二进制只打包门户的
> HTML。非 API 的 `/admin` 与 `/admin/*` 请求会返回 `302` 跳转到 `/portal`，
> 而不是 404——这些 URL 已经存在于人们的书签里。管理 API 行为完全不变。

## 首次启动

在管理员存在之前，`GET /admin/api/session` 返回 `setup_required: true`。
`POST /admin/api/setup` 接受一个用户名和密码，创建第一个管理员，打开会话并记
录一条 `auth.setup` 审计事件。该路由按来源地址限流为每分钟四次尝试。

如需跳过 API 调用，启动二进制时设置 `GPROXY_ADMIN_PASSWORD`（可选
`GPROXY_ADMIN_USER`，默认 `admin`）。账户在首次运行时创建，管理员 API 密钥自
动生成或取自 `GPROXY_BOOTSTRAP_ADMIN_API_KEY`，`GPROXY_BOOTSTRAP_CHANNELS`
可为列出的通道 id 创建空 Provider。引导密钥与通道只在首次运行时生效，但指定
管理员的密码会在每次启动时重新应用，因此登录后请移除
`GPROXY_ADMIN_PASSWORD`。见[配置](/zh-cn/reference/configuration/)。

## 管理员凭证

门户用密码登录用户。管理员通过以下两种方式之一访问 API：

| 凭证 | 请求头 | 说明 |
| --- | --- | --- |
| 用户 API 密钥 | `Authorization: Bearer sk-gp-…` | 密钥须属于标记为 `is_admin` 的用户。Bearer 调用不做同源检查，因此脚本与 MCP 客户端可以执行写操作。 |
| 浏览器会话 | `Cookie: gproxy_admin_session=…` | 由 `POST /admin/api/login` 签发，有效期 12 小时。写操作还要求 `Origin` 与 `Host` 匹配。 |

`POST /admin/api/login` 与 `/logout` 分别审计为 `auth.login` 和
`auth.logout`。路由目录与工具面见 [管理 API 与 MCP](/zh-cn/reference/admin-api/)。

## 用户门户

任何设有密码的用户都可以在 `/portal` 登录（`POST /portal/api/login`；Cookie
`gproxy_portal_session`，12 小时）。管理员通过管理 API 创建用户并设置初始密
码；用户在门户中修改密码。

侧边栏链接五个板块，每个都是可收藏的真实 URL。

| 板块 | 路径 | 作用 |
| --- | --- | --- |
| 概览 | `/portal` | 作用于账户的支出配额窗口；管理员启用后还会显示最近结算请求。 |
| 接入 | `/portal/connect` | 选择一个获准使用的模型，复制可直接运行的片段：curl、OpenAI Python、Claude Python、Gemini Python、Codex CLI 配置、Claude Code 环境变量。片段仅限该模型能服务的线上格式。片段下方是获准使用的模型目录。 |
| 用量 | `/portal/usage` | 1、7 或 30 天内已结算的请求数、输入、输出与缓存 Token 以及成本，与配额窗口并列显示。 |
| API 密钥 | `/portal/keys` | 创建前缀为 `sk`（API 客户端）或 `at`（Codex access-token 登录）的密钥并可加备注；密钥只显示一次。列出、显示并吊销自己的密钥。 |
| 授权会话 | `/portal/sessions` | 你以 OAuth 登录过的应用，含首次登录、最近刷新与过期时间。吊销后会立即停止其 access/refresh token。 |

密钥形如 `<prefix>-gp-<random>`。Codex 与 Claude Code 片段在
[CLI 客户端](/zh-cn/guides/cli-clients/)中说明。

### 设置

侧边栏底部显示当前登录用户。点击它会打开菜单：**设置** 会滑出抽屉，内含修改
密码表单与主题/语言控件；**退出登录** 用于登出。主题与语言保存在浏览器中
（`gproxy-console-theme`、`gproxy-console-lang`）；侧边栏自身的宽度与折叠状
态保存在 `gproxy.portal.sidebar.preferences`。

### 键盘与小屏幕

可打开详情的表格行和卡片可获得焦点，响应 Enter 和空格。侧边栏拖动柄接受方向
键、Home 和 End，并记住宽度。低于 `lg` 断点时侧边栏变为可横向滚动的条。

## 门户设置

门户唯一的设置——用户能否看到最近结算请求——位于 `GET` 与
`PATCH /admin/api/portal-settings`。

## 构建并嵌入门户

Web 应用位于 `console/`，用 pnpm 管理。

```bash
cd console
pnpm install
pnpm build      # tsc -b, vite build, then scripts/sync-to-embed.mjs
```

最后一步把 `console/dist/` 复制到 `crates/gproxy-host-axum/assets/web/`，由
`rust-embed` 编译进二进制。之后重新构建 `gproxy`。没有该目录也能构建二进制并
提供 API；页面请求返回
`web assets are not embedded; run pnpm build in console/ and rebuild gproxy`。

开发时 `pnpm dev` 启动 Vite，并把管理与门户 API 代理到运行中的后端。前端改动
以 `pnpm lint` 和 `pnpm test` 收尾。`console/src/generated/` 下的类型由 `ts-rs`
在 `cargo test` 期间从 Rust DTO 生成，绝不手工编辑；
`crates/gproxy-admin/src/dto/export.rs` 的导出列表已裁剪为门户用到的 DTO，
以免生成器产出无用文件。

嵌入的 `index.html` 服务于 `/`、`/portal` 以及任意 `/portal/<板块>` 深链接。
`/portal/api/**` 在静态处理之前就被分发，绝不会落回应用外壳。`/assets/` 下带
哈希的文件缓存一年，HTML 为 `no-cache`，`/build-info.js` 注入构建标识。