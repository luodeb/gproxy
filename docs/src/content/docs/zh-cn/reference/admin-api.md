---
title: "管理 API 与 MCP"
description: "/admin/api 控制面、其认证模型，以及把控制面暴露为工具、供 AI 代理使用的进程内 MCP 服务"
---

运维控制台做的每一件事，都是通过调用 `/admin/api/**` 完成的。这套 JSON
接口就是完整的控制面：控制台本身只是一个瘦客户端，同样的路由也可以由脚本、
CI、curl 或内置的 **MCP 服务**驱动。

## 认证

浏览器路径与命令行路径最终落到同一段认证代码，只要满足其中一种凭证即可通过。

| 凭证 | 请求头 | 说明 |
| --- | --- | --- |
| 用户 API key | `Authorization: Bearer sk-gp-…` | key 必须属于 `is_admin` 用户。key 认证**跳过同源检查**，因此非浏览器客户端可以执行写操作。 |
| 浏览器会话 | `Cookie: gproxy_admin_session=…` | 由 `POST /admin/api/login` 签发，有效期 12 小时。写操作额外要求 `Origin` 与 `Host` 一致。 |

`GET /admin/api/session` 无需凭证即可访问，用于查询初始化状态；
`POST /admin/api/setup` 完成首次初始化。

因为 key 携带其属主的全部权限，推荐做法是为代理单独建号：新建用户、标记
`is_admin`、为该代理签发一把专用 key；需要切断代理时吊销这把 key 即可。
这样审计记录里会是一个独立的操作主体，而不是与人类运维者共用身份。

## 路由形态

接口是 CRUD 形态，路径使用 kebab-case。

| 操作 | 路由 |
| --- | --- |
| 列表 | `GET /admin/api/{entity}` |
| 新建 | `POST /admin/api/{entity}` → 201 |
| 更新 | `PATCH /admin/api/{entity}/{id}` |
| 删除 | `DELETE /admin/api/{entity}/{id}` |
| 批量 | `POST /admin/api/batch/{entity}`，body 为 `{"action": "enable" \| "disable" \| "delete", "ids": [...]}` |

`PATCH` 是**整体替换**语义：要发送**完整**对象，而不是部分字段补丁。
先读取记录，改掉要改的字段，再整体发回。

通用 CRUD 路由之外还有单例与动作路由：用量报表、请求日志、审计、实例/日志/门户
设置、配置导入导出、连通性与模型测试、配额探测，以及各渠道的登录流程。
`GET /admin/api/channels`、`/tls-presets`、`/rule-presets` 描述当前构建
支持的能力。

`/admin/api/batch/{entity}` 返回逐 id 的结果，因此部分成功也会显式可见，
而不会被静默吞掉。

### 不属于控制面

`/admin/api/native/**`（自更新与自启动）由宿主二进制在 admin 分发器之前处理。
它**不会**通过 MCP 暴露——代理无法借助这些工具重启或替换正在运行的二进制。

## MCP 服务

网关内嵌了一个
[Model Context Protocol](https://modelcontextprotocol.io) 服务，地址为：

```
POST /admin/api/mcp
```

它使用 streamable HTTP 传输，并要求与其包装的管理 API 相同的管理员凭证。
`GET /admin/api/openapi.json` 提供按目录生成的 OpenAPI 3.1 描述。

工具调用是**进程内**通过管理 API 分发的：调用方的凭证被原样重放，调用会经过
与直接 API 请求完全相同的认证、授权、校验、审计与配额记账流程。不存在需要
同步维护的第二条控制通路。

### 工具

| 工具 | 用途 |
| --- | --- |
| `gproxy_endpoints` | 列出可调用路由，可按实体、方法或关键字过滤。 |
| `gproxy_openapi` | 获取 OpenAPI 文档，或单个路由的定义。 |
| `gproxy_request` | 兜底通道：直接调用任意管理路由（`method`、`path`、`query`、`body`、`headers`）。 |
| `gproxy_list` / `gproxy_get` | 读取某个实体，或其中一条记录。 |
| `gproxy_create` / `gproxy_update` / `gproxy_delete` | 写入一条记录。 |
| `gproxy_batch` | 对多个 id 执行 `enable` / `disable` / `delete`。 |
| `gproxy_usage` | 聚合、分页、汇总或趋势查询用量。 |
| `gproxy_logs` | 列出请求日志、获取单次请求的完整交换、读取或更新抓取设置。 |
| `gproxy_audit` | 读取审计记录。 |
| `gproxy_settings` | 读取或更新实例、日志与门户设置。 |
| `gproxy_transfer` | 导出或导入整份配置。 |
| `gproxy_diagnostics` | 运行连通性/模型测试，读取通道、预设、配额与周期目录。 |

`gproxy_request` 保证覆盖完整：即使没有专用工具，目录中的任何路由都可到达。
`/admin/api/mcp` 自身会被拒绝作为目标。

工具带有 MCP 注解，便于客户端在执行前提示。`GET` 形态的工具标记为
`readOnlyHint`；会替换或删除既有状态的写操作标记为 `destructiveHint`。
`gproxy_create` 是写操作但只做新增，因此两者都不标记。

### 配置

| 变量 | 默认 | 含义 |
| --- | --- | --- |
| `GPROXY_MCP_ALLOWED_HOSTS` | 未设置 | 传输层 DNS-rebinding 检查的允许列表，逗号分隔。未设置时接受任意 `Host`；当该端点可被直接访问、前面没有可信反向代理时，应当设置它。 |

请求体上限为 32 MiB，以便容纳配置导入；SSE 每 30 秒发送一次保活帧。

### 接入客户端

把任意 streamable-HTTP MCP 客户端指向该端点，并以 bearer token 方式提供管理员
key。以 `pi-mcp-adapter` 为例：

```json
{
  "mcpServers": {
    "gproxy": {
      "url": "https://ai.debin.cc/admin/api/mcp",
      "auth": "bearer",
      "bearerToken": "sk-gp-…"
    }
  }
}
```

`auth` 字段决定凭据策略，缺少它时适配器会把该服务当作匿名服务，token 不会
被发送。`bearerToken` 以 `!` 开头时会在连接时执行该命令，因此 key 可以只放在
文件里而不写进配置：

```json
"bearerToken": "!cat ~/.config/gproxy/agent.key"
```

走回环接口时，端点同样是 `http://127.0.0.1:58881/admin/api/mcp`。