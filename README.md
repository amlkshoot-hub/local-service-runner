# Local Service Runner

![Platform](https://img.shields.io/badge/platform-Windows%2010%20%7C%2011-0078D4)
![Tauri](https://img.shields.io/badge/Tauri-2-24C8DB)
![Rust](https://img.shields.io/badge/Rust-1.75%2B-000000)
![Vue](https://img.shields.io/badge/Vue-3-42B883)
![TypeScript](https://img.shields.io/badge/TypeScript-6-3178C6)

> Windows 本地 Spring Boot 服务运行管理器：像 IntelliJ IDEA 的 Run 面板一样管理多个本地服务，负责启动、停止、重启、Profile 切换与实时日志，并保证 Maven/Gradle Wrapper 派生的 Java 进程一个都不残留。
>
> A lightweight Windows desktop manager for local Spring Boot services, built with Tauri 2, Rust, Vue 3 and TypeScript. It focuses on reliable process-tree lifecycle control, Spring profile switching and streaming logs.

---

## 为什么需要它

在 Windows 上用 `mvn spring-boot:run` 或 `gradlew bootRun` 调试多个微服务时，常见问题包括：

- 命令行窗口一关，服务跟着退出；或者反过来，窗口关了 Java 进程还在后台占端口。
- Maven / Gradle Wrapper 会派生出多级子进程，按 `java.exe` 名称批量结束容易误杀别的服务。
- 每个服务要用的 `dev` / `test` / `prod` Profile 不同，切换时得反复改命令或配置文件。
- 日志分散在多个终端里，中文输出还可能因 GBK 编码变成乱码。

Local Service Runner 把这些问题收敛到一个桌面窗口里：服务配置集中管理，启动即纳入 Windows Job Object，停止时按进程树精确回收，日志统一流式展示。

## 核心特性

### 1. Spring Boot Profile 智能配置管理
- **自动识别**：根据服务工作目录，自动扫描识别 `src/main/resources/` 下的：
  - `application-{profile}.yml`
  - `application-{profile}.yaml`
  - `application-{profile}.properties`
  去重提取出 `dev`、`test`、`prod`、`local` 等环境配置，排除 `application.yml` 基础配置。
- **独立持久化**：每个服务独立保存上次选择的待用 Profile，应用重启后自动恢复。
- **运行与待生效解耦**：
  - 界面明确显示 **当前实际运行使用的 Profile** 与 **下次启动待生效的 Profile**。
  - 运行中调整 Profile 不会影响当前进程，只有在下一次启动或重启时生效。
- **多 Profile 支持**：支持选择 `default`（不显式指定），或手动输入多 Profile 组合（如 `dev,local`），严格保留参数顺序。
- **启动参数构建**：
  - **JAR 模式**：追加 `--spring.profiles.active=dev,local`
  - **Maven Wrapper 模式**：使用标准系统属性 `-Dspring-boot.run.profiles=dev,local`
  - **Gradle Wrapper 模式**：使用标准应用参数 `--args=--spring.profiles.active=dev,local`
  - **自定义命令**：提供可选 Profile 注入开关

### 2. Windows 进程树与 Job Object 终极管控（最高优先级）
- **挂起启动彻底防逃逸**：
  - 创建原生 Windows Job Object，并配置 `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`。
  - 使用 `CREATE_SUSPENDED`（挂起）标志启动根进程。
  - 在主线程恢复运行前调用 `AssignProcessToJobObject`，**从根本上彻底消除了进程创建到加入 Job 之间的竞态**。所有后续由 Maven、Gradle 或命令行派生的多级 Java 子进程无一遗漏地处于受控 Job 树中。
- **可靠的两阶段停止流程**：
  - **第一阶段（正常停止尝试）**：等待配置的超时时间（默认 10 秒）。
  - **第二阶段（内核强制终结）**：超时后调用 `TerminateJobObject`，内核级清理整个进程树，绝不凭 `java.exe` 盲目杀进程，绝不误杀未受管理的其他外部进程。
  - 严格等待进程句柄退出信号（`WaitForSingleObject`），确保旧进程真正退出后才将服务标记为“已停止”。
- **严格安全的重启机制**：
  - 串行化状态机：`Running` -> `Stopping` -> 确认完全退出 -> `Starting`（采用最新选中的 Profile） -> `Running`。
  - 防重入保护，避免快速连续点击造成重复进程实例。
- **崩溃自动保护**：
  - 依赖 Windows 操作系统的 `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` 特性，即使用户意外强制终止 Runner 或 Runner 异常退出，操作系统内核也会自动回收全部受管理的子进程。

### 3. 高性能实时日志视窗
- **异步非阻塞管道**：Rust 后端通过 Windows 匿名管道异步读取标准输出与标准错误流，避免管道缓冲阻塞。
- **编码自适应**：按行以 UTF-8 解码，失败时自动回退 GBK，兼容 Windows 中文环境下 Maven / Java 的 GBK 输出，不会因中文日志中断日志流。
- **有界环形缓冲区**：每个服务独立维护最近 5000 行日志，超限丢弃最早行，内存占用恒定。
- **日志过滤与交互**：
  - 即时文本过滤搜索
  - 自动滚动 / 暂停滚动切换
  - 一键清空日志
  - 一键复制全部日志到剪贴板
  - 敏感参数自动脱敏保护
  - 识别 INFO（蓝/白）、WARN（黄）、ERROR（红）、SYSTEM（紫）颜色高亮
  - 自动剥离 Spring Boot / Maven 输出的 ANSI 颜色转义字符

### 4. IntelliJ IDEA 风格精致暗色界面
- 基于 Vue 3 + TypeScript + 自定义精致 CSS，无臃肿第三方组件库。
- 启动仅需数毫秒，内存占用低，高信息密度与纯正暗色工整布局。

### 5. 项目目录选择与 Maven 多模块启动
- **资源管理器选择目录**：添加/编辑服务时点击「📂 选择目录」直接弹出系统目录选择框，选中后自动回填工作目录；**服务名称默认取所选目录的最后一级目录名**（用户手动改过名称时不会被覆盖）。
- **多模块自动识别**：自动扫描工程内真正在 `<build><plugins>` 中声明了 `spring-boot-maven-plugin` 的子模块。当工作目录是聚合根（packaging=pom）时，自动使用 `mvn -pl <module> spring-boot:run` 在正确的模块中启动，从根本上解决
  `No plugin found for prefix 'spring-boot' in the current project` 导致服务完全无法启动的问题。
- **模块可手动覆盖**：配置弹窗中可选择/切换启动模块（留空为自动检测），主界面标题处显示当前实际使用的模块。
- **Profile 自动定位**：Profile 扫描会优先读取启动模块下的 `src/main/resources`，多模块工程同样能识别 `application-{profile}.yml`。
- **启动失败可见**：进程以非 0 退出码结束时状态显示为「异常退出」，而不是「已停止」，便于快速定位问题。

> 说明：Maven 多模块工程的子模块 jar 需要存在于本地仓库，首次使用前请在工程根目录执行一次 `mvn install -DskipTests`（IDEA 会自动使用工作区模块，命令行 Maven 不会）。

---

## 技术栈

| 层次 | 选型 | 说明 |
| :--- | :--- | :--- |
| 桌面框架 | Tauri 2 | 原生窗口、体积小、内存占用低 |
| 后端 | Rust（`windows-sys`、Tokio、Serde） | Job Object、匿名管道、异步流式读取 |
| 前端 | Vue 3 + TypeScript + Vite | 单页界面，无第三方 UI 组件库 |
| 平台能力 | Win32 Job Objects / Threading / Pipes | 进程树治理、挂起启动、内核级回收 |
| 数据存储 | `%LOCALAPPDATA%\LocalServiceRunner\services.json` | 原子写入，本地持久化 |

## 目录结构

```text
local-service-runner/
├── src/                        # 前端 Vue 3 + TypeScript
│   ├── components/
│   │   └── ServiceModal.vue    # 服务配置模态框（增删改、Profile 扫描）
│   ├── App.vue                 # 主界面（服务列表、控制台、实时日志）
│   ├── main.ts
│   └── types.ts                # TypeScript 类型定义
├── src-tauri/                  # 后端 Rust + Win32 原生集成
│   ├── src/
│   │   ├── config.rs           # 服务配置原子持久化
│   │   ├── maven.rs            # Maven 多模块工程探测与启动模块解析
│   │   ├── profiles.rs         # Spring Boot Profile 扫描与参数生成
│   │   ├── process.rs          # Windows Job Object、挂起启动与进程树生命周期
│   │   ├── logs.rs             # 异步日志缓冲、GBK/UTF-8 解码、脱敏与事件推送
│   │   ├── commands.rs         # 受控 Tauri 命令接口（含目录选择）
│   │   ├── lib.rs
│   │   └── main.rs
│   ├── capabilities/           # Tauri 权限能力声明
│   ├── build.rs                # 自动注入 Common-Controls v6 清单
│   ├── Cargo.toml
│   └── tauri.conf.json
├── fixtures/                   # 验收测试夹具
│   └── demo-spring-service/    # 多 Profile 配置 + 无需 Spring 依赖的仿真 JAR
├── index.html
├── package.json
├── vite.config.ts
└── README.md
```

## 开发与构建指南

### 1. 环境准备
- **操作系统**：Windows 10 / Windows 11
- **Node.js**：18+（推荐 v24）
- **Rust**：1.75+（MSVC 工具链 `x86_64-pc-windows-msvc`）
- **C++ Build Tools**：Visual Studio 2022 C++ 生成工具 + Windows 10/11 SDK
- **WebView2 Runtime**：Windows 10/11 自带

### 2. 依赖安装与启动调试
```powershell
# 1. 安装前端依赖
npm install

# 2. 运行单元测试与集成测试（验证 Job Object 与仿真 JAR 启停）
cd src-tauri
cargo test --lib -- --nocapture
cd ..

# 3. 启动前端 + 桌面开发调试模式
npm run tauri dev
```

### 3. 构建发布独立可执行程序
```powershell
# 构建生产包（前端 + Rust + 安装包）
npm run tauri build
```
编译产物位于 `src-tauri/target/release/`，安装包位于 `src-tauri/target/release/bundle/`。
仅需可执行文件时也可以：
```powershell
npm run build
cd src-tauri
cargo build --release
```

## 配置与数据位置

- 服务配置：`%LOCALAPPDATA%\LocalServiceRunner\services.json`
- 日志：仅保存在内存环形缓冲区（每服务 5000 行），不落盘
- 删除 `services.json` 即可重置全部服务配置

## 验收测试覆盖情况

项目中已编写自动化集成测试并全部执行通过：

| 测试项 | 验证内容 | 结果 |
| :--- | :--- | :---: |
| `test_build_profile_args_jar` | 验证 JAR 模式正确传入 `--spring.profiles.active=dev` | ✅ PASS |
| `test_build_profile_args_maven` | 验证 Maven 模式传入 `-Dspring-boot.run.profiles=prod` | ✅ PASS |
| `test_build_profile_args_gradle` | 验证 Gradle 模式传入 `--args=--spring.profiles.active=test` | ✅ PASS |
| `test_format_active_profiles` | 验证多 Profile 格式化（保留顺序、过滤 default） | ✅ PASS |
| `test_detect_profiles_in_maven_module` | 验证启动模块下的 `application-{profile}.yml` 扫描 | ✅ PASS |
| `test_detects_module_in_aggregator` | 验证聚合根下自动定位声明了插件的可启动子模块 | ✅ PASS |
| `test_prefers_shallow_module_and_ignores_plugin_management` | 验证优先浅层模块且忽略 pluginManagement | ✅ PASS |
| `test_root_with_plugin_needs_no_module` | 验证聚合根自身声明插件时无需 `-pl` 参数 | ✅ PASS |
| `test_configured_module_is_validated` | 验证手动配置的模块必须真实存在 | ✅ PASS |
| `test_non_maven_dir_is_reported` | 验证非 Maven 目录被正确识别 | ✅ PASS |
| `test_maven_command_uses_detected_module` | 验证启动命令生成 `-pl <module> spring-boot:run` | ✅ PASS |
| `test_maven_command_rejects_missing_module` | 验证缺失模块被拒绝并返回明确错误 | ✅ PASS |
| `test_cmd_command_line_quotes_command_with_spaces` | 验证含空格路径的 cmd.exe 命令行引号处理 | ✅ PASS |
| `test_decode_console_bytes_handles_gbk_and_utf8` | 验证 GBK / UTF-8 日志行解码与 ANSI 转义剥离 | ✅ PASS |
| `test_service_status_serializes_camel_case_fields` | 验证服务状态 JSON 字段为 camelCase（activeProfiles / startedAt / exitCode） | ✅ PASS |
| `test_real_jar_lifecycle_and_job_object` | 真实启动 Java 进程、注入 Job Object、捕获输出、正常停止（含重复启动拒绝） | ✅ PASS |
| `test_service_restart_flow` | 验证重启严格先停后启，新 Profile 生效并重新获得 PID | ✅ PASS |

> 集成测试通过 `CARGO_MANIFEST_DIR` 定位仓库内的 `fixtures/demo-spring-service`，克隆到任意路径均可运行；若夹具 JAR 不存在则自动跳过。

## 已知限制与运行说明

1. **Spring Boot 优雅停机**：Job Object 的强制终止属于操作系统级清理，并非 Spring Boot 的 Graceful Shutdown。应用配置了超时时间（默认 10s），超时后执行 Job Object 树级强制终止，确保不留孤儿进程。
2. **WebView2 依赖**：应用依赖系统 WebView2 运行时（Windows 10 现代版本及 Windows 11 默认已预装；若在精简版 Windows 上缺失，双击启动时微软官方会提示安装 WebView2 Evergreen Runtime）。
3. **平台限定**：进程控制、管道读取与命令行拼装均针对 Windows 实现，暂不支持 macOS / Linux。

## 更新记录

### 2026-11-18
- **修复点击启动后界面变黑 / 卡死**：`ServiceStatus` 枚举的 `rename_all` 只重命名变体名，不重命名变体内的字段，导致前端拿到的状态是 `active_profiles` / `started_at` / `exit_code`，而前端按 `activeProfiles` 读取，在「运行中」状态渲染时抛出 `TypeError: Cannot read properties of undefined (reading 'length')`，Vue 根组件渲染失败后整个界面只剩黑屏。
  现已显式指定 `activeProfiles` / `startedAt` / `exitCode`，并让前端对缺失字段做防御性处理，同时新增序列化回归测试。
- 日志控制台自动剥离 Spring Boot / Maven 输出的 ANSI 颜色转义字符，避免出现 `[0;39m` 等乱码。
- 端到端验证：真实启动 `dytelcp-pms`（`mvn -pl ry-admin spring-boot:run -Dspring-boot.run.profiles=dev`）后可正常显示「运行中」、PID、活动 Profile 与实时日志（含中文），停止后无残留进程。

## 参与与反馈

- 仓库地址：<https://github.com/amlkshoot-hub/local-service-runner>
- 问题反馈与功能建议：欢迎提交 Issue 或 Pull Request
- 提交前建议本地执行 `npm run build` 与 `cd src-tauri && cargo test --lib`，确保前端类型检查与后端测试通过

## 许可证

当前仓库暂未附开源许可证。如需复用代码，请先通过 Issue 联系作者。

> AI生成
