---
title: "Provider 与凭证"
description: "渠道、Provider、凭证池、登录流程、令牌刷新、健康状态跟踪，以及通过管理 API 提供的 Provider 工具"
---

**渠道（channel）** 是编译进二进制的适配器，对应一类上游 API。**Provider**
是使用某个渠道的一条已保存连接：名称、设置和一组凭证。同一渠道可以创建任意
多个 Provider，例如 `openai-main` 和 `openai-eu` 都使用 `openai` 渠道。

通过管理 API 保存的修改对新请求立即生效，无需重启。

## 渠道

渠道列表来自正在运行的二进制：`GET /admin/api/channels`。每个渠道声明显示名称、能服务的路由、
接受的 Provider 设置、所需的凭证字段，以及是否提供登录流程。28 个渠道 id
按凭证形态分组如下：

| 凭证形态 | 渠道 id |
| --- | --- |
| API 密钥（`api_key`） | `aistudio`、`azure`、`claudeapi`、`cloudflare-ai-gateway`、`custom`、`dashscope`、`deepseek`、`nvidia`、`openai`、`openrouter`、`vercel`、`vertexexpress`、`xai` |
| API 密钥或 OAuth 令牌 | `cline`、`kimi`、`opencode` |
| OAuth 令牌（`access_token`、`refresh_token`） | `claudecode`、`codex`、`grokbuild`、`kiro`、`workbuddy` |
| Google OAuth（`access_token`、`refresh_token`、`project_id`） | `antigravity`、`geminicli` |
| Google 服务账号（`client_email`、`private_key`、`project_id`） | `vertex` |
| AWS（`api_key`，或 `access_key_id` + `secret_access_key` + 可选 `session_token`） | `aws-bedrock` |
| GitHub 令牌（`github_token`） | `copilotcli` |
| 浏览器 Cookie（`cookie`、`account_uuid`） | `claudeweb`（仅 native 构建） |

导入时会规范化两组 v2 id：`kimiapi`、`kimicode` 变为 `kimi`；`opencodezen`、
`opencodego` 变为 `opencode`，并把 `tier` 设为 `zen` 或 `go`。`claudeweb`
不会编译进 edge 构建。

## Provider 字段

| 字段 | 含义 |
| --- | --- |
| 路由名（`name`） | 唯一标识，同时也是 URL 中的命名前缀，例如 `/openai-main/v1/chat/completions`。 |
| 显示名称（`label`） | 可选，仅供操作者自己参考。 |
| 渠道 | 上表中的某个 id，创建后不可更改。 |
| 凭证策略 | `round_robin`（默认）或 `sticky`，见下文。 |
| Provider 代理 URL | 覆盖实例代理；凭证可以再次覆盖。 |
| 客户端指纹 | 可选的 TLS/HTTP 配置：预设或自定义 JSON。凭证可以覆盖。 |
| 转发元数据 | 允许哪些调用方请求头和 Query 参数发往上游、哪些响应头返回给调用方。默认值来自渠道。 |
| 启用 | 禁用的 Provider 退出路由。 |

### 渠道设置

设置保存为一个 JSON 对象。渠道为常用键声明了类型化字段；其余内容直接编辑
Provider 的 `settings` JSON。

| 键 | 渠道 | 含义 |
| --- | --- | --- |
| `base_url` | 全部 | 没有精确端点覆盖时使用的上游地址。 |
| `auto_refresh_models` | 全部 | 客户端列出模型时向该 Provider 拉取实时模型列表。默认 `true`。 |
| `endpoints` | 全部 | 按操作类型指定精确 URL，见下文。 |
| `enable_openai_magic_cache` | `openai`、`codex`、`azure`、`custom`、`openrouter`、`vercel`、`opencode` | 在 OpenAI 目标上识别缓存触发字符串。 |
| `enable_claude_magic_cache` | `claudeapi`、`claudecode`、`azure`、`custom`、`openrouter`、`vercel`、`aws-bedrock`、`opencode` | 在 Claude 目标上识别缓存触发字符串。 |
| `claude_fallback_mode`、`claude_fallback_models` | `claudeapi`、`claudecode`、`custom`、`openrouter`、`vercel` | `off`、`default`，或 `models` 加一个有序模型列表。 |
| `region`、`video_output_s3_uri` | `aws-bedrock` | AWS 区域；生成视频的 S3 目标地址。 |
| `region`、`profile_arn`、`auth_base_url` | `kiro` | AWS 区域、Profile ARN、认证服务地址。 |
| `location`、`oauth_client_id`、`oauth_client_secret`、`oauth_token_url` | `vertex`、`antigravity`、`geminicli` | Google Cloud 区域与 OAuth 客户端覆盖。 |
| `tier`、`console_base_url` | `opencode` | `zen` 或 `go`；设备登录使用的厂商地址。 |

精确端点覆盖放在 `endpoints` 下，按操作类型作键，优先于 `base_url`。不会再
追加路径；`{model}` 会替换为上游模型 id：

```json
{
  "endpoints": {
    "openai_chat_completions": "https://api.example/v1/chat/completions",
    "gemini_generate_content": "https://api.example/v1beta/models/{model}:generateContent"
  }
}
```

凭证的 `secret` 接受该渠道能服务的任意类型。常见键有 `openai_chat_completions`、
`openai_responses`、`claude_messages`、`gemini_generate_content`、
`gemini_stream_generate_content`、`openai_list_models`、`openai_embeddings`、
`image_generations`。

## 凭证

凭证属于一个 Provider，包含：

| 字段 | 含义 |
| --- | --- |
| 标签 | 可选，建议用来区分上游账号。 |
| 类型 | `api_key`、`oauth` 或 `cookie`，记录密文的取得方式。 |
| 密文 | 直接粘贴单个密钥，或填写包含渠道声明字段的 JSON 对象。编辑时会预填已存储的密文；留空则保留原值。 |
| 流量权重 | 默认 100，决定在凭证池中的流量份额。 |
| 每分钟请求数、每分钟 Token 数 | 可选的单凭证上限。 |
| 代理覆盖 URL | 替代 Provider 和实例代理。 |
| 客户端指纹 | 替代 Provider 指纹。 |
| 启用 | 禁用的凭证会被跳过。 |

配置 master key 后，密文在静态存储中密封（见
[配置](/zh-cn/reference/configuration/)）。读回已存储的密文是一项单独的、会被审计的
操作（`POST /admin/api/credentials/<id>/reveal`）。

### 凭证池策略

选择是确定性的计数轮转，绝不随机。

| 策略 | 行为 |
| --- | --- |
| `round_robin` | 每个请求推进对应路由成员的计数器；计数器按权重比例选出一个凭证。 |
| `sticky` | 槽位由调用方的 API 密钥（若客户端带会话 id，则由会话 id）推导，因此同一密钥在凭证池不变时始终落在同一凭证。 |

### 单凭证限流

RPM 与 TPM 通过缓存后端按固定 60 秒窗口执行。TPM 在发往上游之前用分词器
阶梯统计请求的输入 Token。超过任一上限的请求以 `429` 失败，且不消耗窗口。
这些限制保护的是上游账号；调用方限流见
[权限、限流与配额](/zh-cn/guides/permissions/)。

### 健康状态

健康状态按（凭证，上游模型）记录，来源是上游响应：`2xx` 记为健康，`429`
和 `5xx` 记为降级，`401`–`403` 记为失效。令牌刷新失败会把整个凭证记为降级；
密文格式错误则记为失效。同一层级内降级凭证排在健康凭证之后；失效凭证从计划
中移除。新观测覆盖旧观测，针对旧凭证版本记录的观测会被忽略，因此重新保存
密文后会从零开始。`GET /admin/api/credentials` 报告当前最差状态、异常模型、
最近一次状态码与详情；`POST /admin/api/credentials/<id>/health-reset` 清除它。

### 令牌刷新

持有 OAuth 令牌的渠道会声明密文何时到期。刷新在缓存中一个 60 秒的独占租约
下运行，并发请求只刷新一次；其余请求每秒轮询，直到拿到新版本。轮换后的
密文以版本保护方式持久化。Claude 每次刷新都会轮换 refresh token，这条路径
绝不能丢失任何一次写入。

## 登录流程

两个渠道可以通过登录路由获取首个凭证：

| 渠道 | 模式 |
| --- | --- |
| `codex` | 浏览器登录（授权码 + PKCE）、设备代码 |
| `claudecode` | 浏览器登录（授权码 + PKCE）、浏览器 Cookie（仅 native 构建） |

- **浏览器登录**：GPROXY 生成带 S256 PKCE challenge 和 state 的授权 URL
  （`POST /admin/api/login/authcode/start`）。在浏览器中授权后，用完整回调 URL
  完成登录（`POST /admin/api/login/authcode/complete`）。代码交换后保存 access 与
  refresh 令牌。
- **设备代码**：`POST /admin/api/login/device/start` 返回用户代码和厂商验证页面，
  `/device/poll` 轮询直到厂商报告已批准或已拒绝。
- **浏览器 Cookie**：`POST /admin/api/login/cookie` 接受完整 `Cookie` 请求头或
  `sessionKey` 值。GPROXY 发现
  组织、用 Cookie 完成 OAuth 交换，并把 Cookie 密封保存，供令牌过期后重新
  登录。

其他渠道都通过粘贴密钥或令牌创建凭证。

## 凭证费用额度

为凭证创建一条 `quotas` 行即可分别设置总、月、周、日美元额度
（`POST /admin/api/quotas`，`subject_kind` 为 `"credential"`、`subject_id` 为凭证 id，
字段为 `quota_total`、`quota_monthly`、`quota_weekly`、`quota_daily`）。
留空表示不限，0 立即禁止付费请求。任一额度用尽，就不再向该凭证发送新的
付费请求；路由可切换到其他凭证，全部候选凭证额度耗尽时返回 HTTP 402。
免费操作仍可使用。

费用按网关模型定价累计，从配置后开始计算；受限凭证缺少模型定价时拒绝
付费请求。累计独立于用量日志持久化，关闭限制仍继续累计，修改额度不会
清空已用金额。总额度不自动重置；日、周、月额度分别在每天零点、周一零点、
每月 1 日零点重置（UTC），门户按查看者的本地时区显示恢复时间。

付费请求发出前，会先按模型价格估算输入 token 的费用并在该凭证的额度窗口
里预留，因此并发请求彼此可见，合起来最多只会超出"估算与结算之差"；请求结算
或中止时释放预留。输出 token 只有结算时才知道，所以很长的回复仍可能在额度
之上完成。这些计数不包含绕过网关的调用，也不会与上游账单自动对账。

## 工具

- **连通性测试**：`POST /admin/api/connectivity/test` 经由所选作用域实际生效的代理探测
  `https://1.1.1.1/cdn-cgi/trace`（IPv4 与 IPv6），报告出口 IP、位置、延迟，
  以及使用了哪个代理来源。
- **上游额度**：对暴露该信息的渠道（`codex`、`claudecode`、`geminicli`），
  `GET /admin/api/credentials/<id>/quota` 报告观测到的配额窗口、已用百分比和
  周期结束时间。`POST /admin/api/credentials/<id>/quota-probe` 实时探测上游账号；
  Codex 重置卡可用 `/quota-reset` 兑现。有效窗口达到 90% 及以上
  的凭证在同一故障转移层级中排在其他凭证之后，达到 100% 则排在最后。
- **批量操作**：`POST /admin/api/batch/providers` 与
  `/admin/api/batch/credentials` 对多条记录执行 `enable`、`disable` 或 `delete`，
  并逐项返回结果。
- **导出 / 导入**：配置导出涵盖身份、Provider、凭证、密钥、配额、价格、
  路由、别名和规则。除非明确要求，否则不含密文；含密文的导出会记录其为明文
  还是密封，以及所用密钥的指纹。导入时会用本地密钥重新密封，并报告跳过的
  凭证和密钥数量。

模型、从上游拉取以及模型测试见[模型、路由与别名](/zh-cn/guides/models/)。
