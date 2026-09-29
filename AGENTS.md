# AGENTS.md

给在本仓库（`luodeb/gproxy`）工作的 AI 编码助手的项目说明与操作手册。

> 这是 upstream `LeenHawk/gproxy` 的 **fork**，由 luodeb 自行维护并用于生产。
> 本文件描述的是**我们自己的**约定、构建与部署方式，与 upstream 的 CI 流程不同。

---

## 1. 这是什么项目

**gproxy** 是一个**自托管 LLM API 网关**（Rust，AGPL-3.0-or-later）。
它把多家上游 LLM 供应商的凭证聚合成一个入口，对外提供统一的 API，
并负责路由、格式转换、限流、配额和计费。

一个可执行文件同时提供三样东西：

| 组件 | 路径 | 说明 |
|---|---|---|
| API 网关 | `/v1/*` | 客户端调用入口 |
| 运营控制台 | `/admin` | 管理员 Web UI |
| 用户门户 | `/portal` | 终端用户自助页面 |

### 能力概览

- **多协议接入**：OpenAI Chat Completions、OpenAI Responses、Claude Messages、Gemini GenerateContent（含流式）；支持跨格式转换。
- **上游池化**：API key / OAuth / cookie 多种凭证，支持刷新、健康检查、凭证轮换与故障转移。
- **模型名稳定**：对外模型名 → 路由 → 上游模型 的映射，换供应商不用改客户端。
- **权限与成本**：用户/组织/团队/权限、速率限制、消费配额、按维度定价。
- **全 UI 运维**：供应商、凭证、模型目录、规则、用量、配额历史都在控制台里管，不用改 JSON。

### 技术栈

- Rust workspace（13 个 crate），edition 2024。
- 前端控制台：`console/`，React 19 + Vite + pnpm 9.15.9（TypeScript）。
- 主二进制：`gproxy-host-axum`，`[[bin]] name = "gproxy"`。
- 前端通过 `rust-embed` **编译期嵌入**二进制（`crates/gproxy-host-axum/assets/web/`）。
- 存储：SQLite（默认）；可选 Redis / Upstash / PostgreSQL。

### 目录结构

```
crates/
  gproxy-host-axum/    # 可执行入口（HTTP 服务、静态资源、自更新）
  gproxy-app/          # 应用层：控制面、配置、bootstrap、定价快照
  gproxy-admin/        # 管理 API 的 DTO 与路由定义
  gproxy-core/         # 执行引擎（路由、模型刷新、计费管道）
  gproxy-channels/     # 各上游渠道的协议实现
  gproxy-channel-api/  # 渠道抽象与 endpoint 定义
  ...                  # （其余为存储、CLI 支撑等）
console/               # React 控制台前端
docs/                  # 文档（Astro）
scripts/               # 构建 / 打包 / 发布脚本
deploy/                # 部署素材
BUILD.local.md         # 本地构建与部署备忘（更细的踩坑记录）
```

---

## 2. 我们的生产环境

**gproxy 是 `https://ai.debin.cc` 的实际网关。**

```
客户端 (pi / opencode / codex / …)
   │  https://ai.debin.cc/v1/…
   ▼
Aliyun Caddy  (ai.debin.cc { reverse_proxy 127.0.0.1:56188 })
   │
   ▼
frps  (8.130.17.158:57000)
   │
   ▼
本机 frpc   (localIP=10.42.30.102, localPort=58881 → remotePort=56188)
   │
   ▼
gproxy  @ yocto:58881   ← 本仓库产物（docker 容器）
   │
   ├── zhipu     (open.bigmodel.cn)
   ├── aliyun    (token-plan.cn-beijing.maas.aliyuncs.com)
   └── trae-hub  (127.0.0.1:58880)
```

> 隧道由 **本机 frpc** 承载（store 模式，映射存于 `/root/.config/frpc/db.json`）。
yocto 侧原来的 frpc 容器已删除；`remotePort` 仍为 56188，Caddy 无需改动。

### 关键事实

| 项 | 值 |
|---|---|
| 部署主机 | `yocto`（10.42.30.102，Ubuntu 24.04，x86_64） |
| 安装目录 | `/home/yocto/gproxy`（二进制 `bin/gproxy`，数据 `data/gproxy.db`） |
| 运行方式 | **docker 容器 `gproxy`**（`network_mode: host`，supervisord 作为 PID 1，`restart: unless-stopped`） |
| 镜像 / 编排 | `/home/yocto/gproxy/container/`（`Dockerfile`、`supervisord.conf`、`docker-compose.yml`） |
| 监听端口 | **58881** |
| 管理台 | `http://10.42.30.102:58881/admin`（用户 `admin`） |
| 上游 trae-hub | 同机 58880 |
| 其它容器 | `trae-hub`、`merged-proxy`、`x-kernel-jenkins` |
| 旧 systemd 单元 | `/home/yocto/.config/systemd/user/gproxy.service`（已 stop + disable，仅作回滚） |

### 环境变量（由 `container/.env` 经 `env_file` 注入，**不入库**）

| 变量 | 说明 |
|---|---|
| `GPROXY_HOST` / `GPROXY_PORT` | 监听地址与端口（生产为 `0.0.0.0` / `58881`） |
| `GPROXY_DATA_DIR` | 数据目录（生产 `/home/yocto/gproxy/data`） |
| `GPROXY_MASTER_KEY` | **数据库封存密钥**（32 字节 base64）——不是登录凭证 |
| `GPROXY_ADMIN_USER` / `GPROXY_ADMIN_PASSWORD` | 管理台账号；**每次启动都会重置该用户密码** |
| `GPROXY_BOOTSTRAP_ADMIN_API_KEY` | **仅用于首次初始化**；store 非空时忽略。移除它不影响已有 key |
| `GPROXY_UPDATE_CHANNEL_SERVE` | 更新通道 |

**凭证种类别混淆：**

- 管理台用户名/密码 → 登录 Web UI。
- 用户 API key（`sk-…`）→ 调 `/v1/*`。每个客户端一把（`sk-gp-*`，带 label）。
- `GPROXY_MASTER_KEY` → 只用于封存数据库里的密钥，**不能用来调用 API**。

### ⚠️ 已知陷阱

1. **`GPROXY_MASTER_KEY` 必须在首次启动前设好。**
   若先以明文模式初始化，之后再加 key 会启动失败：
   `Error: Encryption("store requires plaintext mode, but GPROXY_MASTER_KEY is set")`。
   补救：把 `data/` 改名留档、建空目录重启（会重新生成 admin 密码与 API key）。

2. **`GPROXY_ADMIN_PASSWORD` 会覆盖现有密码。**
   在 UI 里改过密码后，如果 compose/unit 里的环境变量没同步改，重启会被改回去。

3. **同一时间只能有一个实例绑定 58881。**
   容器与 systemd --user 单元同时启会端口冲突。切换时务必先停另一个。

---

## 3. 构建

> 本机（Debian 13, x86_64）**没有 docker**，upstream CI 用的 GitHub Actions + `cross` 在这里不可用，
> 因此我们用 **zig 作为 musl 交叉工具链**，产物是**静态链接**的二进制。

### 一键脚本（推荐）

```sh
scripts/local-deploy.sh            # 构建 + 部署到 yocto + 健康检查
scripts/local-deploy.sh build      # 只构建
scripts/local-deploy.sh --skip-console   # 前端没改时跳过前端构建
scripts/local-deploy.sh --release  # 构建 + 部署 + 在 fork 上建 GitHub Release
scripts/local-deploy.sh --help     # 全部选项
```

脚本会：检查工具链 → （可选）构建前端 → zig 构建静态 musl → scp 到 yocto（先备份旧二进制，原子替换）→ 重启远端运行实例（容器 / systemd 二选一自动识别）→ 健康检查。

### 手动构建（脚本内部做的事）

```sh
# 1) 前端（必须先做，产物编译期嵌入二进制）
pnpm --dir console install --frozen-lockfile
pnpm --dir console build
touch crates/gproxy-host-axum/src/static_assets.rs   # 触发重新嵌入

# 2) 静态 musl 二进制
export PATH=/tmp/zig-x86_64-linux-0.15.2:/tmp/zigbin:$PATH
export CC_x86_64_unknown_linux_musl=/tmp/zigbin/x86_64-linux-musl-gcc
export CXX_x86_64_unknown_linux_musl=/tmp/zigbin/x86_64-linux-musl-g++
export AR_x86_64_unknown_linux_musl=/tmp/zigbin/x86_64-linux-musl-ar
cargo zigbuild --locked --release -p gproxy-host-axum --target x86_64-unknown-linux-musl
# → target/x86_64-unknown-linux-musl/release/gproxy  (~49MB, static)
```

### 前置依赖

| 依赖 | 版本 | 备注 |
|---|---|---|
| rustc / cargo | **>= 1.98** | 1.97 会失败：`wreq@0.16.1 requires rustc 1.98` |
| cargo-zigbuild | — | `cargo install cargo-zigbuild --locked` |
| zig | 0.15.2 | `/tmp/zig-x86_64-linux-0.15.2/zig`（脚本可自动下载） |
| pnpm | 9.15.9 | `npm i -g pnpm@9.15.9` |
| node | >= 22 | 构建 console |
| ar / ranlib | 系统 binutils | **不要用 zig 的 ar** |

`BUILD.local.md` 里有更完整的 zig shim 生成步骤。

### 六个必须记住的构建陷阱

1. **rustc 必须 >= 1.98**（`wreq` 要求）。
2. **先前端后 Rust**。否则 `/admin` 会 404。前端改动后需 `touch static_assets.rs` 才会重新嵌入（体积会从 ~38MB 变 ~49MB）。
3. **zig 的 `ar` 不兼容 BoringSSL**：`ar: error: expected [relpos] for 'a', 'b', or 'i' modifier`。必须指向系统 `/usr/bin/ar`。
4. **CMake 需要真实文件形式的 `x86_64-linux-musl-g++`**，且**不能用符号链接**（`cat >` 会跟随链接，把所有 shim 覆盖成同一内容）。
5. **切换 CC 后要清 CMake 缓存**：`rm -rf target/*/release/build/aws-lc-sys-* target/*/release/build/btls-sys-*`。
6. **不要手写 `CARGO_TARGET_*_LINKER`** 指向 gcc shim（会重复引入 CRT，报 `duplicate symbol: _start`）；交给 `cargo zigbuild` 处理链接。

---

## 4. 部署

`scripts/local-deploy.sh` 已封装全流程；手动等价操作：

```sh
scp target/x86_64-unknown-linux-musl/release/gproxy yocto:/home/yocto/gproxy/bin/gproxy.tmp
ssh yocto 'chmod 755 /home/yocto/gproxy/bin/gproxy.tmp && mv -f /home/yocto/gproxy/bin/gproxy.tmp /home/yocto/gproxy/bin/gproxy'
ssh yocto 'cd /home/yocto/gproxy/container && docker compose up -d --force-recreate gproxy'
```

运维命令：

```sh
ssh yocto 'docker ps --filter name=gproxy'                 # 容器状态
ssh yocto 'docker logs -f gproxy'                          # 日志
ssh yocto 'docker exec gproxy supervisorctl status'        # gproxy 子进程状态
ssh yocto 'docker restart gproxy'                          # 重启容器
ssh yocto '/home/yocto/gproxy/bin/gproxy --version'        # 版本
```

回滚：`bin/` 下保留了最近 5 份 `gproxy.bak-<时间戳>`，直接 `mv` 回去再 `docker compose up -d --force-recreate` 即可。
回滚到 systemd 裸进程：`docker compose down` → `systemctl --user enable --now gproxy.service`（先停容器，避免端口冲突）。

> yocto 上**二进制由本机 scp 过去**（静态 musl），不在远端构建。

---

## 5. 版本与发布

- 版本号在 `Cargo.toml` 的 `[workspace.package].version`，`scripts/release-metadata.sh` 读取。
- tag 形如 `v3.0.22`，必须与 workspace 版本一致。
- upstream 的 `scripts/release.sh` / `release.yml` 走 GitHub Actions + `cross` 全平台矩阵；
  **我们的 fork 不用它**——用 `scripts/local-deploy.sh --release` 直接上传本地 zig musl 产物到 fork 的 Release。

### 与 upstream 同步
```sh
git fetch upstream
git merge upstream/main        # 或 git rebase upstream/main
git push fork main
```

远端约定：**`fork` = 我们的仓库（`luodeb/gproxy`）**，**`upstream` = 原作者（`LeenHawk/gproxy`）**。
向 upstream 提 PR 时注意：与本地部署相关的改动（如 `scripts/local-deploy.sh`、`AGENTS.md`、`BUILD.local.md`）
属于 fork 专属内容，通常不必上游化。

---

## 6. 数据与配置

- 生产数据全部在 `/home/yocto/gproxy/data/gproxy.db`（SQLite）。**改动前先备份。**
- 配置通过管理台 UI 或 admin API 修改，不直接编辑数据库。
- 管理 API：路径用 kebab-case（`providers`、`credentials`、`routes`、`model-aliases`、`user-keys`、`price-rules`…）。
  - `POST` 返回 **201**。
  - `PATCH /admin/api/providers/{id}` **需要完整 body**，只发部分字段会被忽略/报 400。
  - 用 admin API key 走 Bearer 认证可跳过同源检查。

### 重要配置开关

- `providers.settings.auto_refresh_models`（默认 `true`）：
  会向上游拉取模型列表并 fan-out 到 `/v1/models`。若要 `/v1/models` 只返回对外模型，
  在每个 provider 上设为 `false`。
- `endpoint_overrides`：`custom` 渠道默认用朴素字符串拼接 URL
  （`crates/gproxy-channels/src/shared/http.rs` 的 `join()` = `base.trim_end_matches('/') + path`），
  上游路径不规整时用 `settings.endpoints` 覆盖（如 `openai_chat_completions`、`claude_messages`）。
- 定价：内置目录优先级约 `999xxx`，自定义规则用 **priority 0** 即可覆盖（解析按 `(priority,id)` 升序取首个匹配）。
  模型匹配 `crates/gproxy-app/src/model_pattern.rs`：**精确匹配、大小写敏感**，支持 `*`，匹配对象是**上游模型名**。

---

## 7. 给 AI 助手的工作约定

- **改动保持可回滚**：先备份再改（数据库、二进制、系统配置）。
- **不要为了“干净”而删除生产数据**；拿不准就先留档改名。
- 改动前端后记得走 `pnpm --dir console build` + `touch static_assets.rs`，否则页面不更新。
- 破坏性操作（删容器/镜像/目录、轮换密钥）**先确认再执行**，并在事后核对残留。
- 参考 `BUILD.local.md`（本地构建/部署）与 `docs/`（upstream 功能文档）。