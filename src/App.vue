<template>
  <div class="app-container">
    <!-- 顶部应用栏 -->
    <header class="app-header">
      <div class="header-left">
        <span class="app-logo"><AppIcon name="bolt" :size="17" :stroke-width="2.2" /></span>
        <span class="app-title">Local Service Runner</span>
        <span class="app-badge">Spring Boot 管理器</span>
      </div>
      <div class="header-right">
        <!-- 状态胶囊：点击联动侧栏过滤 -->
        <div class="stat-chips">
          <button
            :class="['stat-chip', { active: statusFilter === 'all' }]"
            title="全部服务，点击过滤"
            @click="setStatusFilter('all')"
          >
            全部 <b>{{ services.length }}</b>
          </button>
          <button
            :class="['stat-chip', { active: statusFilter === 'running', ok: true }]"
            title="运行中服务，点击过滤"
            @click="setStatusFilter('running')"
          >
            <span class="chip-dot ok"></span> 运行中 <b>{{ runningCount }}</b>
          </button>
          <button
            v-if="stoppedCount > 0"
            :class="['stat-chip', { active: statusFilter === 'stopped' }]"
            title="已停止服务，点击过滤"
            @click="setStatusFilter('stopped')"
          >
            <span class="chip-dot off"></span> 已停止 <b>{{ stoppedCount }}</b>
          </button>
          <button
            v-if="failedCount > 0"
            :class="['stat-chip', { active: statusFilter === 'stopped', err: true }]"
            title="异常退出服务，点击过滤"
            @click="setStatusFilter('stopped')"
          >
            <span class="chip-dot err"></span> 异常 <b>{{ failedCount }}</b>
          </button>
        </div>
        <button class="btn ghost" title="从 JSON 文件导入服务配置（同 id 覆盖，新 id 追加）" @click="importConfig">
          <AppIcon name="upload" :size="13" /> 导入
        </button>
        <button class="btn ghost" title="导出全部服务配置到 JSON 文件" @click="exportConfig">
          <AppIcon name="download" :size="13" /> 导出
        </button>
        <button class="btn primary add-btn" @click="openAddModal">
          <AppIcon name="plus" :size="14" />
          添加服务
        </button>
      </div>
    </header>

    <!-- 主工作区：左侧服务列表 + 右侧控制与日志 -->
    <div class="app-workspace">
      <!-- 左侧：服务列表 -->
      <aside class="services-sidebar" :style="{ width: sidebarWidth + 'px' }">
        <div class="sidebar-search">
          <span class="search-icon"><AppIcon name="search" :size="13" /></span>
          <input
            ref="sidebarSearchRef"
            v-model="serviceSearch"
            type="text"
            placeholder="过滤服务 (Ctrl+K)…"
            class="search-input"
          />
        </div>

        <!-- 快捷筛选 + 分组折叠 + 聚合入口 -->
        <div class="sidebar-quickbar">
          <div class="quick-chips">
            <button
              v-for="f in [
                { key: 'all', label: '全部' },
                { key: 'running', label: '运行中' },
                { key: 'stopped', label: '已停止' },
              ]"
              :key="f.key"
              :class="['quick-chip', { active: statusFilter === f.key }]"
              @click="setStatusFilter(f.key as any)"
            >
              {{ f.label }}
            </button>
          </div>
          <button
            class="quickbar-icon-btn"
            :title="groupsLastCollapseState ? '展开全部分组' : '折叠全部分组'"
            @click="toggleAllGroups"
          >
            <AppIcon :name="groupsLastCollapseState ? 'chevronRight' : 'chevronDown'" :size="13" />
          </button>
        </div>

        <!-- 聚合日志常驻入口（无需选中服务即可观测跨服务日志） -->
        <button
          :class="['agg-entry', { active: aggregatedView }]"
          title="同屏观测全部服务的日志流（按时间线合并，带服务色标）"
          @click="aggregatedView = !aggregatedView"
        >
          <AppIcon name="layers" :size="13" />
          聚合日志流
          <span class="agg-entry-count">{{ services.length }}</span>
        </button>

        <div class="services-list">
          <div v-for="group in groupedServices" :key="group.name" class="service-group">
            <div class="group-header">
              <span class="group-chevron" @click="toggleGroup(group.name)">
                <AppIcon :name="isGroupCollapsed(group.name) ? 'chevronRight' : 'chevronDown'" :size="12" />
              </span>
              <span class="group-name" :title="group.name" @click="toggleGroup(group.name)">{{ group.name }}</span>
              <span class="group-count" @click="toggleGroup(group.name)">{{ group.items.length }}</span>
              <span class="group-actions">
                <button
                  class="group-action-btn"
                  title="一键启动本组全部服务"
                  @click="groupStartAll(group.items)"
                >
                  <AppIcon name="play" :size="11" />
                </button>
                <button
                  class="group-action-btn stop"
                  title="一键停止本组全部服务"
                  @click="groupStopAll(group.items)"
                >
                  <AppIcon name="stop" :size="10" />
                </button>
              </span>
            </div>

            <template v-if="!isGroupCollapsed(group.name)">
            <div
              v-for="svc in group.items"
              :key="svc.id"
              :class="['service-item', { active: currentService?.id === svc.id }]"
              @click="selectService(svc)"
            >
              <!-- 状态圆点 -->
              <div
                :class="['status-dot', getStatusClass(svc.id)]"
                :title="getStatusText(svc.id)"
              ></div>

              <div class="service-meta">
                <div class="service-name-row">
                  <span class="service-name" :title="svc.name">{{ svc.name }}</span>
                  <span v-if="svc.port" class="service-port">:{{ svc.port }}</span>
                </div>
                <div class="service-sub-row">
                  <span class="type-tag">{{ getLaunchTypeLabel(svc.launchType) }}</span>
                  <span class="profile-preview" :title="svc.selectedProfiles.join(',')">
                    {{ svc.selectedProfiles.join(',') || 'default' }}
                  </span>
                </div>
              </div>

              <!-- 悬浮快捷控制 -->
              <div class="service-actions" @click.stop>
                <button
                  v-if="!isRunning(svc.id) && !isStarting(svc.id) && !isBuilding(svc.id)"
                  class="action-icon-btn start-icon"
                  title="启动"
                  @click="startService(svc.id)"
                >
                  <AppIcon name="play" :size="13" />
                </button>
                <button
                  v-if="isRunning(svc.id) || isStarting(svc.id) || isBuilding(svc.id)"
                  class="action-icon-btn stop-icon"
                  title="停止"
                  @click="stopService(svc.id)"
                >
                  <AppIcon name="stop" :size="12" />
                </button>
              </div>
            </div>
            </template>
          </div>

          <div v-if="services.length === 0" class="empty-list-tip">
            暂无服务
            <button class="link-btn" @click="openAddModal">立即添加</button>
          </div>
          <div v-else-if="filteredServices.length === 0" class="empty-list-tip">
            没有匹配「{{ statusFilter === 'running' ? '运行中' : statusFilter === 'stopped' ? '已停止' : serviceSearch }}」的服务
          </div>
        </div>

        <!-- 侧栏宽度拖拽手柄 -->
        <div class="sidebar-resizer" @mousedown="onSidebarResizeStart"></div>
      </aside>

      <!-- 右侧：服务主面板 -->
      <main class="main-panel">
        <div v-if="currentService || aggregatedView" class="service-detail">
          <!-- 服务控制头部（三段式第 1、2 段） -->
          <div v-if="currentService" class="control-header">
            <div class="control-header-top">
              <div class="title-section">
                <div class="service-main-title">
                  {{ currentService.name }}
                  <span class="type-pill">
                    {{ getLaunchTypeLabel(currentService.launchType) }}
                    <template v-if="currentModule && isMavenLaunchType">
                      · {{ currentModule }}
                    </template>
                  </span>
                  <!-- 运行状态 + 健康组合徽章 -->
                  <span :class="['state-badge', getStatusClass(currentService.id)]">
                    {{ getStatusText(currentService.id) }}
                  </span>
                  <span
                    v-if="getHealthBadge(currentService.id)"
                    :class="['task-badge', 'health-badge', getHealthBadge(currentService.id)!.cls]"
                    title="健康探针：周期性 GET 健康检查端点"
                  >
                    <AppIcon name="activity" :size="11" />
                    {{ getHealthBadge(currentService.id)!.text }}
                  </span>
                </div>
                <div class="service-work-dir" :title="currentService.workDir">
                  <AppIcon name="folder" :size="12" />
                  <span class="dir-text">{{ currentService.workDir }}</span>
                </div>
              </div>

              <!-- 右侧：生命周期按钮 + 次级图标操作 -->
              <div class="service-mgmt-btns">
                <div class="action-btn-group">
                  <button
                    class="btn primary action-btn"
                    :disabled="
                      isRunning(currentService.id) ||
                      isStarting(currentService.id) ||
                      isStopping(currentService.id) ||
                      isBuilding(currentService.id) ||
                      isMavenTaskRunning(currentService.id)
                    "
                    @click="startCurrentService"
                  >
                    <AppIcon name="play" :size="12" /> 启动
                  </button>
                  <button
                    class="btn danger action-btn"
                    :disabled="
                      !isRunning(currentService.id) &&
                      !isStarting(currentService.id) &&
                      !isBuilding(currentService.id)
                    "
                    @click="stopCurrentService"
                  >
                    <AppIcon name="stop" :size="11" /> 停止
                  </button>
                  <button
                    class="btn secondary action-btn"
                    :disabled="!isRunning(currentService.id) && !isBuilding(currentService.id)"
                    @click="restartCurrentService"
                  >
                    <AppIcon name="restart" :size="13" /> 重启
                  </button>
                </div>
                <span class="mgmt-divider"></span>
                <button class="btn ghost sm icon-only-btn" title="编辑服务配置" @click="openEditModal">
                  <AppIcon name="edit" :size="13" />
                </button>
                <button class="btn ghost sm icon-only-btn" title="克隆服务（起多实例调试）" @click="cloneCurrentService">
                  <AppIcon name="copy" :size="13" />
                </button>
                <button class="btn ghost danger-ghost sm icon-only-btn" title="删除服务" @click="confirmDeleteService">
                  <AppIcon name="trash" :size="13" />
                </button>
              </div>
            </div>

            <!-- 次级信息与工具条（三段式第 2 段） -->
            <div class="control-header-bottom">
              <!-- 当前实际运行 Profile -->
              <div class="profile-box">
                <span class="box-label">运行 Profile</span>
                <span :class="['active-profile-badge', { running: isRunning(currentService.id) }]">
                  {{ getActiveProfileDisplay(currentService.id) }}
                </span>
              </div>

              <!-- 下次生效 Profile 选择/输入框 -->
              <div class="profile-box next-profile-box">
                <span class="box-label">下次启动</span>
                <div class="profile-selector-wrap">
                  <select
                    v-model="profileSelectValue"
                    class="profile-select"
                    @change="onProfileSelectChange"
                  >
                    <option value="default">default (不显式指定)</option>
                    <option
                      v-for="p in currentServiceDetectedProfiles"
                      :key="p"
                      :value="p"
                    >
                      {{ p }}
                    </option>
                    <option value="custom">自定义多配置…</option>
                  </select>
                  <input
                    v-if="isCustomProfileInput"
                    v-model="customProfileInput"
                    type="text"
                    class="profile-custom-input"
                    placeholder="如 dev,local"
                    @blur="onCustomProfileBlur"
                  />
                </div>
                <span
                  v-if="profilePendingRestart"
                  class="pending-restart-badge"
                  title="下次启动的 Profile 与当前运行不一致，重启后生效"
                >
                  ● 重启生效
                </span>
              </div>

              <!-- 运行态遥测：端口（可点击打开浏览器）+ PID + 指标 -->
              <div class="telemetry-box">
                <button
                  v-if="currentService.port"
                  class="telemetry-chip clickable"
                  title="在浏览器打开 http://127.0.0.1:{{ currentService.port }}"
                  @click="openServiceUrl(currentService.port)"
                >
                  <AppIcon name="externalLink" :size="11" /> :{{ currentService.port }}
                </button>
                <span v-if="getPid(currentService.id)" class="telemetry-chip">
                  PID {{ getPid(currentService.id) }}
                </span>
                <div v-if="currentMetrics" class="metrics-box" title="进程树实时占用（每 2s 采样）">
                  <svg class="spark" width="72" height="18" viewBox="0 0 72 18" preserveAspectRatio="none">
                    <polyline
                      v-if="cpuSparkPoints"
                      :points="cpuSparkPoints"
                      fill="none"
                      stroke="var(--accent)"
                      stroke-width="1.5"
                    />
                  </svg>
                  <span class="metrics-text">
                    CPU {{ currentMetrics.cpuPercent !== null ? currentMetrics.cpuPercent.toFixed(1) : '—' }}%
                    · {{ currentMetrics.memoryMb }} MB
                    · {{ currentMetrics.processCount }} 进程
                  </span>
                </div>
              </div>

              <!-- Maven 工具箱 + 项目终端 -->
              <div class="maven-toolbox">
                <button
                  class="btn ghost action-btn terminal-btn"
                  title="在项目目录唤起终端（Windows Terminal / cmd）"
                  @click="openTerminal"
                >
                  <AppIcon name="terminal" :size="13" /> 终端
                </button>
                <template v-if="isMavenLaunchType">
                  <template v-if="!isMavenTaskRunning(currentService.id)">
                    <div class="mvn-segment">
                      <button
                        v-for="g in ['clean', 'compile', 'package', 'install']"
                        :key="g"
                        class="mvn-seg-btn"
                        :disabled="!canRunMavenTask"
                        :title="
                          canRunMavenTask
                            ? `mvn ${g}（全工程构建，输出进日志视窗）`
                            : '服务运行中或构建进行中，请先停止后再执行构建'
                        "
                        @click="runMavenTask(g)"
                      >
                        {{ g }}
                      </button>
                    </div>
                  </template>
                  <template v-else>
                    <span class="task-badge running">
                      <span class="spinner"></span>
                      mvn {{ getMavenTaskGoal(currentService.id) }} 构建中…
                    </span>
                    <button class="btn danger action-btn" @click="stopMavenTask">
                      <AppIcon name="stop" :size="11" /> 停止构建
                    </button>
                  </template>
                </template>
                <span v-if="mavenTaskBadge" :class="['task-badge', mavenTaskBadge.cls]">
                  {{ mavenTaskBadge.text }}
                </span>
              </div>
            </div>
          </div>

          <!-- 日志视窗区 -->
          <div class="console-section">
            <div class="console-toolbar">
              <div class="toolbar-left">
                <span class="console-title"><AppIcon name="terminal" :size="13" /> 实时控制台</span>
                <div v-if="currentService && services.length > 1" class="view-switch">
                  <button
                    :class="['view-switch-btn', { active: !aggregatedView }]"
                    @click="aggregatedView = false"
                  >
                    单服务
                  </button>
                  <button
                    :class="['view-switch-btn', { active: aggregatedView }]"
                    @click="aggregatedView = true"
                  >
                    聚合全部
                  </button>
                </div>
                <div v-if="!currentService" class="console-title-sub">全部服务聚合视图</div>
                <!-- 日志级别快速过滤胶囊 -->
                <div class="level-switch">
                  <button
                    v-for="lv in ['ALL', 'INFO', 'WARN', 'ERROR']"
                    :key="lv"
                    :class="['level-chip', lv.toLowerCase(), { active: levelFilter === lv }]"
                    @click="levelFilter = levelFilter === lv ? 'ALL' : (lv as any)"
                  >
                    {{ lv }}
                  </button>
                </div>
                <span class="log-count">{{ logMatchCount }}</span>
                <span v-if="aggregatedView" class="log-count">· {{ services.length }} 服务</span>
              </div>
              <div class="toolbar-right">
                <div class="log-filter-wrap">
                  <span class="log-filter-icon"><AppIcon name="filter" :size="12" /></span>
                  <input
                    ref="logFilterRef"
                    v-model="logFilter"
                    type="text"
                    placeholder="搜日志 (Ctrl+F)…"
                    class="log-filter-input"
                  />
                </div>
                <label class="auto-scroll-label" title="新日志自动滚动到底部；上翻历史时自动暂停">
                  <span class="checkbox-fake"><AppIcon name="check" :size="10" :stroke-width="3" /></span>
                  <input type="checkbox" v-model="autoScroll" />
                  自动滚屏
                </label>
                <button class="btn ghost sm" @click="clearLogs">
                  <AppIcon name="trash" :size="12" /> 清空
                </button>
                <button class="btn ghost sm" @click="copyLogs">
                  <AppIcon name="copy" :size="12" />
                  {{ copyTip || '复制' }}
                </button>
              </div>
            </div>

            <!-- 终端日志内容 -->
            <div ref="terminalRef" class="terminal-container" @scroll="onScroll">
              <div
                v-for="(log, idx) in filteredLogs"
                :key="idx"
                :class="['log-row', getLogLevelClass(log.level)]"
              >
                <span
                  v-if="aggregatedView && log.svcId"
                  class="svc-tag"
                  :style="{ color: svcColor(log.svcId), borderColor: svcColor(log.svcId) }"
                  :title="log.svcName"
                >
                  {{ (log.svcName || '').slice(0, 12) }}
                </span>
                <span class="log-time">{{ log.timestamp }}</span>
                <span :class="['log-level', log.level]">{{ formatLevel(log.level) }}</span>
                <span class="log-content">{{ log.content }}</span>
              </div>
              <div v-if="filteredLogs.length === 0" class="empty-log-tip">
                {{
                  logFilter || levelFilter !== 'ALL'
                    ? '未匹配到符合过滤条件的日志'
                    : '等待服务启动或输出日志…'
                }}
              </div>

              <!-- 脱底后回到底部悬浮胶囊 -->
              <button
                v-if="!autoScroll && pendingLogCount > 0"
                class="jump-bottom-pill"
                @click="jumpToBottom"
              >
                <AppIcon name="arrowDown" :size="12" />
                回到底部（{{ pendingLogCount }} 条新日志）
              </button>
            </div>
          </div>
        </div>

        <!-- 冷启动引导页：无服务且未开聚合视图 -->
        <div v-else class="empty-state">
          <div class="empty-hero">
            <div class="empty-hero-icon">
              <AppIcon name="bolt" :size="26" :stroke-width="1.8" />
            </div>
            <div class="empty-title">欢迎使用 Local Service Runner</div>
            <div class="empty-subtitle">把 Spring Boot 服务收进一个窗口：启动、停止、Profile 切换与实时日志</div>
          </div>

          <div class="empty-actions">
            <button class="btn primary empty-primary-btn" @click="openAddModal">
              <AppIcon name="plus" :size="14" /> 添加第一个服务
            </button>
            <button class="btn secondary" @click="importConfig">
              <AppIcon name="upload" :size="13" /> 导入配置文件 (JSON)
            </button>
          </div>

          <div :class="['empty-dropzone', { over: dragOver }]">
            <AppIcon name="folder" :size="22" :stroke-width="1.5" />
            <div class="dropzone-text">
              将 Spring Boot / Maven 项目文件夹拖到此处
              <span class="dropzone-sub">自动识别模块与 Profile，直接预填配置</span>
            </div>
          </div>

          <div class="empty-features">
            <div class="feature-card">
              <AppIcon name="shield" :size="16" />
              <div class="feature-title">进程树精准回收</div>
              <div class="feature-desc">Job Object 内核级治理，不留孤儿进程</div>
            </div>
            <div class="feature-card">
              <AppIcon name="sliders" :size="16" />
              <div class="feature-title">多 Profile 独立切换</div>
              <div class="feature-desc">运行/待生效解耦，重启即生效</div>
            </div>
            <div class="feature-card">
              <AppIcon name="package" :size="16" />
              <div class="feature-title">内置 Maven 工具箱</div>
              <div class="feature-desc">clean / compile / package / install 一键直达</div>
            </div>
          </div>
        </div>
      </main>
    </div>

    <!-- 添加 / 编辑模态框 -->
    <ServiceModal
      :is-open="isModalOpen"
      :service-data="editingService"
      :existing-groups="allGroupNames"
      :initial-work-dir="pendingWorkDir"
      @close="onModalClosed"
      @saved="onServiceSaved"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount, nextTick } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import ServiceModal from './components/ServiceModal.vue';
import AppIcon from './components/AppIcon.vue';
import type {
  HealthChangeEvent,
  HealthState,
  ImportSummary,
  MavenProjectInfo,
  MavenTaskStatus,
  MavenTaskStatusEvent,
  ServiceConfig,
  ServiceMetrics,
  ServiceStatus,
  LogEntry,
  ServiceLogEvent,
  StatusChangeEvent,
} from './types';

// 服务列表与当前选中
const services = ref<ServiceConfig[]>([]);
const currentService = ref<ServiceConfig | null>(null);
const serviceSearch = ref('');

// 侧栏状态快捷筛选（全部 / 运行中 / 已停止）与顶栏胶囊联动
const statusFilter = ref<'all' | 'running' | 'stopped'>('all');

// 分组折叠：记录被折叠的分组名
const collapsedGroups = ref<Set<string>>(new Set());

// 侧栏宽度拖拽（220 ~ 400px，localStorage 持久化）
const SIDEBAR_W_KEY = 'lsr-sidebar-width';
const sidebarWidth = ref<number>(
  Math.min(400, Math.max(220, Number(localStorage.getItem(SIDEBAR_W_KEY)) || 282))
);
let sidebarResizing = false;

// 日志级别过滤（ALL / INFO / WARN / ERROR）
const levelFilter = ref<'ALL' | 'INFO' | 'WARN' | 'ERROR'>('ALL');

// 脱底后新增日志计数（回到底部胶囊用）
const pendingLogCount = ref(0);

// 拖拽项目文件夹建服务：拖拽悬停高亮 + 预填目录
const dragOver = ref(false);
const pendingWorkDir = ref('');

// 状态字典：serviceId -> ServiceStatus
const statusMap = ref<Record<string, ServiceStatus>>({});

// 日志字典：serviceId -> LogEntry[]
const logsMap = ref<Record<string, LogEntry[]>>({});

// 当前服务检测到的 Profiles
const currentServiceDetectedProfiles = ref<string[]>([]);
// 当前服务实际使用的 Maven 启动模块（配置为空时取自动探测结果）
const currentModule = ref('');
const profileSelectValue = ref('dev');
const isCustomProfileInput = ref(false);
const customProfileInput = ref('');

// Maven 工具箱任务状态：serviceId -> MavenTaskStatus
const mavenTaskMap = ref<Record<string, MavenTaskStatus>>({});

// 健康探针状态：serviceId -> HealthState
const healthMap = ref<Record<string, HealthState>>({});

// 聚合日志视图：跨服务同屏观测
const aggregatedView = ref(false);

// 当前服务实时指标与历史采样（sparkline 用）
const currentMetrics = ref<ServiceMetrics | null>(null);
const metricsHistory = ref<Array<{ cpu: number | null; mem: number }>>([]);
let metricsTimer: ReturnType<typeof setInterval> | null = null;

// 日志视窗控制
const logFilter = ref('');
const autoScroll = ref(true);
const terminalRef = ref<HTMLElement | null>(null);
const copyTip = ref('');

// 模态框控制
const isModalOpen = ref(false);
const editingService = ref<ServiceConfig | null>(null);

const runningCount = computed(() => {
  return Object.values(statusMap.value).filter((s) => s.type === 'running').length;
});

const failedCount = computed(() => {
  return Object.values(statusMap.value).filter((s) => s.type === 'failed').length;
});

const stoppedCount = computed(
  () => services.value.length - runningCount.value - failedCount.value
);

const filteredServices = computed(() => {
  const q = serviceSearch.value.trim().toLowerCase();
  return services.value.filter((s) => {
    if (q && !(s.name.toLowerCase().includes(q) || s.workDir.toLowerCase().includes(q))) {
      return false;
    }
    const status = statusMap.value[s.id];
    const type = status?.type || 'stopped';
    if (statusFilter.value === 'running') return type === 'running' || type === 'starting' || type === 'building';
    if (statusFilter.value === 'stopped') return type === 'stopped' || type === 'failed';
    return true;
  });
});

const currentLogs = computed(() => {
  if (!currentService.value) return [];
  return logsMap.value[currentService.value.id] || [];
});

// 视图日志：聚合模式取所有服务合并，单服务模式取当前服务
const viewLogs = computed<Array<LogEntry & { svcId?: string; svcName?: string }>>(() => {
  if (!aggregatedView.value) {
    const list = currentLogs.value;
    const svc = currentService.value;
    return svc ? list.map((l) => ({ ...l, svcId: svc.id, svcName: svc.name })) : list;
  }
  const result: Array<LogEntry & { svcId: string; svcName: string }> = [];
  for (const [svcId, list] of Object.entries(logsMap.value)) {
    const svc = services.value.find((s) => s.id === svcId);
    if (!svc) continue;
    for (const l of list) {
      result.push({ ...l, svcId, svcName: svc.name });
    }
  }
  result.sort((a, b) => a.timestamp.localeCompare(b.timestamp));
  return result;
});

const filteredLogs = computed(() => {
  let all: Array<LogEntry & { svcId?: string; svcName?: string }> = viewLogs.value;
  // 级别过滤胶囊（ALL / INFO / WARN / ERROR）
  if (levelFilter.value !== 'ALL') {
    const lv = levelFilter.value;
    all = all.filter((l) => l.level === lv);
  }
  const q = logFilter.value.trim().toLowerCase();
  if (!q) return all;
  return all.filter(
    (l) =>
      l.content.toLowerCase().includes(q) ||
      (l.svcName || '').toLowerCase().includes(q)
  );
});

// 日志匹配计数（x/y）
const logMatchCount = computed(() => `${filteredLogs.value.length}/${viewLogs.value.length}`);

// 待生效 Profile 与运行中不一致 → 琥珀色「重启生效」提示
const profilePendingRestart = computed(() => {
  if (!currentService.value) return false;
  const status = statusMap.value[currentService.value.id];
  if (!status || status.type !== 'running') return false;
  const next = (currentService.value.selectedProfiles || [])
    .map((p) => p.trim())
    .filter((p) => p && p.toLowerCase() !== 'default');
  const running = status.activeProfiles || [];
  return next.join(',') !== running.join(',');
});

// 侧栏分组视图：按 group 字段分节，「未分组」排在最后
const groupedServices = computed<Array<{ name: string; items: ServiceConfig[] }>>(() => {
  const map = new Map<string, ServiceConfig[]>();
  for (const s of filteredServices.value) {
    const g = (s.group || '').trim() || '未分组';
    if (!map.has(g)) map.set(g, []);
    map.get(g)!.push(s);
  }
  const entries = [...map.entries()];
  entries.sort((a, b) => {
    const ua = a[0] === '未分组';
    const ub = b[0] === '未分组';
    if (ua !== ub) return ua ? 1 : -1;
    return a[0].localeCompare(b[0]);
  });
  return entries.map(([name, items]) => ({ name, items }));
});

// 当前服务是否 Maven 系启动方式（决定是否展示 Maven 工具箱）
const isMavenLaunchType = computed(() => {
  if (!currentService.value) return false;
  return ['maven-wrapper', 'maven-build-jar', 'source-main-class'].includes(
    currentService.value.launchType
  );
});

// 能否对当前服务发起 Maven 构建任务：服务处于停止/异常状态且没有任务在跑
const canRunMavenTask = computed(() => {
  if (!currentService.value) return false;
  const id = currentService.value.id;
  const status = statusMap.value[id];
  const serviceIdle =
    !status || status.type === 'stopped' || status.type === 'failed';
  return serviceIdle && !isMavenTaskRunning(id);
});

// Maven 工具箱最近一次任务结果徽章
const mavenTaskBadge = computed<{ text: string; cls: string } | null>(() => {
  if (!currentService.value) return null;
  const status = mavenTaskMap.value[currentService.value.id];
  if (!status) return null;
  if (status.type === 'success') {
    return { text: `mvn ${status.goal} 成功`, cls: 'ok' };
  }
  if (status.type === 'failed') {
    return { text: `mvn ${status.goal} 失败 (exit ${status.exitCode})`, cls: 'err' };
  }
  return null;
});

// CPU 采样 sparkline 折线点
const cpuSparkPoints = computed(() => {
  const n = metricsHistory.value.length;
  if (n < 2) return '';
  const W = 72;
  const H = 18;
  return metricsHistory.value
    .map((m, i) => {
      const x = (i / (n - 1)) * W;
      const y = H - (Math.min(m.cpu ?? 0, 100) / 100) * H;
      return `${x.toFixed(1)},${y.toFixed(1)}`;
    })
    .join(' ');
});

// 服务标签色块（聚合日志视图中区分不同服务）
const SVC_COLORS = [
  '#818cf8', '#f472b6', '#34d399', '#fbbf24', '#60a5fa',
  '#c084fc', '#2dd4bf', '#fb923c', '#a5b4fc', '#4ade80',
];
function svcColor(id: string): string {
  let h = 0;
  for (let i = 0; i < id.length; i++) h = (h * 31 + id.charCodeAt(i)) >>> 0;
  return SVC_COLORS[h % SVC_COLORS.length];
}

// 全部已使用的分组名（配置弹窗 datalist 补全）
const allGroupNames = computed(() => {
  const set = new Set<string>();
  for (const s of services.value) {
    const g = (s.group || '').trim();
    if (g) set.add(g);
  }
  return [...set];
});

onMounted(async () => {
  await loadServices();
  await loadAllStatuses();

  // 监听后端日志推送
  await listen<ServiceLogEvent>('service-log', (event) => {
    const { serviceId, entry } = event.payload;
    if (!logsMap.value[serviceId]) {
      logsMap.value[serviceId] = [];
    }
    const list = logsMap.value[serviceId];
    if (list.length >= 5000) {
      list.shift();
    }
    list.push(entry);

    const visible = aggregatedView.value || currentService.value?.id === serviceId;
    if (!visible) return;
    if (autoScroll.value) {
      scrollToBottom();
    } else {
      // 用户在翻阅历史日志：暂停滚底并计数新日志
      pendingLogCount.value += 1;
    }
  });

  // 监听后端状态变更推送
  await listen<StatusChangeEvent>('service-status-change', (event) => {
    const { serviceId, status } = event.payload;
    statusMap.value[serviceId] = status;
    // 选中服务的运行状态变化 → 同步指标轮询
    if (currentService.value?.id === serviceId) {
      syncMetricsPolling();
      if (status.type !== 'running') {
        currentMetrics.value = null;
      }
    }
  });

  // 监听 Maven 工具箱任务状态变更
  await listen<MavenTaskStatusEvent>('maven-task-status-change', (event) => {
    const { serviceId, status } = event.payload;
    mavenTaskMap.value[serviceId] = status;
  });

  // 监听健康探针状态推送
  await listen<HealthChangeEvent>('service-health-change', (event) => {
    const { serviceId, health } = event.payload;
    healthMap.value[serviceId] = health;
  });

  // 拖拽项目文件夹快速创建服务（仅 Tauri WebView 环境可用）
  try {
    const webview = getCurrentWebview();
    await webview.onDragDropEvent((event) => {
      if (event.payload.type === 'enter' || event.payload.type === 'over') {
        dragOver.value = true;
      } else {
        dragOver.value = false;
      }
      if (event.payload.type === 'drop') {
        const paths = (event.payload as { paths: string[] }).paths || [];
        if (paths.length === 1) handleFolderDrop(paths[0]);
      }
    });
  } catch (err) {
    console.info('Drag-drop not available (browser preview):', err);
  }

  // 全局快捷键
  window.addEventListener('keydown', onGlobalKeydown);
});

onBeforeUnmount(() => {
  window.removeEventListener('keydown', onGlobalKeydown);
  if (metricsTimer !== null) {
    clearInterval(metricsTimer);
    metricsTimer = null;
  }
});

/** 全局快捷键：Ctrl+K 过滤 / Ctrl+N 新建 / Ctrl+R 重启 / Ctrl+L 清空日志 / Ctrl+F 搜日志 */
function onGlobalKeydown(e: KeyboardEvent) {
  if (!(e.ctrlKey || e.metaKey)) return;
  const key = e.key.toLowerCase();
  if (key === 'k') {
    e.preventDefault();
    sidebarSearchRef.value?.focus();
    sidebarSearchRef.value?.select();
  } else if (key === 'n') {
    e.preventDefault();
    if (!isModalOpen.value) openAddModal();
  } else if (key === 'r') {
    if (currentService.value && isRunning(currentService.value.id)) {
      e.preventDefault();
      restartCurrentService();
    }
  } else if (key === 'l') {
    e.preventDefault();
    clearLogs();
  } else if (key === 'f') {
    e.preventDefault();
    logFilterRef.value?.focus();
    logFilterRef.value?.select();
  }
}

const sidebarSearchRef = ref<HTMLInputElement | null>(null);
const logFilterRef = ref<HTMLInputElement | null>(null);

/** 拖入项目文件夹 → 校验目录后弹新建弹窗并预填路径 */
async function handleFolderDrop(path: string) {
  try {
    const isDir = await invoke<boolean>('path_is_dir', { path });
    if (!isDir) {
      alert('拖入的不是文件夹，请拖拽 Spring Boot / Maven 项目目录');
      return;
    }
    pendingWorkDir.value = path;
    openAddModal();
  } catch (err: any) {
    console.error('handleFolderDrop failed:', err);
  }
}

/** 侧栏宽度拖拽 */
function onSidebarResizeStart(e: MouseEvent) {
  sidebarResizing = true;
  e.preventDefault();
}
function onGlobalMouseMove(e: MouseEvent) {
  if (!sidebarResizing) return;
  const w = Math.min(400, Math.max(220, e.clientX));
  sidebarWidth.value = w;
}
function onGlobalMouseUp() {
  if (!sidebarResizing) return;
  sidebarResizing = false;
  localStorage.setItem(SIDEBAR_W_KEY, String(sidebarWidth.value));
}

// 挂载期常驻监听拖拽调宽（document 级别）
if (typeof document !== 'undefined') {
  document.addEventListener('mousemove', onGlobalMouseMove);
  document.addEventListener('mouseup', onGlobalMouseUp);
}

/** 分组折叠/展开 */
function toggleGroup(name: string) {
  const set = new Set(collapsedGroups.value);
  if (set.has(name)) set.delete(name);
  else set.add(name);
  collapsedGroups.value = set;
}

function isGroupCollapsed(name: string): boolean {
  return collapsedGroups.value.has(name);
}

/** 一键折叠/展开全部分组 */
let groupsLastCollapseState = false;
function toggleAllGroups() {
  const allCollapsed = groupedServices.value.every((g) => collapsedGroups.value.has(g.name));
  if (allCollapsed && groupedServices.value.length > 0) {
    collapsedGroups.value = new Set();
    groupsLastCollapseState = false;
  } else {
    collapsedGroups.value = new Set(groupedServices.value.map((g) => g.name));
    groupsLastCollapseState = true;
  }
}

/** 侧栏无服务时的单行轻提示（带一键新建） */
function setStatusFilter(f: 'all' | 'running' | 'stopped') {
  statusFilter.value = statusFilter.value === f ? 'all' : f;
}

/** 点端口徽章 → 浏览器打开 http://127.0.0.1:{port} */
async function openServiceUrl(port?: number) {
  if (!port) return;
  try {
    await invoke('open_url', { url: `http://127.0.0.1:${port}` });
  } catch (err: any) {
    alert('打开浏览器失败: ' + (err?.message || err));
  }
}

/** 回到底部（悬浮胶囊） */
function jumpToBottom() {
  autoScroll.value = true;
  pendingLogCount.value = 0;
  scrollToBottom();
}

async function loadServices() {
  try {
    const list = await invoke<ServiceConfig[]>('get_services');
    services.value = list;
    if (list.length > 0 && !currentService.value) {
      selectService(list[0]);
    }
    // 预加载所有服务日志，聚合视图可随时切换
    for (const svc of list) {
      if (!logsMap.value[svc.id]) {
        try {
          logsMap.value[svc.id] = await invoke<LogEntry[]>('get_service_logs', { id: svc.id });
        } catch (err) {
          console.error('Failed to preload logs:', err);
        }
      }
    }
  } catch (err) {
    console.error('Failed to load services:', err);
  }
}

async function loadAllStatuses() {
  try {
    const statuses = await invoke<Record<string, ServiceStatus>>('get_all_service_statuses');
    statusMap.value = statuses;
    // 状态加载完成后再同步一次指标轮询（启动时已在运行的服务不会有状态事件触发）
    syncMetricsPolling();
  } catch (err) {
    console.error('Failed to load statuses:', err);
  }
}

async function selectService(svc: ServiceConfig) {
  currentService.value = svc;
  // 同步 Profile 选择框
  initProfileSelector(svc);

  // 加载该服务的日志
  if (!logsMap.value[svc.id]) {
    try {
      const logs = await invoke<LogEntry[]>('get_service_logs', { id: svc.id });
      logsMap.value[svc.id] = logs;
    } catch (err) {
      console.error('Failed to load logs:', err);
    }
  }

  // 加载该服务的 Maven 工具箱任务状态
  try {
    const taskStatus = await invoke<MavenTaskStatus>('get_maven_task_status', { id: svc.id });
    mavenTaskMap.value[svc.id] = taskStatus;
  } catch (err) {
    console.error('Failed to load maven task status:', err);
  }

  // 加载该服务的健康探针状态
  try {
    healthMap.value[svc.id] = await invoke<HealthState>('get_service_health', { id: svc.id });
  } catch (err) {
    console.error('Failed to load health state:', err);
  }

  // 指标采样随选中服务切换重置
  currentMetrics.value = null;
  metricsHistory.value = [];
  syncMetricsPolling();

  // 扫描该服务支持的 profiles（多模块工程需要定位到启动模块的 resources）
  currentModule.value = (svc.mavenModule || '').trim();
  try {
    if (!currentModule.value && isMavenLaunchTypeOf(svc)) {
      const info = await invoke<MavenProjectInfo>('detect_maven_modules', {
        workDir: svc.workDir,
      });
      currentModule.value = info.suggestedModule || '';
    }
    const profiles = await invoke<string[]>('detect_profiles', {
      workDir: svc.workDir,
      module: currentModule.value,
    });
    currentServiceDetectedProfiles.value = profiles;
  } catch (err) {
    currentServiceDetectedProfiles.value = [];
  }

  scrollToBottom();
}

/** 判断服务的启动类型是否为 Maven 系（供模块探测使用） */
function isMavenLaunchTypeOf(svc: ServiceConfig): boolean {
  return ['maven-wrapper', 'maven-build-jar', 'source-main-class'].includes(svc.launchType);
}

function initProfileSelector(svc: ServiceConfig) {
  const profiles = svc.selectedProfiles || [];
  if (profiles.length === 0 || (profiles.length === 1 && profiles[0] === 'default')) {
    profileSelectValue.value = 'default';
    isCustomProfileInput.value = false;
  } else if (profiles.length === 1 && ['dev', 'test', 'prod', 'local'].includes(profiles[0])) {
    profileSelectValue.value = profiles[0];
    isCustomProfileInput.value = false;
  } else {
    profileSelectValue.value = 'custom';
    isCustomProfileInput.value = true;
    customProfileInput.value = profiles.join(',');
  }
}

async function onProfileSelectChange() {
  if (!currentService.value) return;
  if (profileSelectValue.value === 'custom') {
    isCustomProfileInput.value = true;
    customProfileInput.value = currentService.value.selectedProfiles.join(',');
  } else {
    isCustomProfileInput.value = false;
    currentService.value.selectedProfiles = [profileSelectValue.value];
    await saveCurrentServiceConfig();
  }
}

async function onCustomProfileBlur() {
  if (!currentService.value) return;
  const raw = customProfileInput.value
    .split(',')
    .map((p) => p.trim())
    .filter(Boolean);
  currentService.value.selectedProfiles = raw.length > 0 ? raw : ['default'];
  await saveCurrentServiceConfig();
}

async function saveCurrentServiceConfig() {
  if (!currentService.value) return;
  try {
    await invoke('save_service', { config: currentService.value });
    // 更新本地列表对应项
    const idx = services.value.findIndex((s) => s.id === currentService.value?.id);
    if (idx !== -1) {
      services.value[idx] = { ...currentService.value };
    }
  } catch (err) {
    console.error('Failed to save service profile:', err);
  }
}

// 启动 / 停止 / 重启
async function startCurrentService() {
  if (!currentService.value) return;
  await startService(currentService.value.id);
}

async function startService(id: string) {
  try {
    await invoke('start_service', { id });
  } catch (err: any) {
    alert('启动失败: ' + (err?.message || err));
  }
}

async function stopCurrentService() {
  if (!currentService.value) return;
  await stopService(currentService.value.id);
}

async function stopService(id: string) {
  try {
    await invoke('stop_service', { id });
  } catch (err: any) {
    alert('停止失败: ' + (err?.message || err));
  }
}

async function restartCurrentService() {
  if (!currentService.value) return;
  try {
    await invoke('restart_service', { id: currentService.value.id });
  } catch (err: any) {
    alert('重启失败: ' + (err?.message || err));
  }
}

// Maven 工具箱任务
async function runMavenTask(goal: string) {
  if (!currentService.value) return;
  try {
    await invoke('run_maven_task', { id: currentService.value.id, goal });
  } catch (err: any) {
    alert('构建任务启动失败: ' + (err?.message || err));
  }
}

async function stopMavenTask() {
  if (!currentService.value) return;
  try {
    await invoke('stop_maven_task', { id: currentService.value.id });
  } catch (err: any) {
    alert('停止构建失败: ' + (err?.message || err));
  }
}

function isMavenTaskRunning(id: string) {
  return mavenTaskMap.value[id]?.type === 'running';
}

function getMavenTaskGoal(id: string): string {
  const status = mavenTaskMap.value[id];
  if (status && status.type === 'running') {
    return status.goal;
  }
  return '';
}

// 项目终端
async function openTerminal() {
  if (!currentService.value) return;
  try {
    await invoke('open_service_terminal', { id: currentService.value.id });
  } catch (err: any) {
    alert('打开终端失败: ' + (err?.message || err));
  }
}

// 分组一键启停：按列表顺序串行触发
async function groupStartAll(items: ServiceConfig[]) {
  for (const svc of items) {
    if (!isRunning(svc.id) && !isStarting(svc.id) && !isBuilding(svc.id)) {
      await invoke('start_service', { id: svc.id }).catch(() => {});
    }
  }
}

async function groupStopAll(items: ServiceConfig[]) {
  for (const svc of items) {
    if (isRunning(svc.id) || isStarting(svc.id) || isBuilding(svc.id)) {
      await invoke('stop_service', { id: svc.id }).catch(() => {});
    }
  }
}

// 健康探针徽章
function getHealthBadge(id: string): { text: string; cls: string } | null {
  if (!isRunning(id)) return null;
  const health = healthMap.value[id];
  if (!health || health === 'unknown') return null;
  if (health === 'up') return { text: '健康', cls: 'ok' };
  if (health === 'down') return { text: '不可用', cls: 'err' };
  return { text: '检测中', cls: 'running' };
}

// 指标轮询：仅当前选中服务处于 Running 时开启
function syncMetricsPolling() {
  const running =
    !!currentService.value &&
    (statusMap.value[currentService.value.id]?.type === 'running');

  if (running && metricsTimer === null) {
    metricsTimer = setInterval(async () => {
      if (!currentService.value) return;
      try {
        const metrics = await invoke<ServiceMetrics | null>('get_service_metrics', {
          id: currentService.value!.id,
        });
        if (metrics) {
          currentMetrics.value = metrics;
          metricsHistory.value.push({ cpu: metrics.cpuPercent, mem: metrics.memoryMb });
          if (metricsHistory.value.length > 40) {
            metricsHistory.value.shift();
          }
        }
      } catch (err) {
        console.error('Failed to load metrics:', err);
      }
    }, 2000);
  } else if (!running && metricsTimer !== null) {
    clearInterval(metricsTimer);
    metricsTimer = null;
  }
}

// 配置导入 / 导出 / 克隆
async function exportConfig() {
  try {
    const path = await invoke<string | null>('export_services');
    if (path) {
      alert(`已导出 ${services.value.length} 个服务配置到:\n${path}`);
    }
  } catch (err: any) {
    alert('导出失败: ' + (err?.message || err));
  }
}

async function importConfig() {
  try {
    const summary = await invoke<ImportSummary | null>('import_services');
    if (summary) {
      await loadServices();
      if (currentService.value) {
        const refreshed = services.value.find((s) => s.id === currentService.value!.id);
        if (refreshed) selectService(refreshed);
      }
      alert(
        `导入完成：新增 ${summary.added} 个，更新 ${summary.updated} 个，当前共 ${summary.total} 个服务`
      );
    }
  } catch (err: any) {
    alert('导入失败: ' + (err?.message || err));
  }
}

async function cloneCurrentService() {
  if (!currentService.value) return;
  try {
    const cloned = await invoke<ServiceConfig>('clone_service', {
      id: currentService.value.id,
    });
    await loadServices();
    const fresh = services.value.find((s) => s.id === cloned.id);
    if (fresh) {
      selectService(fresh);
      // 克隆后直接进入编辑：改名 / 改端口一气呵成
      openEditModal();
    }
  } catch (err: any) {
    alert('克隆失败: ' + (err?.message || err));
  }
}

function isRunning(id: string) {
  return statusMap.value[id]?.type === 'running';
}

function isStarting(id: string) {
  return statusMap.value[id]?.type === 'starting';
}

function isStopping(id: string) {
  return statusMap.value[id]?.type === 'stopping';
}

function isBuilding(id: string) {
  return statusMap.value[id]?.type === 'building';
}

function getStatusClass(id: string) {
  const status = statusMap.value[id];
  if (!status) return 'stopped';
  if (status.type === 'building') return 'starting';
  return status.type;
}

function getStatusText(id: string) {
  const status = statusMap.value[id];
  if (!status) return '已停止';
  switch (status.type) {
    case 'running':
      return '运行中';
    case 'starting':
      return '启动中...';
    case 'building':
      return `构建中 (mvn ${status.goal})...`;
    case 'stopping':
      return '停止中...';
    case 'failed':
      return '异常退出';
    case 'stopped':
    default:
      return '已停止';
  }
}

function getPid(id: string): number | null {
  const status = statusMap.value[id];
  if (status && status.type === 'running' && typeof status.pid === 'number') {
    return status.pid;
  }
  return null;
}

function getActiveProfileDisplay(id: string): string {
  const status = statusMap.value[id];
  if (status && status.type === 'running') {
    // 防御后端字段缺失/大小写不一致：不能因为单个字段异常导致整个界面渲染崩溃
    const profiles = Array.isArray(status.activeProfiles)
      ? status.activeProfiles
      : [];
    return profiles.length > 0 ? profiles.join(',') : 'default';
  }
  return '-';
}

function getLaunchTypeLabel(type: string): string {
  switch (type) {
    case 'jar':
      return 'JAR';
    case 'maven-wrapper':
      return 'Maven';
    case 'maven-build-jar':
      return '构建+JAR';
    case 'source-main-class':
      return '源码直启';
    case 'gradle-wrapper':
      return 'Gradle';
    case 'custom':
      return 'Custom';
    default:
      return type;
  }
}

// 模态框
function openAddModal() {
  editingService.value = null;
  isModalOpen.value = true;
}

function onModalClosed() {
  isModalOpen.value = false;
  pendingWorkDir.value = '';
}

function openEditModal() {
  if (!currentService.value) return;
  pendingWorkDir.value = '';
  editingService.value = JSON.parse(JSON.stringify(currentService.value));
  isModalOpen.value = true;
}

async function onServiceSaved(saved: ServiceConfig) {
  try {
    await invoke('save_service', { config: saved });
    await loadServices();
    if (!currentService.value || currentService.value.id === saved.id) {
      selectService(saved);
    }
  } catch (err: any) {
    alert('保存失败: ' + err);
  }
}

async function confirmDeleteService() {
  if (!currentService.value) return;
  if (!confirm(`确定要删除服务 "${currentService.value.name}" 吗？如果服务正在运行将被终止。`)) {
    return;
  }
  try {
    await invoke('delete_service', { id: currentService.value.id });
    await loadServices();
    if (services.value.length > 0) {
      selectService(services.value[0]);
    } else {
      currentService.value = null;
    }
  } catch (err: any) {
    alert('删除失败: ' + err);
  }
}

// 日志控制
function clearLogs() {
  if (aggregatedView.value) {
    // 聚合模式：清空所有服务的日志
    for (const svcId of Object.keys(logsMap.value)) {
      logsMap.value[svcId] = [];
      invoke('clear_service_logs', { id: svcId }).catch(console.error);
    }
    return;
  }
  if (!currentService.value) return;
  const id = currentService.value.id;
  logsMap.value[id] = [];
  invoke('clear_service_logs', { id }).catch(console.error);
}

function copyLogs() {
  const text = filteredLogs.value
    .map((l) => `[${l.timestamp}] [${l.level}] ${l.content}`)
    .join('\n');
  navigator.clipboard.writeText(text).then(() => {
    copyTip.value = '已复制!';
    setTimeout(() => {
      copyTip.value = '';
    }, 2000);
  });
}

function onScroll() {
  if (!terminalRef.value) return;
  const el = terminalRef.value;
  const isAtBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 30;
  if (isAtBottom) {
    autoScroll.value = true;
    pendingLogCount.value = 0;
  } else {
    autoScroll.value = false; // 用户上翻历史：自动暂停滚底
  }
}

function scrollToBottom() {
  nextTick(() => {
    if (terminalRef.value) {
      terminalRef.value.scrollTop = terminalRef.value.scrollHeight;
    }
  });
}

function getLogLevelClass(level: string) {
  return 'level-' + level.toLowerCase();
}

function formatLevel(level: string) {
  if (level === 'DEFAULT') return 'LOG';
  return level;
}
</script>

<style>
/* ============ 浅色主题设计令牌（全局共享，ServiceModal 同样使用） ============ */
:root {
  /* 三层明度阶梯：画布最亮 / 侧栏次之 / 顶栏毛玻璃白 */
  --bg-base: #eef1f7;
  --bg-panel: #f7f9fc;
  --bg-card: #ffffff;
  --bg-elevated: #ffffff;
  --bg-input: #f1f4f9;
  --bg-console: #ffffff;
  --bg-hover: rgba(23, 32, 51, 0.045);

  /* 淡蓝主色 */
  --accent: #4f7cf9;
  --accent-deep: #3a63e8;
  --accent-text: #3a56d4;
  --accent-grad: linear-gradient(135deg, #4f7cf9 0%, #6a67f7 100%);

  /* 状态色（浅底深化前景） */
  --ok: #16a34a;
  --warn: #b45309;
  --danger: #dc2626;
  --info: #2563eb;

  /* 文本层次（WCAG AA） */
  --text-1: #1b2434;
  --text-2: #52607a;
  --text-3: #7c8798;

  /* 边框层次 */
  --line-1: rgba(23, 32, 51, 0.08);
  --line-2: rgba(23, 32, 51, 0.14);

  /* 圆角 */
  --r-sm: 6px;
  --r-md: 9px;
  --r-lg: 14px;

  --shadow-pop: 0 16px 44px rgba(23, 32, 51, 0.18);
  --font-mono: 'Cascadia Code', 'Consolas', 'Courier New', monospace;
}

* {
  box-sizing: border-box;
  margin: 0;
  padding: 0;
}

html,
body {
  width: 100%;
  height: 100%;
  overflow: hidden;
  font-family: 'Segoe UI', 'PingFang SC', 'Microsoft YaHei', system-ui, sans-serif;
  font-size: 13px;
  background: var(--bg-base);
  color: var(--text-1);
  user-select: none;
  -webkit-font-smoothing: antialiased;
  font-variant-numeric: tabular-nums; /* 数字等宽，采样值不抖动 */
}

#app {
  width: 100vw;
  height: 100vh;
}

/* 全局细滚动条 */
::-webkit-scrollbar {
  width: 8px;
  height: 8px;
}
::-webkit-scrollbar-thumb {
  background: rgba(23, 32, 51, 0.18);
  border-radius: 4px;
}
::-webkit-scrollbar-thumb:hover {
  background: rgba(23, 32, 51, 0.32);
}
::-webkit-scrollbar-track {
  background: transparent;
}
::-webkit-scrollbar-corner {
  background: transparent;
}

::selection {
  background: rgba(79, 124, 249, 0.22);
}
</style>

<style scoped>
/* ============ 布局骨架 ============ */
.app-container {
  display: flex;
  flex-direction: column;
  width: 100vw;
  height: 100vh;
  background:
    radial-gradient(1100px 500px at 88% -12%, rgba(79, 124, 249, 0.07), transparent 60%),
    radial-gradient(900px 420px at -8% 112%, rgba(106, 103, 247, 0.06), transparent 60%),
    var(--bg-base);
}

.app-workspace {
  flex: 1;
  display: flex;
  overflow: hidden;
}

/* ============ 顶部应用栏（毛玻璃白） ============ */
.app-header {
  height: 48px;
  background: rgba(255, 255, 255, 0.82);
  backdrop-filter: blur(16px);
  border-bottom: 1px solid var(--line-2);
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 18px;
  flex-shrink: 0;
  z-index: 10;
}

.header-left {
  display: flex;
  align-items: center;
  gap: 10px;
}

.app-logo {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 30px;
  height: 30px;
  border-radius: var(--r-md);
  background: var(--accent-grad);
  color: #fff;
  box-shadow: 0 2px 10px rgba(79, 124, 249, 0.4);
}

.app-title {
  font-size: 14px;
  font-weight: 700;
  letter-spacing: 0.2px;
  background: linear-gradient(90deg, #232f4b 20%, #4f7cf9);
  -webkit-background-clip: text;
  background-clip: text;
  -webkit-text-fill-color: transparent;
}

.app-badge {
  font-size: 10.5px;
  color: var(--accent-text);
  background: rgba(79, 124, 249, 0.1);
  padding: 2px 9px;
  border-radius: 20px;
  letter-spacing: 0.3px;
  font-weight: 500;
}

.header-right {
  display: flex;
  align-items: center;
  gap: 10px;
}

/* 状态统计胶囊（可点击联动过滤） */
.stat-chips {
  display: inline-flex;
  background: var(--bg-input);
  border: 1px solid var(--line-1);
  border-radius: 8px;
  padding: 2px;
  gap: 2px;
}

.stat-chip {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  padding: 3px 10px;
  font-size: 11.5px;
  color: var(--text-2);
  background: transparent;
  border: none;
  border-radius: 6px;
  cursor: pointer;
  font-family: inherit;
  transition: all 0.15s ease;
  white-space: nowrap;
}
.stat-chip b {
  font-weight: 700;
  color: var(--text-1);
}
.stat-chip:hover {
  color: var(--text-1);
}
.stat-chip.active {
  background: #fff;
  color: var(--accent-text);
  box-shadow: 0 1px 4px rgba(23, 32, 51, 0.1);
}
.stat-chip.active b {
  color: var(--accent-text);
}
.stat-chip.active.ok b {
  color: var(--ok);
}
.stat-chip.active.err b {
  color: var(--danger);
}

.chip-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
}
.chip-dot.ok {
  background: var(--ok);
  box-shadow: 0 0 5px rgba(22, 163, 74, 0.5);
}
.chip-dot.off {
  background: #9aa6b8;
}
.chip-dot.err {
  background: var(--danger);
  box-shadow: 0 0 5px rgba(220, 38, 38, 0.5);
}

/* ============ 左侧服务列表 ============ */
.services-sidebar {
  position: relative;
  background: var(--bg-panel);
  border-right: 1px solid var(--line-2);
  display: flex;
  flex-direction: column;
  flex-shrink: 0;
  min-width: 220px;
  max-width: 400px;
}

/* 宽度拖拽手柄 */
.sidebar-resizer {
  position: absolute;
  top: 0;
  right: -3px;
  width: 6px;
  height: 100%;
  cursor: col-resize;
  z-index: 20;
}
.sidebar-resizer:hover {
  background: rgba(79, 124, 249, 0.25);
}

.sidebar-search {
  padding: 12px 14px 8px;
  position: relative;
}

.search-icon {
  position: absolute;
  left: 26px;
  top: 50%;
  transform: translateY(-50%);
  color: var(--text-3);
  display: inline-flex;
  pointer-events: none;
}

.search-input {
  width: 100%;
  background: #fff;
  border: 1px solid var(--line-2);
  color: var(--text-1);
  font-size: 12px;
  padding: 8px 10px 8px 32px;
  border-radius: var(--r-md);
  outline: none;
  transition: all 0.18s ease;
  font-family: inherit;
}
.search-input:focus {
  border-color: var(--accent);
  box-shadow: 0 0 0 3px rgba(79, 124, 249, 0.14);
}

/* 快捷筛选行 */
.sidebar-quickbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 14px 8px;
  gap: 6px;
}

.quick-chips {
  display: inline-flex;
  gap: 4px;
}

.quick-chip {
  padding: 3px 10px;
  font-size: 11px;
  border-radius: 20px;
  border: 1px solid transparent;
  background: var(--bg-input);
  color: var(--text-2);
  cursor: pointer;
  font-family: inherit;
  transition: all 0.15s ease;
}
.quick-chip:hover {
  color: var(--text-1);
  border-color: var(--line-2);
}
.quick-chip.active {
  background: rgba(79, 124, 249, 0.12);
  color: var(--accent-text);
  font-weight: 600;
  border-color: rgba(79, 124, 249, 0.3);
}

.quickbar-icon-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  border: none;
  background: none;
  border-radius: var(--r-sm);
  color: var(--text-3);
  cursor: pointer;
  transition: all 0.15s ease;
}
.quickbar-icon-btn:hover {
  background: var(--bg-hover);
  color: var(--text-1);
}

/* 聚合日志常驻入口 */
.agg-entry {
  display: flex;
  align-items: center;
  gap: 7px;
  margin: 0 14px 8px;
  padding: 7px 11px;
  font-size: 12px;
  font-weight: 500;
  font-family: inherit;
  color: var(--text-2);
  background: #fff;
  border: 1px solid var(--line-2);
  border-radius: var(--r-md);
  cursor: pointer;
  transition: all 0.16s ease;
}
.agg-entry:hover {
  border-color: var(--accent);
  color: var(--accent-text);
}
.agg-entry.active {
  background: rgba(79, 124, 249, 0.1);
  border-color: rgba(79, 124, 249, 0.45);
  color: var(--accent-text);
}

.agg-entry-count {
  margin-left: auto;
  font-size: 10px;
  font-family: var(--font-mono);
  background: #fff;
  border: 1px solid var(--line-2);
  color: var(--text-2);
  padding: 0 7px;
  border-radius: 10px;
}

.services-list {
  flex: 1;
  overflow-y: auto;
  padding: 2px 10px 12px;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

/* 服务分组 */
.service-group {
  display: flex;
  flex-direction: column;
  gap: 3px;
}

.group-header {
  display: flex;
  align-items: center;
  gap: 5px;
  padding: 7px 8px 4px;
  user-select: none;
}

.group-chevron {
  display: inline-flex;
  color: var(--text-3);
  cursor: pointer;
  border-radius: 4px;
  padding: 1px;
}
.group-chevron:hover {
  color: var(--text-1);
  background: var(--bg-hover);
}

.group-name {
  font-size: 10.5px;
  font-weight: 700;
  color: var(--text-2);
  text-transform: uppercase;
  letter-spacing: 0.8px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  cursor: pointer;
  max-width: 140px;
}

.group-count {
  font-size: 9.5px;
  color: var(--text-2);
  background: var(--bg-input);
  padding: 0 6px;
  border-radius: 8px;
  font-family: var(--font-mono);
  cursor: pointer;
}

.group-actions {
  display: inline-flex;
  gap: 1px;
  margin-left: auto;
  opacity: 0;
  transition: opacity 0.15s;
}

.service-group:hover .group-actions {
  opacity: 1;
}

.group-action-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 21px;
  height: 21px;
  border: none;
  background: none;
  border-radius: 5px;
  color: var(--ok);
  cursor: pointer;
  transition: all 0.15s ease;
}
.group-action-btn:hover {
  background: rgba(22, 163, 74, 0.12);
}
.group-action-btn.stop {
  color: var(--danger);
}
.group-action-btn.stop:hover {
  background: rgba(220, 38, 38, 0.1);
}

/* 服务卡片 */
.service-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 9px 11px;
  cursor: pointer;
  border-radius: var(--r-md);
  border: 1px solid transparent;
  position: relative;
  background: transparent;
  transition: all 0.15s ease;
}
.service-item:hover {
  background: #fff;
  border-color: var(--line-1);
  box-shadow: 0 1px 5px rgba(23, 32, 51, 0.06);
}
.service-item.active {
  background: rgba(79, 124, 249, 0.08);
  border-color: rgba(79, 124, 249, 0.35);
}
.service-item.active::before {
  content: '';
  position: absolute;
  left: -1px;
  top: 20%;
  bottom: 20%;
  width: 3px;
  border-radius: 2px;
  background: var(--accent-grad);
}

/* 状态点 */
.status-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
  background: #a8b2c4;
}
.status-dot.starting,
.status-dot.stopping {
  background: #f59e0b;
  box-shadow: 0 0 6px rgba(245, 158, 11, 0.45);
  animation: pulse-dot 2.4s ease-in-out infinite;
}
.status-dot.stopped {
  background: #b3bccb;
}
.status-dot.failed {
  background: var(--danger);
  box-shadow: 0 0 7px rgba(220, 38, 38, 0.4);
}
.status-dot.running {
  background: var(--ok);
  box-shadow: 0 0 6px rgba(22, 163, 74, 0.45);
  animation: pulse-dot 2s ease-in-out infinite;
}

@keyframes pulse-dot {
  0%,
  100% {
    opacity: 1;
  }
  50% {
    opacity: 0.4;
  }
}

.service-meta {
  flex: 1;
  overflow: hidden;
}

.service-name-row {
  display: flex;
  align-items: center;
  gap: 6px;
}

.service-name {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-1);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.service-port {
  font-size: 10.5px;
  color: #46618a;
  background: rgba(79, 124, 249, 0.08);
  border: 1px solid rgba(79, 124, 249, 0.22);
  padding: 0 5px;
  border-radius: 5px;
  font-family: var(--font-mono);
}

.service-sub-row {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-top: 3px;
}

.type-tag {
  font-size: 10px;
  color: var(--text-2);
  background: var(--bg-input);
  border: 1px solid var(--line-1);
  padding: 1px 6px;
  border-radius: 5px;
  white-space: nowrap;
}

.profile-preview {
  font-size: 10.5px;
  color: var(--accent-text);
  font-family: var(--font-mono);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

/* 悬浮快捷操作 */
.service-actions {
  display: flex;
  align-items: center;
  gap: 2px;
  opacity: 0;
  transition: opacity 0.15s;
}
.service-item:hover .service-actions,
.service-item.active .service-actions {
  opacity: 1;
}

.action-icon-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 26px;
  background: none;
  border: 1px solid transparent;
  border-radius: var(--r-sm);
  cursor: pointer;
  transition: all 0.15s ease;
}

.start-icon {
  color: var(--ok);
}
.start-icon:hover {
  background: rgba(22, 163, 74, 0.12);
  border-color: rgba(22, 163, 74, 0.3);
}

.stop-icon {
  color: var(--danger);
}
.stop-icon:hover {
  background: rgba(220, 38, 38, 0.1);
  border-color: rgba(220, 38, 38, 0.28);
}

.empty-list-tip {
  padding: 22px 14px;
  font-size: 12px;
  color: var(--text-2);
  text-align: center;
  line-height: 1.7;
}

.link-btn {
  border: none;
  background: none;
  color: var(--accent-text);
  font-size: 12px;
  font-family: inherit;
  cursor: pointer;
  padding: 0;
  text-decoration: underline;
}
.link-btn:hover {
  color: var(--accent-deep);
}

/* ============ 右侧主面板 ============ */
.main-panel {
  flex: 1;
  background: transparent;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.service-detail {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

/* 控制头部（白卡） */
.control-header {
  background: var(--bg-card);
  border-bottom: 1px solid var(--line-2);
  padding: 14px 20px 12px;
  display: flex;
  flex-direction: column;
  gap: 10px;
  flex-shrink: 0;
}

.control-header-top {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  gap: 12px;
}

.title-section {
  display: flex;
  flex-direction: column;
  gap: 5px;
  min-width: 0;
}

.service-main-title {
  font-size: 17px;
  font-weight: 700;
  color: var(--text-1);
  display: flex;
  align-items: center;
  gap: 10px;
  letter-spacing: 0.2px;
  flex-wrap: wrap;
}

.type-pill {
  font-size: 10.5px;
  font-weight: 600;
  background: rgba(79, 124, 249, 0.09);
  color: var(--accent-text);
  border: 1px solid rgba(79, 124, 249, 0.25);
  padding: 2px 9px;
  border-radius: 20px;
  white-space: nowrap;
}

/* 运行状态徽章 */
.state-badge {
  font-size: 11px;
  font-weight: 600;
  padding: 2px 10px;
  border-radius: 20px;
  white-space: nowrap;
  background: var(--bg-input);
  color: var(--text-2);
}
.state-badge.running {
  background: rgba(22, 163, 74, 0.1);
  color: var(--ok);
}
.state-badge.starting,
.state-badge.stopping {
  background: rgba(245, 158, 11, 0.12);
  color: #b45309;
}
.state-badge.failed {
  background: rgba(220, 38, 38, 0.09);
  color: var(--danger);
}

.service-work-dir {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  color: var(--text-2);
  max-width: 640px;
}
.service-work-dir .app-icon {
  flex-shrink: 0;
  color: var(--text-3);
}
.dir-text {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  font-family: var(--font-mono);
  font-size: 11.5px;
}

.service-mgmt-btns {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-shrink: 0;
}

.mgmt-divider {
  width: 1px;
  height: 20px;
  background: var(--line-2);
  margin: 0 3px;
}

.icon-only-btn {
  padding: 6px 9px !important;
  height: 30px;
  box-sizing: border-box;
}

.action-btn {
  height: 30px;
  box-sizing: border-box;
}

.control-header-bottom {
  display: flex;
  align-items: center;
  gap: 14px;
  flex-wrap: wrap;
  padding-top: 2px;
}

/* Profile 区 */
.profile-box {
  display: flex;
  align-items: center;
  gap: 7px;
}

.box-label {
  font-size: 11.5px;
  color: var(--text-2);
}

.active-profile-badge {
  font-size: 11.5px;
  font-family: var(--font-mono);
  padding: 3px 10px;
  border-radius: 20px;
  background: var(--bg-input);
  color: var(--text-2);
  font-weight: 600;
  border: 1px solid var(--line-1);
}
.active-profile-badge.running {
  background: rgba(22, 163, 74, 0.09);
  color: var(--ok);
  border-color: rgba(22, 163, 74, 0.3);
}

.profile-selector-wrap {
  display: flex;
  gap: 5px;
}

.profile-select {
  background: #fff;
  border: 1px solid var(--line-2);
  color: var(--text-1);
  font-size: 12px;
  padding: 4px 9px;
  border-radius: var(--r-sm);
  outline: none;
  font-family: inherit;
  transition: all 0.18s ease;
}
.profile-select:focus {
  border-color: var(--accent);
  box-shadow: 0 0 0 3px rgba(79, 124, 249, 0.14);
}

.profile-custom-input {
  background: #fff;
  border: 1px solid var(--accent);
  color: var(--text-1);
  font-size: 12px;
  padding: 4px 9px;
  border-radius: var(--r-sm);
  width: 110px;
  outline: none;
  font-family: var(--font-mono);
}
.profile-custom-input:focus {
  box-shadow: 0 0 0 3px rgba(79, 124, 249, 0.14);
}

/* 待重启生效徽章 */
.pending-restart-badge {
  font-size: 10.5px;
  font-weight: 600;
  color: #b45309;
  background: rgba(245, 158, 11, 0.12);
  border: 1px solid rgba(245, 158, 11, 0.35);
  padding: 2px 9px;
  border-radius: 20px;
  white-space: nowrap;
  cursor: help;
}

/* 操作按钮组 */
.action-btn-group {
  display: flex;
  gap: 7px;
}

.action-btn {
  padding: 6px 14px;
  font-size: 12px;
}

/* 运行态遥测 */
.telemetry-box {
  display: flex;
  align-items: center;
  gap: 6px;
  padding-left: 13px;
  border-left: 1px solid var(--line-1);
}

.telemetry-chip {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  font-size: 11px;
  font-family: var(--font-mono);
  background: var(--bg-input);
  border: 1px solid var(--line-1);
  color: var(--text-2);
  padding: 3px 9px;
  border-radius: 20px;
}
.telemetry-chip.clickable {
  cursor: pointer;
  transition: all 0.15s ease;
}
.telemetry-chip.clickable:hover {
  color: var(--accent-text);
  border-color: rgba(79, 124, 249, 0.4);
  background: rgba(79, 124, 249, 0.07);
}

.pid-tag {
  font-size: 10.5px;
  font-family: var(--font-mono);
  background: rgba(37, 99, 235, 0.07);
  border: 1px solid rgba(37, 99, 235, 0.2);
  color: var(--info);
  padding: 2px 8px;
  border-radius: 20px;
}

/* Maven 工具箱 */
.maven-toolbox {
  display: flex;
  align-items: center;
  gap: 8px;
  padding-left: 13px;
  border-left: 1px solid var(--line-1);
  margin-left: auto;
}

/* Maven 分段按钮组（紧凑收纳） */
.mvn-segment {
  display: inline-flex;
  background: #fff;
  border: 1px solid var(--line-2);
  border-radius: var(--r-md);
  overflow: hidden;
}

.mvn-seg-btn {
  padding: 5px 12px;
  font-size: 11px;
  font-family: var(--font-mono);
  border: none;
  background: transparent;
  color: var(--text-2);
  cursor: pointer;
  transition: all 0.15s ease;
  border-right: 1px solid var(--line-1);
  white-space: nowrap;
}
.mvn-seg-btn:last-child {
  border-right: none;
}
.mvn-seg-btn:hover:not(:disabled) {
  background: rgba(79, 124, 249, 0.08);
  color: var(--accent-text);
}
.mvn-seg-btn:disabled {
  opacity: 0.55;
  cursor: not-allowed;
  color: var(--text-2);
  background: var(--bg-input);
}

/* 构建中 spinner */
.spinner {
  width: 11px;
  height: 11px;
  border: 2px solid rgba(245, 158, 11, 0.25);
  border-top-color: #f59e0b;
  border-radius: 50%;
  animation: spin 0.7s linear infinite;
  flex-shrink: 0;
}
@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

/* 任务徽章 */
.task-badge {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: 11px;
  padding: 3px 10px;
  border-radius: 20px;
  white-space: nowrap;
  font-weight: 600;
}

.task-badge.running {
  background: rgba(245, 158, 11, 0.1);
  color: #b45309;
  border: 1px solid rgba(245, 158, 11, 0.35);
}

.task-badge.ok {
  background: rgba(22, 163, 74, 0.09);
  color: var(--ok);
  border: 1px solid rgba(22, 163, 74, 0.3);
}

.task-badge.err {
  background: rgba(220, 38, 38, 0.08);
  color: var(--danger);
  border: 1px solid rgba(220, 38, 38, 0.28);
}

/* 实时指标区 */
.metrics-box {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  padding: 3px 10px;
  border-radius: 20px;
  background: var(--bg-input);
  border: 1px solid var(--line-1);
}

.metrics-box .spark {
  display: block;
  opacity: 0.95;
}

.metrics-text {
  font-size: 10.5px;
  color: var(--text-2);
  font-family: var(--font-mono);
  white-space: nowrap;
}

/* ============ 日志控制台 ============ */
.console-section {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background: var(--bg-console);
  position: relative;
}

.console-toolbar {
  height: 38px;
  background: rgba(247, 249, 252, 0.85);
  backdrop-filter: blur(10px);
  border-bottom: 1px solid var(--line-1);
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 16px;
  flex-shrink: 0;
}

.toolbar-left {
  display: flex;
  align-items: center;
  gap: 10px;
}

.console-title {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  font-size: 12px;
  font-weight: 600;
  color: var(--text-1);
}

.console-title-sub {
  font-size: 11px;
  color: var(--text-2);
}

.log-count {
  font-size: 10.5px;
  color: var(--text-2);
  font-family: var(--font-mono);
  white-space: nowrap;
}

/* 单服务/聚合切换 */
.view-switch {
  display: inline-flex;
  background: #fff;
  border: 1px solid var(--line-2);
  border-radius: 7px;
  padding: 1px;
  gap: 1px;
}

.view-switch-btn {
  padding: 2px 10px;
  font-size: 11px;
  border: none;
  border-radius: 5px;
  background: transparent;
  color: var(--text-3);
  cursor: pointer;
  font-family: inherit;
  transition: all 0.15s ease;
  white-space: nowrap;
}
.view-switch-btn:hover {
  color: var(--text-2);
}
.view-switch-btn.active {
  background: rgba(79, 124, 249, 0.13);
  color: var(--accent-text);
  font-weight: 600;
}

/* 日志级别过滤胶囊 */
.level-switch {
  display: inline-flex;
  gap: 4px;
}

.level-chip {
  padding: 2px 10px;
  font-size: 10px;
  font-weight: 700;
  font-family: var(--font-mono);
  letter-spacing: 0.5px;
  border-radius: 12px;
  border: 1px solid var(--line-2);
  background: #fff;
  color: var(--text-3);
  cursor: pointer;
  transition: all 0.15s ease;
}
.level-chip:hover {
  color: var(--text-1);
  border-color: var(--accent);
}
.level-chip.active {
  background: #fff;
  box-shadow: 0 1px 5px rgba(23, 32, 51, 0.13);
}
.level-chip.all.active {
  border-color: var(--accent);
  color: var(--accent-text);
}
.level-chip.info.active {
  border-color: rgba(37, 99, 235, 0.55);
  color: var(--info);
}
.level-chip.warn.active {
  border-color: rgba(217, 119, 6, 0.6);
  color: #92400e;
}
.level-chip.error.active {
  border-color: rgba(220, 38, 38, 0.55);
  color: var(--danger);
}

.toolbar-right {
  display: flex;
  align-items: center;
  gap: 10px;
}

.log-filter-wrap {
  position: relative;
  display: inline-flex;
  align-items: center;
}

.log-filter-icon {
  position: absolute;
  left: 8px;
  color: var(--text-3);
  display: inline-flex;
  pointer-events: none;
}

.log-filter-input {
  background: #fff;
  border: 1px solid var(--line-2);
  color: var(--text-1);
  font-size: 11.5px;
  padding: 4px 9px 4px 28px;
  border-radius: var(--r-sm);
  width: 170px;
  outline: none;
  transition: all 0.18s ease;
  font-family: inherit;
}
.log-filter-input:focus {
  border-color: var(--accent);
  box-shadow: 0 0 0 3px rgba(79, 124, 249, 0.13);
  width: 210px;
}

.auto-scroll-label {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: 11.5px;
  color: var(--text-2);
  cursor: pointer;
}
.auto-scroll-label input {
  display: none;
}

/* 自绘复选框 */
.checkbox-fake {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 15px;
  height: 15px;
  border-radius: 4px;
  border: 1px solid var(--line-2);
  background: #fff;
  color: transparent;
  transition: all 0.15s ease;
}
.auto-scroll-label:has(input:checked) .checkbox-fake {
  background: var(--accent-grad);
  border-color: transparent;
  color: #fff;
}

/* 终端日志内容 */
.terminal-container {
  flex: 1;
  overflow-y: auto;
  padding: 12px 16px;
  font-family: var(--font-mono);
  font-size: 12px;
  line-height: 1.65;
  user-select: text;
}

.log-row {
  display: flex;
  align-items: flex-start;
  gap: 10px;
  word-break: break-all;
  padding: 1px 0;
}

.log-time {
  color: #94a0b4;
  flex-shrink: 0;
  font-size: 11px;
  padding-top: 1px;
}

.log-level {
  flex-shrink: 0;
  font-size: 9.5px;
  font-weight: 700;
  padding: 1px 7px;
  border-radius: 4px;
  letter-spacing: 0.5px;
  margin-top: 1px;
  font-family: inherit;
}
.log-level.INFO {
  color: var(--info);
  background: rgba(37, 99, 235, 0.09);
}
.log-level.WARN {
  color: #92400e;
  background: rgba(217, 119, 6, 0.15);
}
.log-level.ERROR {
  color: var(--danger);
  background: rgba(220, 38, 38, 0.1);
}
.log-level.DEBUG {
  color: #0d8a52;
  background: rgba(22, 163, 74, 0.09);
}
.log-level.DEFAULT {
  color: var(--text-2);
  background: var(--bg-input);
}

.log-content {
  flex: 1;
  white-space: pre-wrap;
  color: #3a4457;
}

.level-error .log-content {
  color: var(--danger);
}
.level-error {
  background: rgba(220, 38, 38, 0.035);
  border-radius: 5px;
}
.level-warn .log-content {
  color: #92400e;
}

/* 聚合日志服务标签 */
.svc-tag {
  flex-shrink: 0;
  font-size: 9.5px;
  font-family: var(--font-mono);
  padding: 1px 6px;
  border-radius: 4px;
  border: 1px solid;
  background: rgba(255, 255, 255, 0.6);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 120px;
  margin-top: 1px;
}

.empty-log-tip {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
  color: var(--text-3);
  font-size: 12.5px;
  padding: 40px 0;
}

/* 回到底部悬浮胶囊 */
.jump-bottom-pill {
  position: sticky;
  bottom: 14px;
  left: calc(100% - 200px);
  display: inline-flex;
  align-items: center;
  gap: 6px;
  margin: 0 auto;
  padding: 6px 14px;
  font-size: 11.5px;
  font-weight: 600;
  font-family: inherit;
  color: var(--accent-text);
  background: #fff;
  border: 1px solid rgba(79, 124, 249, 0.4);
  border-radius: 20px;
  box-shadow: 0 4px 16px rgba(23, 32, 51, 0.16);
  cursor: pointer;
  transition: all 0.16s ease;
  pointer-events: auto;
}
.jump-bottom-pill:hover {
  background: rgba(79, 124, 249, 0.07);
}

/* ============ 冷启动空状态 ============ */
.empty-state {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 26px;
  padding: 40px;
  overflow-y: auto;
}

.empty-hero {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 12px;
  text-align: center;
}

.empty-hero-icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 56px;
  height: 56px;
  border-radius: 16px;
  background: var(--accent-grad);
  color: #fff;
  box-shadow: 0 8px 28px rgba(79, 124, 249, 0.35);
}

.empty-title {
  font-size: 19px;
  font-weight: 700;
  color: var(--text-1);
  letter-spacing: 0.3px;
}

.empty-subtitle {
  font-size: 13px;
  color: var(--text-2);
  max-width: 420px;
  line-height: 1.6;
}

.empty-actions {
  display: flex;
  gap: 10px;
}

.empty-primary-btn {
  padding: 9px 20px;
  font-size: 13px;
}

/* 拖拽区 */
.empty-dropzone {
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 18px 26px;
  border: 1.5px dashed var(--line-2);
  border-radius: var(--r-lg);
  background: rgba(255, 255, 255, 0.55);
  color: var(--text-3);
  transition: all 0.18s ease;
  max-width: 520px;
}
.empty-dropzone.over {
  border-color: var(--accent);
  background: rgba(79, 124, 249, 0.06);
  color: var(--accent-text);
  box-shadow: 0 0 0 4px rgba(79, 124, 249, 0.1);
}

.dropzone-text {
  display: flex;
  flex-direction: column;
  gap: 3px;
  font-size: 12.5px;
  color: var(--text-1);
}

.dropzone-sub {
  font-size: 11px;
  color: var(--text-3);
}

/* 核心能力微卡片 */
.empty-features {
  display: flex;
  gap: 12px;
  flex-wrap: wrap;
  justify-content: center;
}

.feature-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
  width: 170px;
  padding: 16px 14px;
  background: #fff;
  border: 1px solid var(--line-1);
  border-radius: var(--r-lg);
  color: var(--accent-text);
  text-align: center;
  transition: all 0.18s ease;
}
.feature-card:hover {
  border-color: rgba(79, 124, 249, 0.35);
  box-shadow: 0 6px 20px rgba(23, 32, 51, 0.08);
  transform: translateY(-2px);
}

.feature-title {
  font-size: 12px;
  font-weight: 600;
  color: var(--text-1);
}

.feature-desc {
  font-size: 11px;
  color: var(--text-2);
  line-height: 1.5;
}

/* ============ 按钮系统 ============ */
.btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 5px;
  padding: 6px 14px;
  border-radius: var(--r-md);
  font-size: 12px;
  font-weight: 500;
  border: 1px solid transparent;
  cursor: pointer;
  color: var(--text-1);
  background: transparent;
  font-family: inherit;
  white-space: nowrap;
  transition: all 0.18s cubic-bezier(0.4, 0, 0.2, 1);
}

.btn:active:not(:disabled) {
  transform: scale(0.97);
}

.btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.btn.primary {
  background: var(--accent-grad);
  color: #ffffff;
  box-shadow: 0 2px 10px rgba(79, 124, 249, 0.32);
}
.btn.primary:hover:not(:disabled) {
  filter: brightness(1.06);
  box-shadow: 0 4px 16px rgba(79, 124, 249, 0.42);
}

.btn.danger {
  background: rgba(220, 38, 38, 0.08);
  color: var(--danger);
  border-color: rgba(220, 38, 38, 0.3);
}
.btn.danger:hover:not(:disabled) {
  background: rgba(220, 38, 38, 0.14);
  border-color: rgba(220, 38, 38, 0.45);
}

.btn.secondary {
  background: #fff;
  color: var(--text-1);
  border-color: var(--line-2);
  box-shadow: 0 1px 3px rgba(23, 32, 51, 0.05);
}
.btn.secondary:hover:not(:disabled) {
  border-color: var(--accent);
  color: var(--accent-text);
}

.btn.ghost {
  background: transparent;
  color: var(--text-2);
}
.btn.ghost:hover:not(:disabled) {
  color: var(--text-1);
  background: var(--bg-hover);
  border-color: var(--line-1);
}

.btn.danger-ghost:hover:not(:disabled) {
  color: var(--danger);
  background: rgba(220, 38, 38, 0.08);
  border-color: rgba(220, 38, 38, 0.25);
}

.btn.sm {
  padding: 4px 10px;
  font-size: 11px;
}

.add-btn {
  font-weight: 600;
}
</style>
