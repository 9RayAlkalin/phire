# AGENTS.md

## 项目

**Phire (Phi Recorder)** — 一款 Rust 编写的节奏游戏。支持多平台：Windows、Android、iOS、WASM、HarmonyOS。

## 工作空间（Workspace）

| Crate（包） | 角色 |
|------------|------|
| `phire` | 核心库：游戏引擎、谱面解析、UI 原语、渲染 |
| `phire-ui` | UI 层：在线功能、登录、谱面列表、场景。以 `["lib", "cdylib"]` 方式构建，用于 Android JNI |
| `phire-main` | 极简二进制入口 —— 调用 `phire_ui::quad_main()` |
| `prpr-avc` | 音视频编解码支持（链接原生静态库） |
| `prpr-pbc` | 命令行谱面格式转换器：`prpr-pbc <输入文件> <输出文件>` |

## 常用命令

```bash
# 桌面端构建
cargo build

# 发布构建（剥离符号）
cargo build --release

# 谱面转换器
cargo run -p prpr-pbc -- <输入谱面> <输出.pbc>
```

本仓库没有测试套件、没有 CI、没有 lint/类型检查脚本。

## 构建注意事项

- `phire-ui/build.rs` 会运行 `dotenv_build` —— 工作区根目录下需要一个 `.env` 文件。如果没有，构建可能失败或使用默认值。仓库中没有提交 `.env.example`。
- `prpr-avc/build.rs` 会链接原生静态库。需要设置 `PRPR_AVC_LIBS` 环境变量，或将库文件放置在 `prpr-avc/static-lib/<目标平台>/` 下。`static-lib/` 目录已被 `.gitignore` 忽略。
- **`.cargo/config.toml` 已被 git 忽略**。如果需要 Android 交叉编译，必须在本地创建该文件并配置 NDK 路径（原来的硬编码路径 `/home/hlmc/android-ndk-r27c/...` 已失效）。
- 自定义分支：`macroquad` 和 `miniquad` 固定为 `github.com/2278535805/prpr-*`。音频通过 `sasa`（来自 `github.com/2278535805/sasa`）支持。
- `phire-ui` 还依赖 `phira-mp-client` / `phira-mp-common`（来自 `github.com/TeamFlos/phira-mp`）。
- WASM 目标（`build_wasm.sh`）需要 ffmpeg 静态库，但 `wasm32` 平台不存在此类库，因此目前无法构建 WASM。

## Feature 特性标志

| Crate（包） | Feature | 默认启用 | 说明 |
|------------|---------|----------|------|
| `phire` | `play` | 是 | 游玩模式 vs. 预览模式 |
| `phire` | `video` | 是 | 启用 `prpr-avc` 依赖 |
| `phire` | `log` | 是 | 启用 `tracing-subscriber` / `colored` |
| `phire` | `closed` | 否 | 控制 `inner.rs` 模块（已被 git 忽略，专有代码）—— 没有该文件将无法编译 |
| `phire-ui` | `play` | 是 | 镜像 `phire/play` |
| `phire-ui` | `closed` | 否 | 传递给 `phire/closed` |
| `phire-ui` | `aa` | 否 | 防沉迷系统（中国监管要求，仅 Android） |
| `phire-ui` | `chat` | 否 | 游戏内聊天 |

当修改由 `cfg(feature = "play")` 或 `cfg(feature = "closed")` 控制的代码时，请确保两个分支都能编译通过。

## 格式化

`rustfmt.toml` 中配置了 `max_width = 150`、`fn_call_width = 150`。提交前请运行 `cargo fmt`。

## 架构说明

- 入口流程：`phire-main/src/main.rs` → `phire_ui::quad_main()`（导出为 `extern "C"` 供 JNI 使用）→ `the_main()` 异步循环。
- `closed` 特性控制 `phire` 和 `phire-ui` 中的 `inner.rs` 模块 —— 这是专有/加密资源加载代码。没有该文件（已被 git 忽略）将无法编译。
- `phire/src/bin.rs` **不是**一个二进制文件 —— 它是二进制谱面格式（`.pbc`）的序列化/反序列化模块。
- 全局可变状态：`phire-ui` 将 `DATA` 存储为 `static mut Option<Data>` —— 通过 `phire-ui/src/lib.rs:66-72` 中的 `get_data()`/`get_data_mut()` 不安全包装器访问。
- 平台条件编译随处可见（`cfg(target_os = "android")`、`cfg(target_arch = "wasm32")`、`cfg(target_os = "ios")`、`cfg(target_os = "windows")`）。修改共享代码时请检查所有平台块。
- 防沉迷系统（`aa` 特性）是中国监管合规要求 —— 通过 JNI 回调到 Android Java 层。
- 本地化：`phire/src/l10n.rs` 使用 Fluent `.ftl` 文件，位于 `phire/locales/` 和 `phire-ui/locales/`。语言环境通过 `sys-locale` 自动检测。两个 crate 都使用 `tl_file!` 宏嵌入 `.ftl` 资源。

## 文件约定

- `phire/src/core/` —— 游戏引擎核心（谱面模型、动画、判定线、音符）
- `phire/src/parse/` —— 谱面格式解析器（Phigros、RPE、PEC）
- `phire/src/scene/` —— 游戏场景（主菜单、游戏内等）
- `phire/src/ui/` —— UI 组件库
- `phire/locales/` —— 核心 Fluent 本地化文件
- `phire-ui/src/page/` —— UI 页面
- `phire-ui/src/scene/` —— UI 场景实现
- `phire-ui/src/client/` —— 在线 API 客户端
- `phire-ui/locales/` —— UI 本地化文件