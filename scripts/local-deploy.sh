#!/usr/bin/env bash
#
# local-deploy.sh —— 一键构建 + 发布（luodeb fork 专用）
#
# 本机没有 docker（upstream CI 走 GitHub Actions + cross），因此用 zig 交叉编译
# 出 **静态 musl** 二进制，再 scp 到 yocto 并用 systemd --user 重启。
#
# 用法:
#   scripts/local-deploy.sh              # 构建 + 部署（默认）
#   scripts/local-deploy.sh build        # 只构建
#   scripts/local-deploy.sh deploy       # 只部署（用现有产物）
#   scripts/local-deploy.sh --release    # 构建 + 部署 + 在 fork 上创建 GitHub Release
#   scripts/local-deploy.sh --tag        # 打 tag 并推送（不建 Release）
#
# 常用开关:
#   --skip-console   跳过前端构建（前端没改时可省 ~1 分钟）
#   --no-restart     部署后不重启远端服务
#   --no-deploy      等价于 build
#   --dry-run        只打印将执行的动作
#   -h, --help       帮助
#
# 可覆盖的环境变量:
#   DEPLOY_HOST    远端 ssh 别名        默认 yocto
#   DEPLOY_DIR     远端安装目录         默认 /home/yocto/gproxy
#   DEPLOY_SERVICE 远端 systemd 单元    默认 gproxy.service
#   DEPLOY_PORT    健康检查端口          默认 58881
#   TARGET         Rust triple          默认 x86_64-unknown-linux-musl
#   ZIG_VERSION    zig 版本             默认 0.15.2
#
set -euo pipefail

root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
cd "$root"

DEPLOY_HOST="${DEPLOY_HOST:-yocto}"
DEPLOY_DIR="${DEPLOY_DIR:-/home/yocto/gproxy}"
DEPLOY_SERVICE="${DEPLOY_SERVICE:-gproxy.service}"
DEPLOY_PORT="${DEPLOY_PORT:-58881}"
TARGET="${TARGET:-x86_64-unknown-linux-musl}"
ZIG_VERSION="${ZIG_VERSION:-0.15.2}"
ZIG_ROOT="${ZIG_ROOT:-/tmp/zig-x86_64-linux-$ZIG_VERSION}"
ZIGBIN="${ZIGBIN:-/tmp/zigbin}"
FORK_REPO="${FORK_REPO:-luodeb/gproxy}"
RELEASE_NOTES_DIR="docs/release-notes"

mode=all
skip_console=false
restart=true
dry_run=false
do_tag=false
do_release=false

# ---------------------------------------------------------------- 工具函数
log()  { printf '\033[1;36m==>\033[0m %s\n' "$*"; }
ok()   { printf '\033[1;32m  ✓\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33m  !\033[0m %s\n' "$*" >&2; }
die()  { printf '\033[1;31m  ✗ %s\033[0m\n' "$*" >&2; exit 1; }

run() {
  if $dry_run; then printf '\033[2m  $ %s\033[0m\n' "$*"; return 0; fi
  "$@"
}

usage() { awk 'NR>2 && /^set -euo/{exit} NR>2{sub(/^# ?/,""); print}' "$0"; exit 0; }

# ---------------------------------------------------------------- 参数解析
while [ $# -gt 0 ]; do
  case "$1" in
    build)         mode=build ;;
    deploy)        mode=deploy ;;
    all)           mode=all ;;
    --skip-console) skip_console=true ;;
    --no-restart)  restart=false ;;
    --no-deploy)   mode=build ;;
    --tag)         do_tag=true ;;
    --release)     do_release=true; do_tag=true ;;
    --dry-run)     dry_run=true ;;
    -h|--help)     usage ;;
    *)             die "未知参数: $1（--help 查看用法）" ;;
  esac
  shift
done

# ---------------------------------------------------------------- 前置检查
check_toolchain() {
  log "检查构建工具链"
  command -v cargo >/dev/null || die "缺少 cargo（需要 rustc >= 1.98）"
  command -v cargo-zigbuild >/dev/null \
    || die "缺少 cargo-zigbuild：cargo install cargo-zigbuild --locked"
  local rustc_ver
  rustc_ver="$(rustc --version | awk '{print $2}')"
  ok "rustc $rustc_ver / cargo-zigbuild"
  # rustc >= 1.98（wreq 0.16.1 的最低要求）
  if [ "$(printf '%s\n1.98\n' "$rustc_ver" | sort -V | head -1)" != "1.98" ]; then
    die "rustc $rustc_ver 过旧，wreq 需要 >= 1.98（rustup update stable）"
  fi

  if ! $skip_console; then
    command -v pnpm  >/dev/null || die "缺少 pnpm（npm i -g pnpm@9.15.9）"
    command -v node  >/dev/null || die "缺少 node（>= 22）"
    ok "pnpm $(pnpm --version) / node $(node --version)"
  fi
}

# 准备 zig 交叉工具链（幂等；缺失时自动下载）
setup_zig() {
  if [ ! -x "$ZIG_ROOT/zig" ]; then
    log "下载 zig $ZIG_VERSION"
    local tarball="zig-x86_64-linux-$ZIG_VERSION.tar.xz"
    ( cd /tmp && curl -fsSLO "https://ziglang.org/download/$ZIG_VERSION/$tarball" \
      && tar xf "$tarball" ) || die "zig 下载/解压失败"
  fi
  [ -x "$ZIG_ROOT/zig" ] || die "zig 不可用: $ZIG_ROOT/zig"
  ok "zig $("$ZIG_ROOT/zig" version) @ $ZIG_ROOT"

  # 生成 cc/c++/ar shim（关键：把 Rust triple 翻译成 zig triple）
  #
  # 注意（踩过的坑）：
  #  * cc/c++ 用 *真实文件*（cp），不能用符号链接，否则 `cat >` 会跟随链接
  #    把所有 shim 覆盖成同一个内容。
  #  * ar/ranlib 必须用系统 binutils；zig 的 ar 不兼容 BoringSSL
  #    （ar: error: expected [relpos] for 'a', 'b', or 'i' modifier）。
  #  * 不要手写 CARGO_TARGET_*_LINKER 指向 gcc shim，会 duplicate symbol: _start。
  if $dry_run; then printf '\033[2m  $ bootstrap zig shims -> %s\033[0m\n' "$ZIGBIN"; return 0; fi
  mkdir -p "$ZIGBIN"
  cat > "$ZIGBIN/boot-cc" <<EOF
#!/bin/sh
args=""
for a in "\$@"; do
  case "\$a" in
    --target=x86_64-unknown-linux-musl|--target=x86_64-unknown-linux-gnu)
      args="\$args --target=x86_64-linux-musl" ;;
    *) args="\$args \$a" ;;
  esac
done
exec $ZIG_ROOT/zig cc \$args
EOF
  sed 's/ cc / c++ /' "$ZIGBIN/boot-cc" > "$ZIGBIN/boot-cxx"
  chmod +x "$ZIGBIN/boot-cc" "$ZIGBIN/boot-cxx"
  for n in gcc cc; do cp "$ZIGBIN/boot-cc"  "$ZIGBIN/x86_64-linux-musl-$n"; done
  for n in g++ c++; do cp "$ZIGBIN/boot-cxx" "$ZIGBIN/x86_64-linux-musl-$n"; done
  printf '#!/bin/sh\nexec /usr/bin/ar "$@"\n'     > "$ZIGBIN/x86_64-linux-musl-ar"
  printf '#!/bin/sh\nexec /usr/bin/ranlib "$@"\n' > "$ZIGBIN/x86_64-linux-musl-ranlib"
  chmod +x "$ZIGBIN"/x86_64-linux-musl-*
  ok "zig shims 就绪 @ $ZIGBIN"
}

# ---------------------------------------------------------------- 构建
build_console() {
  log "构建前端 console（rust-embed 编译期固化，必须先于 Rust）"
  run pnpm --dir console install --frozen-lockfile
  run pnpm --dir console build
  $dry_run || [ -f crates/gproxy-host-axum/assets/web/index.html ] \
    || die "前端产物缺失：crates/gproxy-host-axum/assets/web/index.html"
  ok "前端产物已同步到 crates/gproxy-host-axum/assets/web"
  # 前端变动后 cargo 不会自动重编，touch 以重新嵌入
  $dry_run || touch crates/gproxy-host-axum/src/static_assets.rs
}

build_binary() {
  log "交叉编译静态 musl 二进制（$TARGET）"
  setup_zig
  export PATH="$ZIG_ROOT:$ZIGBIN:$PATH"
  export CC_x86_64_unknown_linux_musl="$ZIGBIN/x86_64-linux-musl-gcc"
  export CXX_x86_64_unknown_linux_musl="$ZIGBIN/x86_64-linux-musl-g++"
  export AR_x86_64_unknown_linux_musl="$ZIGBIN/x86_64-linux-musl-ar"
  export ZIG_GLOBAL_CACHE_DIR="${ZIG_GLOBAL_CACHE_DIR:-/tmp/zig-cache}"
  export ZIG_LOCAL_CACHE_DIR="${ZIG_LOCAL_CACHE_DIR:-/tmp/zig-cache-local}"

  local rel="target/$TARGET/release"
  # 切换 CC 后必须清掉 CMake 缓存，否则沿用旧的编译器探测结果
  if ! $dry_run; then
    rm -rf "$rel"/build/aws-lc-sys-* "$rel"/build/btls-sys-* 2>/dev/null || true
  fi

  run cargo zigbuild --locked --release -p gproxy-host-axum --target "$TARGET"

  local bin="$rel/gproxy"
  $dry_run || [ -f "$bin" ] || die "未找到产物 $bin"
  $dry_run || { file "$bin" | grep -q 'statically linked' \
    || warn "产物可能不是静态链接，请检查"; }
  $dry_run || ok "$bin ($(du -h "$bin" | cut -f1))"
}

# ---------------------------------------------------------------- 发布到 yocto
deploy() {
  local bin="target/$TARGET/release/gproxy"
  $dry_run || [ -f "$bin" ] || die "缺少产物 $bin（先执行 build）"

  log "部署到 $DEPLOY_HOST:$DEPLOY_DIR"
  run ssh -o BatchMode=yes "$DEPLOY_HOST" \
    "test -d '$DEPLOY_DIR/bin' || { echo '远端目录不存在: $DEPLOY_DIR/bin' >&2; exit 1; }"

  local ts sha remote_bin
  ts="$(date +%Y%m%d-%H%M%S)"
  sha="$(sha256sum "$bin" | cut -d' ' -f1)"
  remote_bin="$DEPLOY_DIR/bin/gproxy"
  log "sha256=$sha  ts=$ts"

  # 备份远端旧二进制，便于回滚
  run ssh -o BatchMode=yes "$DEPLOY_HOST" \
    "cp -a '$remote_bin' '$remote_bin.bak-$ts' 2>/dev/null || true; ls -1t $DEPLOY_DIR/bin/gproxy.bak-* 2>/dev/null | tail -n +6 | xargs -r rm -f"
  ok "已备份远端旧二进制（保留最近 5 份）"

  run scp -q "$bin" "$DEPLOY_HOST:$remote_bin.tmp-$ts"
  # 原子替换：先落到同目录临时文件，再 mv，避免运行中的进程看到半截文件
  run ssh -o BatchMode=yes "$DEPLOY_HOST" \
    "chmod 755 '$remote_bin.tmp-$ts' && mv -f '$remote_bin.tmp-$ts' '$remote_bin'"
  ok "二进制已更新"

  if $restart; then
    log "重启 $DEPLOY_SERVICE"
    run ssh -o BatchMode=yes "$DEPLOY_HOST" "systemctl --user restart '$DEPLOY_SERVICE'"
    $dry_run || sleep 4
    if ! $dry_run; then
      ssh -o BatchMode=yes "$DEPLOY_HOST" "systemctl --user is-active '$DEPLOY_SERVICE'" \
        | grep -qx active || die "服务未处于 active，请查看 journalctl --user -u $DEPLOY_SERVICE"
      ok "服务 active"
    fi
  else
    warn "已跳过重启（--no-restart），改动将在下次重启后生效"
  fi
}

verify() {
  $restart || return 0
  log "健康检查（远端 127.0.0.1:$DEPLOY_PORT）"
  if $dry_run; then printf '\033[2m  $ curl health checks\033[0m\n'; return 0; fi
  local code
  code="$(ssh -o BatchMode=yes "$DEPLOY_HOST" \
    "curl -s -o /dev/null -w '%{http_code}' -m 10 http://127.0.0.1:$DEPLOY_PORT/admin/api/session")"
  [ "$code" = 200 ] || die "管理台自检失败（/admin/api/session = $code）"
  ok "/admin/api/session -> 200"
  # 未带 key 访问 /v1/models 应得 401，能响应即说明网关在服务
  code="$(ssh -o BatchMode=yes "$DEPLOY_HOST" \
    "curl -s -o /dev/null -w '%{http_code}' -m 10 http://127.0.0.1:$DEPLOY_PORT/v1/models")"
  case "$code" in 401|200) ok "/v1/models -> $code (网关在线)";; *) die "/v1/models 异常: $code";; esac

  local remote_ver
  remote_ver="$(ssh -o BatchMode=yes "$DEPLOY_HOST" "$DEPLOY_DIR/bin/gproxy --version" 2>/dev/null | head -1)"
  ok "远端版本: $remote_ver"
}

# ---------------------------------------------------------------- 打 tag / Release
tag_release() {
  local version tag
  version="$(scripts/release-metadata.sh version)"
  tag="v$version"
  log "打 tag $tag"

  if $dry_run; then printf '\033[2m  $ git tag %s && git push fork %s\033[0m\n' "$tag" "$tag"; return 0; fi
  [ -z "$(git status --porcelain)" ] || die "工作区不干净，先提交再打 tag"

  if git rev-parse -q --verify "refs/tags/$tag" >/dev/null; then
    warn "$tag 已存在于本地，跳过创建"
  else
    git tag -a "$tag" -m "gproxy $tag"
    ok "已创建 tag $tag"
  fi
  git push fork "refs/tags/$tag" && ok "tag 已推送到 fork"
}

github_release() {
  local version tag bin asset
  version="$(scripts/release-metadata.sh version)"
  tag="v$version"
  bin="target/$TARGET/release/gproxy"
  asset="gproxy-linux-x86_64-musl"

  log "在 $FORK_REPO 创建 GitHub Release $tag"
  if $dry_run; then
    printf '\033[2m  $ gh release create %s %s\033[0m\n' "$tag" "$asset"
    return 0
  fi
  command -v gh >/dev/null || die "缺少 gh（GitHub CLI）"
  [ -f "$bin" ] || die "缺少产物 $bin"

  local dist="$root/dist/local"
  mkdir -p "$dist"
  install -m 0755 "$bin" "$dist/$asset"
  sha256sum "$dist/$asset" | sed "s#$dist/##" > "$dist/$asset.sha256"

  local notes="$RELEASE_NOTES_DIR/$tag.md"
  if [ ! -f "$notes" ]; then
    notes="$dist/$tag.notes.md"
    printf 'gproxy %s\n\n本地 zig musl 静态构建（%s）。\n' "$tag" "$TARGET" > "$notes"
    warn "缺少 $RELEASE_NOTES_DIR/$tag.md，使用自动生成的说明"
  fi

  if gh release view "$tag" --repo "$FORK_REPO" >/dev/null 2>&1; then
    gh release upload "$tag" --repo "$FORK_REPO" "$dist/$asset" "$dist/$asset.sha256" --clobber
    gh release edit   "$tag" --repo "$FORK_REPO" --notes-file "$notes"
    ok "已更新 Release $tag"
  else
    gh release create "$tag" --repo "$FORK_REPO" \
      "$dist/$asset" "$dist/$asset.sha256" \
      --title "gproxy $tag" --notes-file "$notes" --latest
    ok "已创建 Release $tag"
  fi
}

# ---------------------------------------------------------------- 主流程
log "gproxy 本地构建/发布  (mode=$mode, target=$TARGET, host=$DEPLOY_HOST)"

case "$mode" in
  build)
    check_toolchain
    $skip_console || build_console
    build_binary
    ;;
  deploy)
    deploy
    verify
    ;;
  all)
    check_toolchain
    $skip_console || build_console
    build_binary
    deploy
    verify
    ;;
esac

$do_tag && tag_release
$do_release && github_release

log "完成"