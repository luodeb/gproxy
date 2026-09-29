# 本地构建 / 部署备忘（luodeb fork）

> **日常直接用一键脚本即可**：`scripts/local-deploy.sh`（构建 + 部署 + 健康检查）。
> 本文件保留手动步骤与踩坑细节，供排查问题或改脚本时参考。
> 项目总说明见 `AGENTS.md`。

本文件记录**本机（Debian 13, x86_64）**构建 gproxy 的完整流程，以及部署到 **yocto**（10.42.30.102, Ubuntu 24.04）的步骤。
upstream 的 CI 用 GitHub Actions + `cross`，本机没有 docker，因此用 **zig 作为 musl 交叉工具链**。

## 0. 目录与远端

```
/root/codes/gproxy
  fork      https://github.com/luodeb/gproxy.git   ← 我们维护，main 跟踪它
  upstream  https://github.com/LeenHawk/gproxy.git ← 原作者，用于同步
```

同步上游：

```sh
cd /root/codes/gproxy
git fetch upstream
git merge upstream/main        # 或 git rebase upstream/main
git push fork main
```

## 1. 前置依赖

| 依赖 | 版本 | 说明 |
|---|---|---|
| rustc / cargo | **>= 1.98** | 1.97 会失败：`wreq@0.16.1 requires rustc 1.98` |
| pnpm | 9.15.9 | 构建 console 前端（`npm i -g pnpm@9.15.9`） |
| node | 22+（本机 26） | console 构建 |
| zig | 0.15.2 | 交叉编译 musl 的 C/C++ 工具链（BoringSSL 需要 C++） |
| cmake | 任意 | BoringSSL 构建 |
| ar / ranlib | 系统 binutils | **不要用 zig ar**，见下文坑 3 |

升级 rustc：

```sh
rustup update stable     # 已配置 rsproxy 镜像，RUSTUP_DIST_SERVER=https://rsproxy.cn
```

cargo 依赖走 rsproxy 镜像（`/root/.cargo/config.toml` 已配置）。

## 2. 重要：构建顺序

前端产物通过 `rust-embed` **编译期固化**进二进制（`crates/gproxy-host-axum/assets/web/`）。
**必须先构建前端、再构建 Rust**，否则 `/admin` 会 404。

```sh
cd /root/codes/gproxy/console
pnpm install --frozen-lockfile
pnpm build            # tsconfig + vite build + scripts/sync-to-embed.mjs → 同步进 assets/web
```

## 3. 构建静态 musl 二进制（推荐，给 yocto 用）

本机 glibc 2.41 > yocto 2.39，gnu 版直接拷过去虽能跑（实测只需 GLIBC_2.38），
但 **musl 静态版无 glibc 耦合**，是长期维护的正确选择。

准备 zig 工具链（一次性）：

```sh
# 下载 zig
cd /tmp && curl -sLO https://ziglang.org/download/0.15.2/zig-x86_64-linux-0.15.2.tar.xz
tar xf zig-x86_64-linux-0.15.2.tar.xz     # → /tmp/zig-x86_64-linux-0.15.2/zig

# 生成 cc/c++ 包装脚本
# 关键点：cargo 传给 CC 的是 Rust triple（x86_64-unknown-linux-musl），
# zig 只认自己的 triple（x86_64-linux-musl），必须翻译。
mkdir -p /tmp/zigbin
ZIG=/tmp/zig-x86_64-linux-0.15.2/zig
cat > /tmp/zigbin/zigcc-realfile <<EOF
#!/bin/sh
args=""
for a in "\$@"; do
  case "\$a" in
    --target=x86_64-unknown-linux-musl|--target=x86_64-unknown-linux-gnu) args="\$args --target=x86_64-linux-musl" ;;
    *) args="\$args \$a" ;;
  esac
done
exec $ZIG cc \$args
EOF
sed 's/ cc / c++ /' /tmp/zigbin/zigcc-realfile > /tmp/zigbin/zigcxx-realfile
chmod +x /tmp/zigbin/zigcc-realfile /tmp/zigbin/zigcxx-realfile
for n in gcc cc;   do cp /tmp/zigbin/zigcc-realfile  /tmp/zigbin/x86_64-linux-musl-$n; done
for n in g++ c++;   do cp /tmp/zigbin/zigcxx-realfile /tmp/zigbin/x86_64-linux-musl-$n; done
# 注意：ar/ranlib 用系统 binutils（zig ar 不兼容 BoringSSL 的构建）
printf '#!/bin/sh\nexec /usr/bin/ar "$@"\n'     > /tmp/zigbin/x86_64-linux-musl-ar
printf '#!/bin/sh\nexec /usr/bin/ranlib "$@"\n' > /tmp/zigbin/x86_64-linux-musl-ranlib
chmod +x /tmp/zigbin/*
```

构建：

```sh
cd /root/codes/gproxy
export PATH=/tmp/zig-x86_64-linux-0.15.2:/tmp/zigbin:$PATH
export CC_x86_64_unknown_linux_musl=/tmp/zigbin/x86_64-linux-musl-gcc
export CXX_x86_64_unknown_linux_musl=/tmp/zigbin/x86_64-linux-musl-g++
export AR_x86_64_unknown_linux_musl=/tmp/zigbin/x86_64-linux-musl-ar
export ZIG_GLOBAL_CACHE_DIR=/tmp/zig-cache
export ZIG_LOCAL_CACHE_DIR=/tmp/zig-cache-local

cargo zigbuild --release -p gproxy-host-axum --target x86_64-unknown-linux-musl
# 产物: target/x86_64-unknown-linux-musl/release/gproxy  (~49MB, static)
```

验证：

```sh
file target/x86_64-unknown-linux-musl/release/gproxy   # statically linked
ldd  target/x86_64-unknown-linux-musl/release/gproxy   # not a dynamic executable
```

### 踩过的坑

1. **rustc 必须 >= 1.98**（`wreq` 要求）。
2. **先前端后 Rust**——否则 `/admin` 404；改动前端后 cargo 不会自动重编，
   需 `touch crates/gproxy-host-axum/src/static_assets.rs` 才能重新嵌入（二进制会从 ~38MB 变 ~49MB）。
3. **zig ar 不能用**：BoringSSL 构建报 `ar: error: expected [relpos] for 'a', 'b', or 'i' modifier`。
   换系统 `/usr/bin/ar` 解决。
4. **CMake 找 `x86_64-linux-musl-g++`**：需要 PATH 里有该名字的脚本，且**不要用符号链接**
   （`cat >` 会跟随链接覆盖同一文件，导致所有 shim 都变成 strip 命令）。
5. **CMake 缓存**：切换 CC 后要删 `target/*/release/build/aws-lc-sys-*` 与 `btls-sys-*`。
6. **链接报 `duplicate symbol: _start`**：不要手写 `CARGO_TARGET_*_LINKER` 指向 gcc 包装脚本
   （会重复带入 CRT），交给 `cargo zigbuild` 自己处理链接。

## 4. 部署到 yocto

yocto 上没有外网，用 scp 拷静态二进制：

```sh
scp target/x86_64-unknown-linux-musl/release/gproxy yocto:/home/yocto/gproxy/bin/gproxy
```

systemd **user** 服务：`/home/yocto/.config/systemd/user/gproxy.service`（已 `loginctl enable-linger yocto`）：

```ini
[Unit]
Description=GPROXY LLM gateway
After=network-online.target

[Service]
Type=simple
WorkingDirectory=/home/yocto/gproxy
Environment=GPROXY_UPDATE_CHANNEL_SERVE=dev
Environment=GPROXY_HOST=0.0.0.0
Environment=GPROXY_PORT=58881
Environment=GPROXY_DATA_DIR=/home/yocto/gproxy/data
Environment=GPROXY_MASTER_KEY=<32字节 base64>
ExecStart=/home/yocto/gproxy/bin/gproxy
Restart=always
RestartSec=3

[Install]
WantedBy=default.target
```

管理：

```sh
ssh yocto 'systemctl --user restart gproxy.service'
ssh yocto 'journalctl --user -u gproxy.service -f'
```

### 坑：GPROXY_MASTER_KEY 与 plaintext 模式

首次启动若**未设** `GPROXY_MASTER_KEY`，DB 会以明文模式初始化；
之后再设 key 会启动失败：

```
Error: Encryption("store requires plaintext mode, but GPROXY_MASTER_KEY is set")
```

因此**第一次启动前就要设好 key**。若已经踩到，且库里没有数据，
把 `data/` 改名留档、建空目录重启即可（会重新生成 admin 密码/API key）。
老目录已在 yocto 留档为 `data.plaintext-<时间戳>`。

## 5. 端口占用（yocto）

| 端口 | 服务 |
|---|---|
| 58880 | trae-hub（本机上游） |
| **58881** | **gproxy（现役）** |
| 8088 | merged-proxy |
| 80/443 | 反代 |

> gproxy 直接监听 58881，与之前的网关同端口，反代配置无需改动。
