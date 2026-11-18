<template>
  <div class="app-container">
    <!-- 顶部应用栏 -->
    <header class="app-header">
      <div class="header-left">
        <span class="app-logo">⚡</span>
        <span class="app-title">Local Service Runner</span>
        <span class="app-badge">Spring Boot 管理器</span>
      </div>
      <div class="header-right">
        <span class="stat-text">
          共 {{ services.length }} 个服务，{{ runningCount }} 个运行中
        </span>
        <button class="btn primary add-btn" @click="openAddModal">
          + 添加服务
        </button>
      </div>
    </header>

    <!-- 主工作区：左侧服务列表 + 右侧控制与日志 -->
    <div class="app-workspace">
      <!-- 左侧：服务列表 -->
      <aside class="services-sidebar">
        <div class="sidebar-search">
          <input
            v-model="serviceSearch"
            type="text"
            placeholder="🔍 过滤服务..."
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
                v-if="!isRunning(svc.id) && !isStarting(svc.id)"
                class="action-icon-btn start-icon"
                title="启动"
                @click="startService(svc.id)"
              >
                ▶
              </button>
              <button
                v-if="isRunning(svc.id) || isStarting(svc.id)"
                class="action-icon-btn stop-icon"
                title="停止"
                @click="stopService(svc.id)"
              >
                ■
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
                    <template
                      v-if="currentModule && currentService.launchType === 'maven-wrapper'"
                    >
                      · {{ currentModule }}
                    </template>
                  </span>
                </div>
                <div class="service-work-dir" :title="currentService.workDir">
                  📁 {{ currentService.workDir }}
                </div>
              </div>

              <!-- 右侧编辑/删除 -->
              <div class="service-mgmt-btns">
                <button class="btn secondary sm" @click="openEditModal">编辑配置</button>
                <button class="btn danger sm" @click="confirmDeleteService">删除</button>
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
                    <option value="custom">✏ 自定义多配置...</option>
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
                  :disabled="isRunning(currentService.id) || isStarting(currentService.id) || isStopping(currentService.id)"
                  @click="startCurrentService"
                >
                  <span class="btn-icon">▶</span> 启动
                </button>
                <button
                  class="btn danger action-btn"
                  :disabled="!isRunning(currentService.id) && !isStarting(currentService.id)"
                  @click="stopCurrentService"
                >
                  <span class="btn-icon">■</span> 停止
                </button>
                <button
                  class="btn secondary action-btn"
                  :disabled="!isRunning(currentService.id)"
                  @click="restartCurrentService"
                >
                  <span class="btn-icon">↻</span> 重启
                </button>
              </div>

              <!-- 状态与 PID -->
              <div class="status-summary">
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
                <span class="console-title">实时控制台 (Console)</span>
                <span class="log-count">{{ filteredLogs.length }} 行</span>
              </div>
              <div class="toolbar-right">
                <input
                  v-model="logFilter"
                  type="text"
                  placeholder="过滤日志 (支持文本过滤)..."
                  class="log-filter-input"
                />
                <label class="auto-scroll-label">
                  <input type="checkbox" v-model="autoScroll" />
                  自动滚屏
                </label>
                <button class="btn secondary sm" @click="clearLogs">清空</button>
                <button class="btn secondary sm" @click="copyLogs">
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
import type {
  MavenProjectInfo,
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

  // 扫描该服务支持的 profiles（多模块工程需要定位到启动模块的 resources）
  currentModule.value = (svc.mavenModule || '').trim();
  try {
    if (!currentModule.value && svc.launchType === 'maven-wrapper') {
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

function isRunning(id: string) {
  return statusMap.value[id]?.type === 'running';
}

function isStarting(id: string) {
  return statusMap.value[id]?.type === 'starting';
}

function isStopping(id: string) {
  return statusMap.value[id]?.type === 'stopping';
}

function getStatusClass(id: string) {
  const status = statusMap.value[id];
  if (!status) return 'stopped';
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
/* 全局重置与深色主题 */
* {
  box-sizing: border-box;
  margin: 0;
  padding: 0;
}

body,
html {
  width: 100%;
  height: 100%;
  overflow: hidden;
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif;
  background-color: #1e1e1e;
  color: #d4d4d4;
  user-select: none;
}

#app {
  width: 100vw;
  height: 100vh;
}
</style>

<style scoped>
.app-container {
  display: flex;
  flex-direction: column;
  width: 100vw;
  height: 100vh;
  background-color: #1e1e1e;
}

/* 顶部导航 */
.app-header {
  height: 42px;
  background-color: #252526;
  border-bottom: 1px solid #333333;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 16px;
  flex-shrink: 0;
}

.header-left {
  display: flex;
  align-items: center;
  gap: 8px;
}

.app-logo {
  font-size: 16px;
}

.app-title {
  font-size: 13px;
  font-weight: 600;
  color: #cccccc;
}

.app-badge {
  font-size: 11px;
  background-color: #333333;
  color: #858585;
  padding: 2px 6px;
  border-radius: 3px;
}

.header-right {
  display: flex;
  align-items: center;
  gap: 12px;
}

.stat-text {
  font-size: 12px;
  color: #858585;
}

.add-btn {
  font-weight: 500;
}

/* 主工作区 */
.app-workspace {
  flex: 1;
  display: flex;
  overflow: hidden;
}

/* 左侧边栏 */
.services-sidebar {
  width: 270px;
  background-color: #252526;
  border-right: 1px solid #333333;
  display: flex;
  flex-direction: column;
  flex-shrink: 0;
}

.sidebar-search {
  padding: 8px 10px;
  border-bottom: 1px solid #2d2d2d;
}

.search-input {
  width: 100%;
  background-color: #3c3c3c;
  border: 1px solid #3c3c3c;
  color: #cccccc;
  font-size: 12px;
  padding: 5px 8px;
  border-radius: 3px;
  outline: none;
}
.search-input:focus {
  border-color: #007acc;
}

.services-list {
  flex: 1;
  overflow-y: auto;
  padding: 4px 0;
}

.service-item {
  display: flex;
  align-items: center;
  padding: 8px 12px;
  gap: 10px;
  cursor: pointer;
  border-left: 3px solid transparent;
  transition: background-color 0.1s;
}

.service-item:hover {
  background-color: #2a2d2e;
}

.service-item.active {
  background-color: #37373d;
  border-left-color: #007acc;
}

.status-dot {
  width: 9px;
  height: 9px;
  border-radius: 50%;
  flex-shrink: 0;
}

.status-dot.running {
  background-color: #89d185;
  box-shadow: 0 0 6px #89d185;
}

.status-dot.starting,
.status-dot.stopping {
  background-color: #cca700;
  box-shadow: 0 0 5px #cca700;
}

.status-dot.stopped {
  background-color: #6e7681;
}

.status-dot.failed {
  background-color: #f14c4c;
  box-shadow: 0 0 6px #f14c4c;
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
  font-weight: 500;
  color: #cccccc;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.service-port {
  font-size: 11px;
  color: #4ec9b0;
  background-color: rgba(78, 201, 176, 0.12);
  padding: 1px 4px;
  border-radius: 3px;
}

.service-sub-row {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-top: 2px;
}

.type-tag {
  font-size: 10px;
  color: #858585;
  background-color: #333333;
  padding: 0 4px;
  border-radius: 2px;
}

.profile-preview {
  font-size: 11px;
  color: #6a9955;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.service-actions {
  display: flex;
  align-items: center;
  gap: 4px;
  opacity: 0;
  transition: opacity 0.15s;
}

.service-item:hover .service-actions {
  opacity: 1;
}

.action-icon-btn {
  background: none;
  border: none;
  font-size: 12px;
  cursor: pointer;
  padding: 2px 5px;
  border-radius: 3px;
}

.start-icon {
  color: #89d185;
}
.start-icon:hover {
  background-color: rgba(137, 209, 133, 0.2);
}

.stop-icon {
  color: #f14c4c;
}
.stop-icon:hover {
  background-color: rgba(241, 76, 76, 0.2);
}

.empty-list-tip {
  padding: 20px 14px;
  font-size: 12px;
  color: #6e7681;
  text-align: center;
  line-height: 1.6;
}

/* 右侧主面板 */
.main-panel {
  flex: 1;
  background-color: #1e1e1e;
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
  background-color: #252526;
  border-bottom: 1px solid #333333;
  padding: 12px 18px;
  display: flex;
  flex-direction: column;
  gap: 10px;
  flex-shrink: 0;
}

.control-header-top {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
}

.title-section {
  display: flex;
  flex-direction: column;
  gap: 3px;
}

.service-main-title {
  font-size: 16px;
  font-weight: 600;
  color: #e0e0e0;
  display: flex;
  align-items: center;
  gap: 8px;
}

.type-pill {
  font-size: 11px;
  font-weight: normal;
  background-color: #333333;
  color: #9cdcfe;
  padding: 2px 7px;
  border-radius: 3px;
}

.service-work-dir {
  font-size: 12px;
  color: #858585;
  max-width: 600px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.service-mgmt-btns {
  display: flex;
  gap: 6px;
}

.control-header-bottom {
  display: flex;
  align-items: center;
  gap: 16px;
  flex-wrap: wrap;
  padding-top: 4px;
}

.profile-box {
  display: flex;
  align-items: center;
  gap: 6px;
}

.box-label {
  font-size: 12px;
  color: #858585;
}

.active-profile-badge {
  font-size: 12px;
  padding: 3px 8px;
  border-radius: 3px;
  background-color: #333333;
  color: #858585;
  font-weight: 500;
}

.active-profile-badge.running {
  background-color: rgba(137, 209, 133, 0.15);
  color: #89d185;
  border: 1px solid rgba(137, 209, 133, 0.35);
}

.next-profile-box {
  display: flex;
  align-items: center;
  gap: 6px;
}

.profile-selector-wrap {
  display: flex;
  gap: 4px;
}

.profile-select {
  background-color: #3c3c3c;
  border: 1px solid #3c3c3c;
  color: #cccccc;
  font-size: 12px;
  padding: 4px 8px;
  border-radius: 3px;
  outline: none;
}
.profile-select:focus {
  border-color: #007acc;
}

.profile-custom-input {
  background-color: #3c3c3c;
  border: 1px solid #3c3c3c;
  color: #cccccc;
  font-size: 12px;
  padding: 4px 8px;
  border-radius: 3px;
  width: 100px;
  outline: none;
}
.profile-custom-input:focus {
  border-color: #007acc;
}

.profile-notice {
  font-size: 11px;
  color: #858585;
}

.action-btn-group {
  display: flex;
  gap: 6px;
}

.action-btn {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 5px 12px;
  font-size: 12px;
  font-weight: 500;
}

.btn-icon {
  font-size: 11px;
}

.status-summary {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-left: auto;
}

.state-label {
  font-size: 12px;
  color: #cccccc;
}

.pid-tag {
  font-size: 11px;
  background-color: #333333;
  color: #4ec9b0;
  padding: 2px 6px;
  border-radius: 3px;
}

/* 控制台日志区 */
.console-section {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background-color: #181a1f;
}

.console-toolbar {
  height: 34px;
  background-color: #21252b;
  border-bottom: 1px solid #282c34;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 14px;
  flex-shrink: 0;
}

.toolbar-left {
  display: flex;
  align-items: center;
  gap: 8px;
}

.console-title {
  font-size: 12px;
  font-weight: 500;
  color: #abb2bf;
}

.log-count {
  font-size: 11px;
  color: #5c6370;
}

.toolbar-right {
  display: flex;
  align-items: center;
  gap: 10px;
}

.log-filter-input {
  background-color: #1b1d23;
  border: 1px solid #3b4048;
  color: #abb2bf;
  font-size: 11px;
  padding: 3px 8px;
  border-radius: 3px;
  width: 170px;
  outline: none;
}
.log-filter-input:focus {
  border-color: #528bff;
}

.auto-scroll-label {
  font-size: 11px;
  color: #abb2bf;
  display: flex;
  align-items: center;
  gap: 4px;
  cursor: pointer;
}

.terminal-container {
  flex: 1;
  overflow-y: auto;
  padding: 10px 14px;
  font-family: 'Consolas', 'Cascadia Code', 'Courier New', monospace;
  font-size: 12px;
  line-height: 1.5;
  user-select: text;
}

.log-row {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  word-break: break-all;
}

.log-time {
  color: #5c6370;
  flex-shrink: 0;
}

.log-level {
  flex-shrink: 0;
  font-size: 10px;
  padding: 0 4px;
  border-radius: 2px;
  font-weight: 600;
}

.log-level.INFO {
  color: #61afef;
}
.log-level.WARN {
  color: #e5c07b;
}
.log-level.ERROR {
  color: #e06c75;
}
.log-level.DEBUG {
  color: #98c379;
}
.log-level.DEFAULT {
  color: #abb2bf;
}

.log-content {
  flex: 1;
  white-space: pre-wrap;
  color: #abb2bf;
}

.level-error .log-content {
  color: #e06c75;
}

.level-warn .log-content {
  color: #e5c07b;
}

.empty-log-tip,
.empty-state {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
  color: #5c6370;
  font-size: 13px;
}

/* 按钮基础 */
.btn {
  padding: 5px 12px;
  border-radius: 3px;
  font-size: 12px;
  border: none;
  cursor: pointer;
  transition: opacity 0.15s;
}
.btn:hover {
  opacity: 0.9;
}
.btn:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}

.btn.primary {
  background-color: #0e639c;
  color: #ffffff;
}
.btn.secondary {
  background-color: #3a3d41;
  color: #cccccc;
}
.btn.danger {
  background-color: #a1260d;
  color: #ffffff;
}

.btn.sm {
  padding: 3px 8px;
  font-size: 11px;
}
</style>
