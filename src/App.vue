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
        <span class="stat-text">
          共 {{ services.length }} 个服务，{{ runningCount }} 个运行中
        </span>
        <button class="btn primary add-btn" @click="openAddModal">
          <AppIcon name="plus" :size="14" />
          添加服务
        </button>
      </div>
    </header>

    <!-- 主工作区：左侧服务列表 + 右侧控制与日志 -->
    <div class="app-workspace">
      <!-- 左侧：服务列表 -->
      <aside class="services-sidebar">
        <div class="sidebar-search">
          <span class="search-icon"><AppIcon name="search" :size="13" /></span>
          <input
            v-model="serviceSearch"
            type="text"
            placeholder="过滤服务..."
            class="search-input"
          />
        </div>

        <div class="services-list">
          <div
            v-for="svc in filteredServices"
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

          <div v-if="services.length === 0" class="empty-list-tip">
            暂无服务配置，点击右上角 “+ 添加服务” 开始
          </div>
        </div>
      </aside>

      <!-- 右侧：服务主面板 -->
      <main class="main-panel">
        <div v-if="currentService" class="service-detail">
          <!-- 服务控制头部 -->
          <div class="control-header">
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
                </div>
                <div class="service-work-dir" :title="currentService.workDir">
                  <AppIcon name="folder" :size="12" />
                  <span class="dir-text">{{ currentService.workDir }}</span>
                </div>
              </div>

              <!-- 右侧编辑/删除 -->
              <div class="service-mgmt-btns">
                <button class="btn ghost sm" @click="openEditModal">
                  <AppIcon name="edit" :size="12" /> 编辑
                </button>
                <button class="btn ghost danger-ghost sm" @click="confirmDeleteService">
                  <AppIcon name="trash" :size="12" /> 删除
                </button>
              </div>
            </div>

            <!-- Profile 选择与操作栏 -->
            <div class="control-header-bottom">
              <!-- 当前实际运行 Profile -->
              <div class="profile-box">
                <span class="box-label">实际运行 Profile:</span>
                <span :class="['active-profile-badge', { running: isRunning(currentService.id) }]">
                  {{ getActiveProfileDisplay(currentService.id) }}
                </span>
              </div>

              <!-- 下次生效 Profile 选择/输入框 -->
              <div class="profile-box next-profile-box">
                <span class="box-label">下次启动 Profile:</span>
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
                    <option value="custom">自定义多配置...</option>
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
                <span class="profile-notice" title="运行中修改只在下一次启动或重启生效">
                  (下次生效)
                </span>
              </div>

              <!-- 操作按钮组 -->
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
                    <button
                      class="btn ghost action-btn mvn-btn"
                      :disabled="!canRunMavenTask"
                      title="mvn clean：清理工程 target 目录"
                      @click="runMavenTask('clean')"
                    >
                      Clean
                    </button>
                    <button
                      class="btn ghost action-btn mvn-btn"
                      :disabled="!canRunMavenTask"
                      title="mvn compile：增量编译工程"
                      @click="runMavenTask('compile')"
                    >
                      Compile
                    </button>
                    <button
                      class="btn ghost action-btn mvn-btn"
                      :disabled="!canRunMavenTask"
                      title="mvn package：打包全工程（不运行服务）"
                      @click="runMavenTask('package')"
                    >
                      Package
                    </button>
                    <button
                      class="btn ghost action-btn mvn-btn"
                      :disabled="!canRunMavenTask"
                      title="mvn install：构建并安装到本地仓库"
                      @click="runMavenTask('install')"
                    >
                      Install
                    </button>
                  </template>
                  <template v-else>
                    <span class="task-badge running">
                      <span class="spinner"></span>
                      mvn {{ getMavenTaskGoal(currentService.id) }} 构建中...
                    </span>
                    <button class="btn danger action-btn" @click="stopMavenTask">
                      <AppIcon name="stop" :size="11" /> 停止构建
                    </button>
                  </template>
                </template>
              </div>

              <!-- 状态与 PID -->
              <div class="status-summary">
                <span v-if="mavenTaskBadge" :class="['task-badge', mavenTaskBadge.cls]">
                  {{ mavenTaskBadge.text }}
                </span>
                <span class="state-label">{{ getStatusText(currentService.id) }}</span>
                <span v-if="getPid(currentService.id)" class="pid-tag">
                  PID: {{ getPid(currentService.id) }}
                </span>
              </div>
            </div>
          </div>

          <!-- 日志视窗区 -->
          <div class="console-section">
            <div class="console-toolbar">
              <div class="toolbar-left">
                <span class="console-title"><AppIcon name="terminal" :size="13" /> 实时控制台</span>
                <span class="log-count">{{ filteredLogs.length }} 行</span>
              </div>
              <div class="toolbar-right">
                <div class="log-filter-wrap">
                  <span class="log-filter-icon"><AppIcon name="filter" :size="12" /></span>
                  <input
                    v-model="logFilter"
                    type="text"
                    placeholder="过滤日志..."
                    class="log-filter-input"
                  />
                </div>
                <label class="auto-scroll-label">
                  <span class="checkbox-fake"><AppIcon name="check" :size="10" :stroke-width="3" /></span>
                  <input type="checkbox" v-model="autoScroll" />
                  自动滚屏
                </label>
                <button class="btn ghost sm" @click="clearLogs">
                  <AppIcon name="trash" :size="12" /> 清空
                </button>
                <button class="btn ghost sm" @click="copyLogs">
                  <AppIcon name="copy" :size="12" />
                  {{ copyTip || '复制全部' }}
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
                <span class="log-time">{{ log.timestamp }}</span>
                <span :class="['log-level', log.level]">{{ formatLevel(log.level) }}</span>
                <span class="log-content">{{ log.content }}</span>
              </div>
              <div v-if="filteredLogs.length === 0" class="empty-log-tip">
                {{ logFilter ? '未匹配到包含搜索词的日志' : '等待服务启动或输出日志...' }}
              </div>
            </div>
          </div>
        </div>

        <div v-else class="empty-state">
          <AppIcon name="layers" :size="34" :stroke-width="1.3" class="empty-icon" />
          <div class="empty-title">请在左侧选择一个服务，或添加新服务</div>
        </div>
      </main>
    </div>

    <!-- 添加 / 编辑模态框 -->
    <ServiceModal
      :is-open="isModalOpen"
      :service-data="editingService"
      @close="isModalOpen = false"
      @saved="onServiceSaved"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, nextTick } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import ServiceModal from './components/ServiceModal.vue';
import AppIcon from './components/AppIcon.vue';
import type {
  MavenProjectInfo,
  MavenTaskStatus,
  MavenTaskStatusEvent,
  ServiceConfig,
  ServiceStatus,
  LogEntry,
  ServiceLogEvent,
  StatusChangeEvent,
} from './types';

// 服务列表与当前选中
const services = ref<ServiceConfig[]>([]);
const currentService = ref<ServiceConfig | null>(null);
const serviceSearch = ref('');

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

const filteredServices = computed(() => {
  if (!serviceSearch.value.trim()) return services.value;
  const q = serviceSearch.value.toLowerCase();
  return services.value.filter(
    (s) => s.name.toLowerCase().includes(q) || s.workDir.toLowerCase().includes(q)
  );
});

const currentLogs = computed(() => {
  if (!currentService.value) return [];
  return logsMap.value[currentService.value.id] || [];
});

const filteredLogs = computed(() => {
  const all = currentLogs.value;
  if (!logFilter.value.trim()) return all;
  const q = logFilter.value.toLowerCase();
  return all.filter((l) => l.content.toLowerCase().includes(q));
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

    if (currentService.value?.id === serviceId && autoScroll.value) {
      scrollToBottom();
    }
  });

  // 监听后端状态变更推送
  await listen<StatusChangeEvent>('service-status-change', (event) => {
    const { serviceId, status } = event.payload;
    statusMap.value[serviceId] = status;
  });

  // 监听 Maven 工具箱任务状态变更
  await listen<MavenTaskStatusEvent>('maven-task-status-change', (event) => {
    const { serviceId, status } = event.payload;
    mavenTaskMap.value[serviceId] = status;
  });
});

async function loadServices() {
  try {
    const list = await invoke<ServiceConfig[]>('get_services');
    services.value = list;
    if (list.length > 0 && !currentService.value) {
      selectService(list[0]);
    }
  } catch (err) {
    console.error('Failed to load services:', err);
  }
}

async function loadAllStatuses() {
  try {
    const statuses = await invoke<Record<string, ServiceStatus>>('get_all_service_statuses');
    statusMap.value = statuses;
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

function openEditModal() {
  if (!currentService.value) return;
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
/* ============ 设计令牌（全局共享，ServiceModal 同样使用） ============ */
:root {
  /* 深空蓝黑底色系 */
  --bg-deep: #070b14;
  --bg-base: #0b1120;
  --bg-panel: #0e1626;
  --bg-card: #131d30;
  --bg-elevated: #1a2740;
  --bg-input: #0b1424;
  --bg-console: #080d18;

  /* 靛紫强调 */
  --accent: #6366f1;
  --accent-soft: #a5b4fc;
  --accent-grad: linear-gradient(135deg, #6366f1 0%, #8b5cf6 100%);

  /* 状态色 */
  --ok: #34d399;
  --warn: #fbbf24;
  --danger: #f87171;
  --info: #60a5fa;

  /* 文本层次 */
  --text-1: #e6edf6;
  --text-2: #98a6bd;
  --text-3: #5f6e85;

  /* 边框层次 */
  --line-1: rgba(148, 163, 184, 0.1);
  --line-2: rgba(148, 163, 184, 0.18);
  --line-accent: rgba(99, 102, 241, 0.45);

  /* 圆角 */
  --r-sm: 6px;
  --r-md: 9px;
  --r-lg: 14px;

  --shadow-pop: 0 16px 48px rgba(0, 0, 0, 0.55);
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
  background: var(--bg-deep);
  color: var(--text-1);
  user-select: none;
  -webkit-font-smoothing: antialiased;
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
  background: rgba(148, 163, 184, 0.22);
  border-radius: 4px;
}
::-webkit-scrollbar-thumb:hover {
  background: rgba(148, 163, 184, 0.4);
}
::-webkit-scrollbar-track {
  background: transparent;
}
::-webkit-scrollbar-corner {
  background: transparent;
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
    radial-gradient(1100px 500px at 85% -10%, rgba(99, 102, 241, 0.09), transparent 60%),
    radial-gradient(900px 420px at -10% 110%, rgba(139, 92, 246, 0.07), transparent 60%),
    var(--bg-base);
}

.app-workspace {
  flex: 1;
  display: flex;
  overflow: hidden;
}

/* ============ 顶部应用栏（毛玻璃） ============ */
.app-header {
  height: 48px;
  background: rgba(14, 22, 38, 0.72);
  backdrop-filter: blur(16px);
  border-bottom: 1px solid var(--line-1);
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 18px;
  flex-shrink: 0;
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
  box-shadow: 0 2px 14px rgba(99, 102, 241, 0.45);
}

.app-title {
  font-size: 14px;
  font-weight: 700;
  letter-spacing: 0.2px;
  background: linear-gradient(90deg, #e6edf6 20%, var(--accent-soft));
  -webkit-background-clip: text;
  background-clip: text;
  -webkit-text-fill-color: transparent;
}

.app-badge {
  font-size: 10.5px;
  color: var(--text-2);
  background: rgba(148, 163, 184, 0.1);
  border: 1px solid var(--line-1);
  padding: 2px 8px;
  border-radius: 20px;
  letter-spacing: 0.3px;
}

.header-right {
  display: flex;
  align-items: center;
  gap: 14px;
}

.stat-text {
  font-size: 12px;
  color: var(--text-3);
}

/* ============ 左侧服务列表 ============ */
.services-sidebar {
  width: 282px;
  background: var(--bg-panel);
  border-right: 1px solid var(--line-1);
  display: flex;
  flex-direction: column;
  flex-shrink: 0;
}

.sidebar-search {
  padding: 12px 14px 10px;
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
  background: var(--bg-input);
  border: 1px solid var(--line-1);
  color: var(--text-1);
  font-size: 12px;
  padding: 8px 10px 8px 32px;
  border-radius: var(--r-md);
  outline: none;
  transition: all 0.18s ease;
  font-family: inherit;
}
.search-input:focus {
  border-color: var(--line-accent);
  box-shadow: 0 0 0 3px rgba(99, 102, 241, 0.12);
}

.services-list {
  flex: 1;
  overflow-y: auto;
  padding: 4px 10px 12px;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

/* 服务卡片 */
.service-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 12px;
  cursor: pointer;
  border-radius: 10px;
  border: 1px solid transparent;
  position: relative;
  transition: all 0.16s ease;
}
.service-item:hover {
  background: rgba(148, 163, 184, 0.06);
  border-color: var(--line-1);
}
.service-item.active {
  background: linear-gradient(90deg, rgba(99, 102, 241, 0.16), rgba(139, 92, 246, 0.05));
  border-color: rgba(99, 102, 241, 0.32);
}
.service-item.active::before {
  content: '';
  position: absolute;
  left: 0;
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
  background: var(--text-3);
}
.status-dot.starting,
.status-dot.stopping {
  background: var(--warn);
  box-shadow: 0 0 6px rgba(251, 191, 36, 0.6);
  animation: pulse-dot 2.4s ease-in-out infinite;
}
.status-dot.stopped {
  background: #475569;
}
.status-dot.failed {
  background: var(--danger);
  box-shadow: 0 0 8px rgba(248, 113, 113, 0.55);
}
.status-dot.running {
  background: var(--ok);
  box-shadow: 0 0 6px rgba(52, 211, 153, 0.65);
  animation: pulse-dot 2s ease-in-out infinite;
}

@keyframes pulse-dot {
  0%,
  100% {
    opacity: 1;
  }
  50% {
    opacity: 0.45;
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
  color: #6ee7b7;
  background: rgba(52, 211, 153, 0.1);
  border: 1px solid rgba(52, 211, 153, 0.22);
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
  background: rgba(148, 163, 184, 0.1);
  padding: 1px 6px;
  border-radius: 5px;
  white-space: nowrap;
}

.profile-preview {
  font-size: 10.5px;
  color: var(--accent-soft);
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
  background: rgba(52, 211, 153, 0.14);
  border-color: rgba(52, 211, 153, 0.3);
}

.stop-icon {
  color: var(--danger);
}
.stop-icon:hover {
  background: rgba(248, 113, 113, 0.14);
  border-color: rgba(248, 113, 113, 0.3);
}

.empty-list-tip {
  padding: 28px 16px;
  font-size: 12px;
  color: var(--text-3);
  text-align: center;
  line-height: 1.7;
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

/* 控制头部 */
.control-header {
  background: var(--bg-panel);
  border-bottom: 1px solid var(--line-1);
  padding: 16px 20px 13px;
  display: flex;
  flex-direction: column;
  gap: 12px;
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
}

.type-pill {
  font-size: 10.5px;
  font-weight: 600;
  background: rgba(99, 102, 241, 0.14);
  color: var(--accent-soft);
  border: 1px solid rgba(99, 102, 241, 0.28);
  padding: 2px 9px;
  border-radius: 20px;
  white-space: nowrap;
}

.service-work-dir {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  color: var(--text-3);
  max-width: 640px;
}
.service-work-dir .app-icon {
  flex-shrink: 0;
}
.dir-text {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.service-mgmt-btns {
  display: flex;
  gap: 6px;
  flex-shrink: 0;
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
  color: var(--text-3);
}

.active-profile-badge {
  font-size: 11.5px;
  font-family: var(--font-mono);
  padding: 3px 10px;
  border-radius: 20px;
  background: rgba(148, 163, 184, 0.1);
  color: var(--text-2);
  font-weight: 600;
  border: 1px solid var(--line-1);
}
.active-profile-badge.running {
  background: rgba(52, 211, 153, 0.12);
  color: var(--ok);
  border-color: rgba(52, 211, 153, 0.35);
}

.profile-selector-wrap {
  display: flex;
  gap: 5px;
}

.profile-select {
  background: var(--bg-input);
  border: 1px solid var(--line-1);
  color: var(--text-1);
  font-size: 12px;
  padding: 4px 9px;
  border-radius: var(--r-sm);
  outline: none;
  font-family: inherit;
  transition: all 0.18s ease;
}
.profile-select:focus {
  border-color: var(--line-accent);
  box-shadow: 0 0 0 3px rgba(99, 102, 241, 0.12);
}

.profile-custom-input {
  background: var(--bg-input);
  border: 1px solid var(--line-accent);
  color: var(--text-1);
  font-size: 12px;
  padding: 4px 9px;
  border-radius: var(--r-sm);
  width: 110px;
  outline: none;
  font-family: var(--font-mono);
}
.profile-custom-input:focus {
  box-shadow: 0 0 0 3px rgba(99, 102, 241, 0.12);
}

.profile-notice {
  font-size: 10.5px;
  color: var(--text-3);
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

/* Maven 工具箱 */
.maven-toolbox {
  display: flex;
  align-items: center;
  gap: 6px;
  padding-left: 14px;
  border-left: 1px solid var(--line-1);
}

.mvn-btn {
  font-family: var(--font-mono);
  font-size: 11.5px;
  padding: 6px 11px;
}

/* 构建中 spinner */
.spinner {
  width: 11px;
  height: 11px;
  border: 2px solid rgba(251, 191, 36, 0.25);
  border-top-color: var(--warn);
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
  background: rgba(251, 191, 36, 0.1);
  color: var(--warn);
  border: 1px solid rgba(251, 191, 36, 0.3);
}

.task-badge.ok {
  background: rgba(52, 211, 153, 0.1);
  color: var(--ok);
  border: 1px solid rgba(52, 211, 153, 0.3);
}

.task-badge.err {
  background: rgba(248, 113, 113, 0.1);
  color: var(--danger);
  border: 1px solid rgba(248, 113, 113, 0.3);
}

/* 状态摘要 */
.status-summary {
  display: flex;
  align-items: center;
  gap: 9px;
  margin-left: auto;
}

.state-label {
  font-size: 12px;
  color: var(--text-2);
  white-space: nowrap;
}

.pid-tag {
  font-size: 10.5px;
  font-family: var(--font-mono);
  background: rgba(96, 165, 250, 0.1);
  border: 1px solid rgba(96, 165, 250, 0.25);
  color: var(--info);
  padding: 2px 8px;
  border-radius: 20px;
}

/* ============ 日志控制台 ============ */
.console-section {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background: var(--bg-console);
}

.console-toolbar {
  height: 38px;
  background: rgba(14, 22, 38, 0.6);
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

.log-count {
  font-size: 10.5px;
  color: var(--text-3);
  font-family: var(--font-mono);
}

.toolbar-right {
  display: flex;
  align-items: center;
  gap: 9px;
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
  background: var(--bg-input);
  border: 1px solid var(--line-1);
  color: var(--text-1);
  font-size: 11.5px;
  padding: 4px 9px 4px 28px;
  border-radius: var(--r-sm);
  width: 180px;
  outline: none;
  transition: all 0.18s ease;
  font-family: inherit;
}
.log-filter-input:focus {
  border-color: var(--line-accent);
  box-shadow: 0 0 0 3px rgba(99, 102, 241, 0.1);
  width: 220px;
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

/* 自绘复选框（:has 匹配选中态） */
.checkbox-fake {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 15px;
  height: 15px;
  border-radius: 4px;
  border: 1px solid var(--line-2);
  background: var(--bg-input);
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
  color: var(--text-3);
  flex-shrink: 0;
  font-size: 11px;
  opacity: 0.75;
  padding-top: 1px;
}

.log-level {
  flex-shrink: 0;
  font-size: 9.5px;
  font-weight: 700;
  padding: 1px 6px;
  border-radius: 4px;
  letter-spacing: 0.5px;
  margin-top: 1px;
  font-family: inherit;
}
.log-level.INFO {
  color: #93c5fd;
  background: rgba(96, 165, 250, 0.13);
}
.log-level.WARN {
  color: #fcd34d;
  background: rgba(251, 191, 36, 0.13);
}
.log-level.ERROR {
  color: #fca5a5;
  background: rgba(248, 113, 113, 0.15);
}
.log-level.DEBUG {
  color: #6ee7b7;
  background: rgba(52, 211, 153, 0.1);
}
.log-level.DEFAULT {
  color: var(--text-2);
  background: rgba(148, 163, 184, 0.09);
}
.log-level.SYSTEM {
  color: #c4b5fd;
  background: rgba(139, 92, 246, 0.13);
}

.log-content {
  flex: 1;
  white-space: pre-wrap;
  color: #a8b6cc;
}

.level-error .log-content {
  color: #f87171;
}
.level-warn .log-content {
  color: #fbbf24;
}

.empty-log-tip,
.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 14px;
  height: 100%;
  color: var(--text-3);
  font-size: 13px;
}

.empty-log-tip {
  padding: 40px 0;
}

.empty-state .empty-icon {
  color: var(--text-3);
  opacity: 0.55;
}

.empty-title {
  letter-spacing: 0.3px;
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
  opacity: 0.35;
  cursor: not-allowed;
}

.btn.primary {
  background: var(--accent-grad);
  color: #ffffff;
  box-shadow: 0 2px 12px rgba(99, 102, 241, 0.35);
}
.btn.primary:hover:not(:disabled) {
  filter: brightness(1.12);
  box-shadow: 0 4px 18px rgba(99, 102, 241, 0.5);
}

.btn.danger {
  background: rgba(248, 113, 113, 0.13);
  color: #fca5a5;
  border-color: rgba(248, 113, 113, 0.32);
}
.btn.danger:hover:not(:disabled) {
  background: rgba(248, 113, 113, 0.22);
  border-color: rgba(248, 113, 113, 0.45);
}

.btn.secondary {
  background: var(--bg-elevated);
  color: var(--text-1);
  border-color: var(--line-2);
}
.btn.secondary:hover:not(:disabled) {
  border-color: var(--line-accent);
  background: #1e2c48;
}

.btn.ghost {
  background: transparent;
  color: var(--text-2);
}
.btn.ghost:hover:not(:disabled) {
  color: var(--text-1);
  background: rgba(148, 163, 184, 0.09);
  border-color: var(--line-1);
}

.btn.danger-ghost:hover:not(:disabled) {
  color: #fca5a5;
  background: rgba(248, 113, 113, 0.1);
  border-color: rgba(248, 113, 113, 0.25);
}

.btn.sm {
  padding: 4px 10px;
  font-size: 11px;
}

.add-btn {
  font-weight: 600;
}
</style>
