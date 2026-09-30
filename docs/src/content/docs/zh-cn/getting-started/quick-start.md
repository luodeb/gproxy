---
title: 快速开始
description: "从下载的二进制到可用网关：启动 gproxy、创建管理员、添加 Provider 与路由、签发密钥并发送请求。"
---

本页把一个全新的原生安装带到第一个成功的请求。假设你使用的是[下载](/zh-cn/getting-started/downloads/)
页上的便携压缩包；安装包会替你完成第 1、2 步并打开门户。

## 1. 启动 gproxy

```bash
chmod +x ./gproxy
./gproxy
```

服务监听 `127.0.0.1:8787`，创建 `./data/gproxy.db`，并输出 `GPROXY listening`。
`gproxy --help` 列出全部参数。每个参数都有对应的 `GPROXY_*` 环境变量，两者都可以
写进工作目录或数据目录下的 `.env` 文件。优先级依次为参数、环境变量、`./.env`、
`<数据目录>/.env`、默认值。

一个最小的 `.env`：

```env
GPROXY_HOST=127.0.0.1
GPROXY_PORT=8787
GPROXY_DATA_DIR=./data
GPROXY_MASTER_KEY=<标准 base64，32 字节>
```

用 `openssl rand -base64 32` 生成密钥。不设置时，凭证和用户密钥以明文保存。请在
添加第一个凭证之前设置它；之后再改属于轮换操作，见[配置](/zh-cn/reference/configuration/)。
安装包会替你写好带有生成密钥的 `.env`。

容器方式：

```bash
docker run -d --name gproxy -p 8787:8787 \
  -v gproxy-data:/app/data \
  ghcr.io/leenhawk/gproxy:<tag>
```

## 2. 创建管理员

全新存储在 `GET /admin/api/session` 中报告 `setup_required: true`。用
`POST /admin/api/setup` 传入用户名和密码创建管理员；它会打开会话并登录。
此后管理 API 就是下面一切的控制面；用户门户仍在 `/portal`。

管理员也可以通过 `GPROXY_ADMIN_PASSWORD` 创建，见[安装](/zh-cn/getting-started/installation/#首次启动)。

## 3. 添加 Provider 和凭证

用 `POST /admin/api/providers` 创建 Provider：路由名（这个 Provider 的稳定标识，
也可用作命名模式的路径前缀）、通道，以及凭证策略——`round_robin` 在凭证池中轮转
请求，`sticky` 让每个客户端密钥固定使用一个凭证。通道决定适用哪些设置，例如
`custom` 的 `base_url` 或 `aws-bedrock` 的 `region`。保存 Provider 时会预置该通道
的路由规则，并为它创建一个名为 `<provider> · defaults` 的空私有规则集。

然后用 `POST /admin/api/credentials` 创建凭证，传入 Provider 的 `id`。提供密钥有两
种方式：

- **直接粘贴。** 把 `kind` 设为 `api_key`、`oauth` 或 `cookie`，填写通道声明的字
  段，或在 `secret` 中填入原始凭证对象。标签可选，默认从密钥推导。
- **登录。** 声明了登录方式的通道使用登录路由：浏览器登录（授权码加 PKCE）用
  `POST /admin/api/login/authcode/start` 与 `/authcode/complete`，设备代码用
  `/device/start` 与 `/device/poll`，粘贴浏览器 Cookie 用 `/cookie`。`codex` 提供
  浏览器登录和设备代码；`claudecode` 提供浏览器登录和浏览器 Cookie。开始登录，在
  浏览器中批准，然后用回调 URL 或设备代码完成登录。token 加密保存，由 GPROXY 在
  独占租约下自行刷新。

每个凭证都带有流量权重、可选的每分钟请求数和每分钟 token 数限制、代理覆盖，以
及观测到的健康状态。

可以选择用 `POST /admin/api/models/discover` 询问该 Provider 提供的模型 id，再把
它们记录为 `provider-models` 行（`POST /admin/api/provider-models`），连同能力和
默认价格。

## 4. 创建路由

用 `POST /admin/api/routes` 创建路由：名称和最大尝试次数（首次尝试加故障转移次
数）。然后用 `POST /admin/api/route-members` 添加成员：`provider_id`、
`upstream_model` id、可选的固定凭证，以及故障转移层级和权重。第 0 层用尽后第 1 层
才会接到流量；权重在同层健康成员之间分流。再从其他 Provider 添加成员用于故障转移。

创建路由并不会把它对外公开。用 `POST /admin/api/model-aliases` 创建一个模型别名，
把公开模型名绑定到该路由；客户端在 `model` 中发送的就是这个名字。聚合解析依次经过
别名、变体后缀、公开模型名，再到路由的成员。路由名本身只能通过命名前缀
`/{route}/v1/...` 访问。

## 5. 创建用户和 API 密钥

用 `POST /admin/api/users` 创建用户。密码可选，只有需要登录门户时才用到。然后用
`POST /admin/api/user-keys` 创建密钥，传入 `user_id`：标签、前缀——`sk` 供 API 客户
端使用，`at` 供 Codex CLI 的 access-token 登录使用——以及可选的 `expires_at`。密钥
显示时立即复制。之后列表只显示前缀；`POST /admin/api/user-keys/<id>/reveal` 会再次
返回完整密钥，这是一项单独的、会被审计的操作。

权限默认拒绝。用 `POST /admin/api/permissions` 创建一条：效果（允许或拒绝）、针对
所有 Provider 或某一个、针对所有操作或某一个操作组。它可以挂在密钥、用户、团队或
组织上并向下继承。没有任何允许权限时，该密钥的每个请求都会被 `403` 拒绝。限流和
成本配额分别通过各自的路由创建。

## 6. 发送请求

把占位符替换为密钥和公开模型名：

```bash
curl http://127.0.0.1:8787/v1/chat/completions \
  -H "Authorization: Bearer sk-<your-key>" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "<public-model-name>",
    "messages": [
      { "role": "user", "content": "Say hello in one short sentence." }
    ]
  }'
```

响应带有 `x-request-id` 头。`GET /admin/api/logs` 列出该请求，
`GET /admin/api/logs/<request_id>` 返回它产生的上游调用。

## 下一步

- [发送第一个请求](/zh-cn/getting-started/first-request/)展示同一调用在每种接受
  格式下的写法、流式、模型列表和命名前缀。
- 设置了密码的用户可以登录 `/portal` 创建自己的密钥，并复制 curl、OpenAI、Claude、
  Gemini SDK、Codex CLI 和 Claude Code 的连接片段。见[门户与 Web 界面](/zh-cn/guides/console/)。
- [CLI 客户端](/zh-cn/guides/cli-clients/)介绍如何把 Codex CLI 和 Claude Code 指向
  网关。
