# Local Service Runner

![Platform](https://img.shields.io/badge/platform-Windows%2010%20%7C%2011-0078D4)
![Tauri](https://img.shields.io/badge/Tauri-2-24C8DB)
![Rust](https://img.shields.io/badge/Rust-1.75%2B-000000)
![Vue](https://img.shields.io/badge/Vue-3-42B883)
![TypeScript](https://img.shields.io/badge/TypeScript-6-3178C6)

> Windows 本地 Spring Boot 服务运行管理器：像 IntelliJ IDEA 的 Run 面板一样管理多个本地服务，负责启动、停止、重启、Profile 切换与实时日志，内置 Maven 构建+JAR 运行、源码主类直启、Maven 工具箱与项目终端，并保证 Maven/Gradle Wrapper 派生的 Java 进程一个都不残留。
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
  - **构建+JAR 运行模式**：构建后以 `java -jar` 运行，追加 `--spring.profiles.active=dev,local`
  - **源码主类直启模式**：通过 `-Dexec.args=--spring.profiles.active=dev,local` 传给 main 方法
  - **Gradle Wrapper 模式**：使用标准应用参数 `--args=--spring.profiles.active=dev,local`
  - **自定义命令**：提供可选 Profile 注入开关

### 2. 六种启动方式：JAR / Maven Wrapper / 构建+JAR / 源码主类 / Gradle / 自定义
- **JAR 包**：直接 `java -jar <jar>`。
- **Maven Wrapper**：`mvn -pl <module> spring-boot:run`，多模块自动定位。
- **构建+JAR 运行（新增）**：一键“先构建后运行”：
  - 启动时自动执行 `mvn install` 或 `mvn package`（可选 `-DskipTests`，默认跳过）；
  - 构建全程在日志视窗实时可见，状态显示为「构建中」，可随时停止；
  - 构建成功后自动在启动模块（或工程根）`target/` 下定位最新产物 JAR（排除 `*-sources.jar` / `*-tests.jar` / `*-javadoc.jar` 等附属包），并以 `java -jar` 拉起服务；
  - 构建失败（非 0 退出码）状态标记为「异常退出」，不启动服务；
  - 构建进程同样纳入 Job Object：停止即终止整棵 Maven 进程树。
- **源码主类直启（新增）**：IDEA “点 main 类运行”的命令行等价物：
  - 执行 `mvn -pl <module> compile exec:java -Dexec.mainClass=<主类>`；
  - 主类可一键探测：扫描 `src/main/java` 下含 `@SpringBootApplication` 注解（优先）或 `public static void main` 的类；
  - 修改源码后直接重启即生效，无需打包。
- **Gradle Wrapper**：`gradlew bootRun`。
- **自定义命令**：任意可执行程序。

### 3. Maven 工具箱与项目终端（新增）
- **一键 Maven 构建**：服务主面板提供 `Clean` / `Compile` / `Package` / `Install` 按钮，一键执行 `mvn clean / compile / package / install`（优先使用工程内 `mvnw.cmd`，全工程构建不带 `-pl`）；
- **输出即日志**：构建输出进入该服务的实时日志视窗，GBK/UTF-8 自适应解码；
- **可中断**：构建中可随时「停止构建」，内核级终结 Maven 进程树，不残留孤儿进程；
- **互斥防呆**：服务运行中（JAR 被进程锁定）不允许发起构建，构建中不允许启动/重启服务；
- **项目终端**：一键在服务工作目录唤起终端——优先 Windows Terminal（`wt -d <dir>`），未安装时回退 `cmd.exe` 新窗口；终端独立于本应用生命周期，关闭 Runner 不会连带关闭终端。

### 4. Windows 进程树与 Job Object 终极管控（最高优先级）
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

### 5. 高性能实时日志视窗
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

### 6. 现代化浅色界面（2026-11 全面重构，基于 UIUX 评审建议）
- **浅色系三阶明度**：白色工作画布 / 淡灰蓝侧栏 / 毛玻璃白顶栏，淡蓝主色（Indigo 蓝渐变），全局 CSS 设计令牌驱动；数字启用 `tabular-nums` 等宽防止采样抖动。
- **冷启动引导页**：无服务时提供「添加第一个服务 / 导入配置文件」直达按钮、虚线拖拽区（项目文件夹拖入即预填配置并自动探测模块与 Profile）、三张核心能力卡片。
- **顶栏状态胶囊**：全部/运行中/已停止/异常带色点计数，点击联动侧栏过滤；导入导出带文字标签。
- **侧栏信息架构**：快捷筛选 chips（全部/运行中/已停止）+ 分组折叠/展开（单击组名或全展开钮）+ 聚合日志流常驻入口 + 宽度拖拽（220~400px，localStorage 持久化）。
- **三段式主工作区**：① 服务头与生命周期栏（名称+模块+状态/健康徽章、启动/停止/重启、编辑/克隆/删除图标钮）② 次级工具条（运行/下次生效 Profile 解耦 + 「重启生效」琥珀提示、端口徽章一键浏览器直达、PID/CPU sparkline/内存、终端与 Maven 分段按钮组及防呆 Tooltip）③ 日志视窗。
- **日志视窗增强**：级别快速过滤胶囊（ALL/INFO/WARN/ERROR）、匹配计数 x/y、搜日志快捷键提示、上翻自动暂停滚底 + 「回到底部（N 条新日志）」悬浮胶囊、ERROR 行淡红底色。
- **快捷键体系**：Ctrl+K 聚焦服务过滤 / Ctrl+N 新建服务 / Ctrl+R 重启 / Ctrl+L 清空日志 / Ctrl+F 搜日志。
- 首次最小化到托盘时弹系统通知提示，避免误以为程序退出。
- 统一 Feather 风格线条 SVG 图标集（`AppIcon.vue`），状态点呼吸脉冲、构建 spinner、按钮按压反馈等微动效。

### 7. 项目目录选择与 Maven 多模块启动
- **资源管理器选择目录**：添加/编辑服务时点击「📂 选择目录」直接弹出系统目录选择框，选中后自动回填工作目录；**服务名称默认取所选目录的最后一级目录名**（用户手动改过名称时不会被覆盖）。
  - **多模块自动识别**：自动扫描工程内真正在 `<build><plugins>` 中声明了
    `spring-boot-maven-plugin` 的子模块。当工作目录是聚合根（packaging=pom）时，自动使用 `mvn -pl <module> spring-boot:run` 在正确的模块中启动，从根本上解决
    `No plugin found for prefix 'spring-boot' in the current project` 导致服务完全无法启动的问题。
  - **模块可手动覆盖**：配置弹窗中可选择/切换启动模块（留空为自动检测），主界面标题处显示当前实际使用的模块；Maven 系启动方式（Wrapper / 构建+JAR / 源码主类）共用该模块选择：构建+JAR 模式下模块用于定位构建产物与 Profile 扫描，源码主类模式下模块用于 `-pl` 运行。
  - **Profile 自动定位**：Profile 扫描会优先读取启动模块下的 `src/main/resources`，多模块工程同样能识别 `application-{profile}.yml`。
  - **启动失败可见**：进程以非 0 退出码结束时状态显示为「异常退出」，而不是「已停止」，便于快速定位问题。
> 说明：多模块工程下，「Maven Wrapper」与「源码主类直启」启动时会自动先执行 `-pl <启动模块> -am install -DskipTests` 把启动模块的**依赖链**安装到本地仓库，再以 `spring-boot:run` / `exec:java` 启动——彻底避免「兄弟模块 was not found / failure was cached」类依赖解析失败（IDEA 会自动使用工作区模块，命令行 Maven 不会，此链式命令即为补齐该差异）。代价是每次启动多几秒安装时间；「构建+JAR 运行」本就是全工程构建，无需此步骤。

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
│   │   └── ServiceModal.vue    # 服务配置模态框（增删改、Profile 扫描、主类探测）
│   ├── App.vue                 # 主界面（服务列表、控制台、实时日志、Maven 工具箱）
│   ├── main.ts
│   └── types.ts                # TypeScript 类型定义
├── src-tauri/                  # 后端 Rust + Win32 原生集成
│   ├── src/
│   │   ├── config.rs           # 服务配置原子持久化
│   │   ├── maven.rs            # Maven 多模块工程探测、构建产物 JAR 定位、源码主类探测
│   │   ├── maven_tasks.rs      # Maven 工具箱任务（clean/compile/package/install 可中断）
│   │   ├── profiles.rs         # Spring Boot Profile 扫描与参数生成
│   │   ├── process.rs          # Windows Job Object、挂起启动、受管进程底层与两阶段「构建+JAR」状态机
│   │   ├── logs.rs             # 异步日志缓冲、GBK/UTF-8 解码、脱敏与事件推送
│   │   ├── terminal.rs         # 项目终端唤起（wt 优先、cmd 回退）
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
| `test_build_profile_args_maven_build_jar` | 验证构建+JAR 模式最终以应用参数形式注入 Profile | ✅ PASS |
| `test_build_profile_args_source_main_class` | 验证源码主类模式通过 `-Dexec.args` 注入 Profile | ✅ PASS |
| `test_maven_build_jar_command` | 验证构建命令生成（goal / -DskipTests / 非法目标拒绝） | ✅ PASS |
| `test_source_main_class_command` | 验证 `compile exec:java -Dexec.mainClass` 命令生成与主类缺失拒绝 | ✅ PASS |
| `test_locate_built_jar_picks_latest_and_ignores_attachments` | 验证产物定位取最新且排除 sources/javadoc 附属包 | ✅ PASS |
| `test_locate_built_jar_prefers_module_target` | 验证多模块下优先定位启动模块 target | ✅ PASS |
| `test_locate_built_jar_missing_dir` | 验证目录缺失时的明确错误提示 | ✅ PASS |
| `test_detect_main_classes_prefers_spring_boot_entry` | 验证主类探测优先 @SpringBootApplication 并过滤非入口类 | ✅ PASS |
| `test_detect_main_classes_in_module` | 验证启动模块下的主类探测与回退行为 | ✅ PASS |
| `test_fixture_main_class_and_jar_location` | 真实夹具冒烟：主类探测与产物 JAR 定位端到端可用 | ✅ PASS |
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
| `test_free_port_reports_available` / `test_occupied_port_reports_pid` | 验证端口预检：空闲放行、占用可解析占用者 PID | ✅ PASS |
| `test_resolve_health_url_variants` / `test_parse_http_url` | 验证健康检查 URL 解析（绝对/相对/缺端口拼接规则） | ✅ PASS |
| `test_http_get_against_real_listener` | 真实 TCP 环境验证零依赖 HTTP 探针：200 判定、无服务端口拒绝 | ✅ PASS |
| `test_merge_imported_appends_and_overwrites` / `test_merge_imported_empty_existing` | 验证导入合并：同 id 覆盖、新 id 追加、顺序保持 | ✅ PASS |
| `test_clone_config_gets_new_identity` | 验证克隆生成新 id 与「副本」名称 | ✅ PASS |

> 集成测试通过 `CARGO_MANIFEST_DIR` 定位仓库内的 `fixtures/demo-spring-service`，克隆到任意路径均可运行；若夹具 JAR 不存在则自动跳过。

## 已知限制与运行说明

1. **Spring Boot 优雅停机**：Job Object 的强制终止属于操作系统级清理，并非 Spring Boot 的 Graceful Shutdown。应用配置了超时时间（默认 10s），超时后执行 Job Object 树级强制终止，确保不留孤儿进程。
2. **WebView2 依赖**：应用依赖系统 WebView2 运行时（Windows 10 现代版本及 Windows 11 默认已预装；若在精简版 Windows 上缺失，双击启动时微软官方会提示安装 WebView2 Evergreen Runtime）。
3. **平台限定**：进程控制、管道读取与命令行拼装均针对 Windows 实现，暂不支持 macOS / Linux。

## 更新记录

### 2026-11-19（六）
- **修复多模块启动的兄弟依赖解析失败**：报错典型为 `org.xxx:ry-delivery:jar was not found ... This failure was cached in the local repository`——多模块工程以 `-pl <模块>` 启动时，兄弟模块只能从本地仓库解析，而它从未 install 过且远程查找失败被 Maven 缓存。「Maven Wrapper」与「源码主类直启」在多模块工程下的命令改为链式两步：`mvn -pl <模块> -am install -DskipTests && mvn -pl <模块> <原启动命令>`，依赖链自动安装、失败自愈；单模块工程保持原直接命令。附带修复仅修托盘双图标（声明式与代码式托盘重复注册）并新增单实例互斥（二次启动唤起已有窗口）。
- 新增 3 项测试（37 项全部通过）：链式命令结构、链式 cmd /c 引号处理（&& 不被引号包裹）、单模块不走 install 链。

### 2026-11-19（五）——浅色主题与 UIUX 全面重构（基于评审建议文档）
- **整体切换浅色主题**：深色系替换为「白底淡蓝」三阶明度（画布/侧栏/顶栏），全局令牌重写；次要文本对比度提升至 WCAG AA，数字 tabular-nums 防抖动。
- **冷启动空状态重构**：右侧主画布引导页（添加第一个服务 / 导入配置直达按钮 + 虚线拖拽区 + 3 张能力卡片）；左侧精简为单行「暂无服务 + 立即添加」，修复孤字折行。
- **拖拽建服务**：把 Spring Boot / Maven 项目文件夹拖到空状态区，自动校验目录、预填路径并探测模块与 Profile。
- **顶栏统计胶囊化**：全部/运行中/已停止/异常可点击、联动侧栏过滤；导入导出补文字标签。
- **侧栏重构**：快捷筛选 chips、分组折叠、全展开/折叠切换、聚合日志流常驻入口、侧栏宽度拖拽并持久化。
- **三段式主工作区**：生命周期按钮上移；Profile 不一致显示琥珀「重启生效」徽章；端口徽章一键打开浏览器；Maven 四目标收纳为分段按钮组并带防呆 Tooltip。
- **日志视窗增强**：级别胶囊过滤、匹配计数、上翻自动暂停滚底 + 「回到底部（N 条新日志）」胶囊；ERROR 行淡红底。
- **快捷键**：Ctrl+K / Ctrl+N / Ctrl+R / Ctrl+L / Ctrl+F。
- **托盘提示**：首次最小化到托盘时弹系统通知。
- 后端新增 `open_url` / `path_is_dir` 命令；实景截图评审两轮修复 10+ 项视觉细节。

### 2026-11-18（四）——健康探针、托盘常驻、聚合日志与资源监控
- **健康探针**：服务运行后周期 GET 健康检查端点（配置项 `healthCheckUrl`，相对路径自动拼 `http://127.0.0.1:{端口}`），状态栏新增「健康 / 不可用 / 检测中」徽章——从「进程活着」升级为「真的能服务请求」。零 HTTP 依赖（手写本机 HTTP/1.1 GET，读到状态行即刻判定）。
- **端口预检**：启动前探测端口占用，被占直接拦截并提示占用者 PID（GetExtendedTcpTable），不再让 Spring Boot 起来一半才报 `Port already in use`。
- **系统托盘 + 崩溃通知**：关闭按钮隐藏到托盘（进程治理与日志后台继续），托盘菜单动态列出全部服务及状态、支持一键启停；服务异常退出时弹 Windows 系统通知。
- **CPU / 内存监控**：基于 Job Object 枚举进程树采样（CPU 时间差分 + 工作集内存），状态栏实时徽章 + CPU 走势 sparkline（2s 采样，仅运行中开启）。
- **多服务日志聚合流**：控制台支持「单服务 / 聚合全部」视图切换，跨服务调用链同屏观测，每行带服务色块标签，过滤支持内容与服务名双维度。
- **服务分组 + 一键启停**：侧栏按业务域分节（`group` 字段，配置弹窗 datalist 自动补全），组头悬浮「一键启动 / 停止」整组服务。
- **配置导入 / 导出 / 克隆**：全量配置一键导出 JSON（换机 / 团队共享）；导入同 id 覆盖更新、新 id 追加；服务一键克隆后直接进入编辑（改端口 / 名称一气呵成）。
- **JVM 参数模板**：JAR / 构建+JAR 模式一键插入堆内存、远程调试 5005·8000、GC 日志常用参数。
- 修复：应用启动时已在运行的服务不触发指标轮询的时序问题（状态加载后同步轮询）。
- 测试 27 → 35 项全部通过；前端 vue-tsc + 构建通过；mock Tauri IPC 实景截图验证（分组 / 聚合 / 健康徽章 / 指标区无缺陷）。

### 2026-11-18（三）
- **界面现代化全面重构**：深空蓝黑底色系 + 靛紫渐变强调色，全局 CSS 设计令牌（色彩 / 圆角 / 阴影 / 等宽字体）统一驱动两个组件。
- 毛玻璃质感：顶栏、控制台工具栏、模态框遮罩 `backdrop-filter` 模糊叠加；弹窗入场缩放动画与错误横幅抖动动效。
- 全部 emoji / Unicode 图标替换为统一的 Feather 风格线条 SVG 图标组件（新增 `AppIcon.vue`）；自绘复选框与渐变选中态。
- 服务卡片化：选中态左侧渐变高亮条 + 微光底；状态点呼吸脉冲动画（运行中 / 启停中）。
- 日志级别徽章化（INFO 蓝 / WARN 琥珀 / ERROR 红 / DEBUG 绿 / 系统紫），等宽字体统一为 Cascadia Code / Consolas。
- 细节：全局细圆角滚动条、输入框聚焦光晕、按钮按压缩放反馈、Maven 构建中 spinner 徽章。
- 浏览器实景验证（mock Tauri IPC 数据 + 截图评审）：主界面与配置弹窗均无溢出 / 错位 / 对比度缺陷。

### 2026-11-18（二）
- **新增「构建+JAR 运行」启动方式**：先执行 `mvn install/package`（可选 `-DskipTests`）构建，成功后自动定位启动模块 `target` 下最新 JAR（排除 sources/javadoc 等附属产物）并以 `java -jar` 运行；构建阶段新增 `building` 状态、全程实时日志、可随时停止；构建失败标记「异常退出」；模块留空时自动复用 spring-boot 插件探测结果定位产物。
- **新增「源码主类直启」启动方式**：`mvn -pl <module> compile exec:java -Dexec.mainClass=<主类>`，支持一键探测源码主类（`@SpringBootApplication` 优先），Profile 通过 `-Dexec.args` 注入，改完源码重启即生效。
- **新增 Maven 工具箱**：服务面板一键 `mvn clean / compile / package / install`（优先 mvnw.cmd，全工程构建），输出进实时日志视窗，可中断（内核级终结进程树）；与服务运行状态互斥防呆（运行中的 JAR 被锁定时禁止构建，构建中禁止启动/重启）。
- **新增项目终端**：一键在服务工作目录唤起终端（Windows Terminal 优先，cmd 回退），终端独立于 Runner 生命周期。
- **底层重构**：抽出可复用的受管进程启动器（Job Object + 挂起 + 管道 + 生命周期回调），服务运行、构建阶段与 Maven 工具箱任务共用同一套进程树治理；应用退出时同步清理构建任务进程。
- 配置兼容：`services.json` 旧配置无需迁移，新增字段自动缺省。

### 2026-11-18（一）
- **修复点击启动后界面变黑 / 卡死**：`ServiceStatus` 枚举的 `rename_all` 只重命名变体名，不重命名变体内的字段，导致前端拿到的状态是 `active_profiles` / `started_at` / `exit_code`，而前端按 `activeProfiles` 读取，在「运行中」状态渲染时抛出 `TypeError: Cannot read properties of undefined (reading 'length')`，Vue 根组件渲染失败后整个界面只剩黑屏。
  现已显式指定 `activeProfiles` / `startedAt` / `exitCode`，并让前端对缺失字段做防御性处理，同时新增序列化回归测试。
- 日志控制台自动剥离 Spring Boot / Maven 输出的 ANSI 颜色转义字符，避免出现 `[0;39m` 等乱码。
- 端到端验证：真实启动 `dytelcp-pms`（`mvn -pl ry-admin spring-boot:run -Dspring-boot.run.profiles=dev`）后可正常显示「运行中」、PID、活动 Profile 与实时日志（含中文），停止后无残留进程。

## 参与与反馈

- 仓库地址：<https://github.com/amlkshoot-std/local-service-runner>
- 问题反馈与功能建议：欢迎提交 Issue 或 Pull Request
- 提交前建议本地执行 `npm run build` 与 `cd src-tauri && cargo test --lib`，确保前端类型检查与后端测试通过

## 许可证

当前仓库暂未附开源许可证。如需复用代码，请先通过 Issue 联系作者。

> AI生成
