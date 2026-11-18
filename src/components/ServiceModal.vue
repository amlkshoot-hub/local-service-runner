<template>
  <div v-if="isOpen" class="modal-backdrop" @click.self="close">
    <div class="modal-card">
      <div class="modal-header">
        <div class="modal-title">{{ isEdit ? '编辑服务配置' : '添加新服务' }}</div>
        <button class="icon-btn close-btn" @click="close">✕</button>
      </div>

      <div class="modal-body">
        <!-- 服务名称 -->
        <div class="form-group">
          <label>服务名称 <span class="required">*</span></label>
          <input
            v-model="form.name"
            type="text"
            class="input-field"
            placeholder="例如: User Service"
          />
        </div>

        <!-- 工作目录 -->
        <div class="form-group">
          <label>工作目录 (Work Directory) <span class="required">*</span></label>
          <div class="input-with-action">
            <input
              v-model="form.workDir"
              type="text"
              class="input-field"
              placeholder="例如: D:\projects\user-service"
              @blur="onWorkDirBlur"
            />
            <button class="btn secondary" @click="pickWorkDir" :disabled="picking">
              {{ picking ? '选择中...' : '📂 选择目录' }}
            </button>
            <button
              class="btn secondary"
              @click="scanProfiles"
              :disabled="scanning || !form.workDir.trim()"
            >
              {{ scanning ? '扫描中...' : '🔍 识别 Profile' }}
            </button>
          </div>
          <div v-if="detectedProfiles.length > 0" class="detect-tip">
            已检测到 Profile:
            <span
              v-for="p in detectedProfiles"
              :key="p"
              class="profile-tag"
              @click="addDetectedProfile(p)"
            >
              + {{ p }}
            </span>
          </div>
        </div>

        <!-- 启动类型 -->
        <div class="form-group">
          <label>启动类型</label>
          <div class="type-capsules">
            <button
              type="button"
              :class="['capsule-btn', { active: form.launchType === 'jar' }]"
              @click="form.launchType = 'jar'"
            >
              JAR 包
            </button>
            <button
              type="button"
              :class="['capsule-btn', { active: form.launchType === 'maven-wrapper' }]"
              @click="form.launchType = 'maven-wrapper'"
            >
              Maven Wrapper
            </button>
            <button
              type="button"
              :class="['capsule-btn', { active: form.launchType === 'maven-build-jar' }]"
              @click="form.launchType = 'maven-build-jar'"
            >
              构建+JAR 运行
            </button>
            <button
              type="button"
              :class="['capsule-btn', { active: form.launchType === 'source-main-class' }]"
              @click="form.launchType = 'source-main-class'"
            >
              源码主类直启
            </button>
            <button
              type="button"
              :class="['capsule-btn', { active: form.launchType === 'gradle-wrapper' }]"
              @click="form.launchType = 'gradle-wrapper'"
            >
              Gradle Wrapper
            </button>
            <button
              type="button"
              :class="['capsule-btn', { active: form.launchType === 'custom' }]"
              @click="form.launchType = 'custom'"
            >
              自定义命令
            </button>
          </div>
        </div>

        <!-- Maven 多模块：启动模块选择（Maven 系启动方式共用） -->
        <div
          v-if="isMavenBasedLaunch"
          class="form-group"
        >
          <label>Maven 启动模块 (多模块工程必选)</label>
          <select v-model="form.mavenModule" class="input-field">
            <option value="">自动检测（推荐）</option>
            <option v-for="m in moduleOptions" :key="m" :value="m">{{ m }}</option>
          </select>
          <div v-if="mavenInfo" class="module-hint">
            <span v-if="!mavenInfo.isMavenProject" class="hint-warn">
              ⚠ 未检测到 pom.xml，Maven 方式无法启动
            </span>
            <span v-else-if="mavenInfo.rootDeclaresSpringBootPlugin" class="hint-ok">
              ✓ 工程根目录本身即 Spring Boot 工程
            </span>
            <span v-else-if="mavenInfo.modules.length > 0" class="hint-ok">
              ✓ 检测到 {{ mavenInfo.modules.length }} 个可启动模块
              <template v-if="mavenInfo.suggestedModule">
                （推荐: {{ mavenInfo.suggestedModule }}）
              </template>
            </span>
            <span v-else class="hint-warn">
              ⚠ 工程中未找到 spring-boot-maven-plugin
            </span>
          </div>
          <div v-if="form.launchType === 'maven-build-jar'" class="module-hint">
            <span class="hint-info">
              ℹ 构建时对整个工程执行（不带 -pl）；模块仅用于定位构建产物 JAR 与 Profile 扫描
            </span>
          </div>
        </div>

        <!-- 构建+JAR 运行模式专有 -->
        <div v-if="form.launchType === 'maven-build-jar'" class="type-fields">
          <div class="form-group">
            <label>构建目标</label>
            <div class="goal-capsules">
              <button
                type="button"
                :class="['goal-btn', { active: buildGoalValue === 'package' }]"
                @click="form.buildGoal = 'package'"
              >
                mvn package
              </button>
              <button
                type="button"
                :class="['goal-btn', { active: buildGoalValue === 'install' }]"
                @click="form.buildGoal = 'install'"
              >
                mvn install
              </button>
            </div>
            <div class="module-hint">
              <span class="hint-info">
                多模块工程建议选 install：跨模块依赖会同步进入本地仓库，避免启动时找不到依赖
              </span>
            </div>
          </div>
          <div class="form-group checkbox-row">
            <label>
              <input type="checkbox" v-model="form.skipTests" />
              构建时跳过测试 (-DskipTests，推荐)
            </label>
          </div>
          <div class="form-group">
            <label>JAR 文件路径 (留空则构建后自动定位模块 target 下最新产物)</label>
            <div class="input-with-action">
              <input
                v-model="form.jarPath"
                type="text"
                class="input-field"
                placeholder="留空自动定位，例如: ry-admin/target/ry-admin.jar"
              />
              <button
                class="btn secondary"
                @click="locateJar"
                :disabled="locatingJar || !form.workDir.trim()"
              >
                {{ locatingJar ? '定位中...' : '📦 定位现有 JAR' }}
              </button>
            </div>
            <div v-if="locatedJarMessage" class="module-hint">
              <span :class="locatedJarFound ? 'hint-ok' : 'hint-warn'">{{ locatedJarMessage }}</span>
            </div>
          </div>
          <div class="form-group">
            <label>Java 可执行文件路径 (留空默认使用系统 PATH 中的 java)</label>
            <input
              v-model="form.javaPath"
              type="text"
              class="input-field"
              placeholder="例如: C:\Program Files\Java\jdk-17\bin\java.exe"
            />
          </div>
        </div>

        <!-- 源码主类直启模式专有 -->
        <div v-if="form.launchType === 'source-main-class'" class="type-fields">
          <div class="form-group">
            <label>启动主类 (全限定名) <span class="required">*</span></label>
            <div class="input-with-action">
              <input
                v-model="form.mainClass"
                type="text"
                class="input-field"
                placeholder="例如: com.ruoyi.RuoYiApplication"
              />
              <button
                class="btn secondary"
                @click="detectMainClasses"
                :disabled="detectingMain || !form.workDir.trim()"
              >
                {{ detectingMain ? '探测中...' : '🔍 探测主类' }}
              </button>
            </div>
            <div v-if="detectedMainClasses.length > 0" class="detect-tip">
              <span
                v-for="m in detectedMainClasses"
                :key="m.fqcn"
                class="profile-tag"
                @click="selectMainClass(m.fqcn)"
              >
                + {{ m.fqcn }}{{ m.isSpringBootEntry ? ' ⭐' : '' }}
              </span>
            </div>
            <div v-if="detectedMainHint" class="module-hint">
              <span class="hint-warn">{{ detectedMainHint }}</span>
            </div>
          </div>
        </div>

        <!-- JAR 模式专有 -->
        <div v-if="form.launchType === 'jar'" class="type-fields">
          <div class="form-group">
            <label>JAR 文件路径 <span class="required">*</span></label>
            <input
              v-model="form.jarPath"
              type="text"
              class="input-field"
              placeholder="例如: target/user-service.jar 或完整绝对路径"
            />
          </div>
          <div class="form-group">
            <label>Java 可执行文件路径 (留空默认使用系统 PATH 中的 java)</label>
            <input
              v-model="form.javaPath"
              type="text"
              class="input-field"
              placeholder="例如: C:\Program Files\Java\jdk-17\bin\java.exe"
            />
          </div>
        </div>

        <!-- 自定义命令模式专有 -->
        <div v-if="form.launchType === 'custom'" class="type-fields">
          <div class="form-group">
            <label>执行程序路径 <span class="required">*</span></label>
            <input
              v-model="form.customProgram"
              type="text"
              class="input-field"
              placeholder="例如: node, python, 或自定义 .exe"
            />
          </div>
          <div class="form-group checkbox-row">
            <label>
              <input type="checkbox" v-model="form.injectProfileCustom" />
              注入 Spring Boot Profile 参数 (--spring.profiles.active=...)
            </label>
          </div>
        </div>

        <!-- 额外命令行参数 -->
        <div class="form-group">
          <label>额外命令行参数 (以空格分隔多个参数)</label>
          <input
            v-model="argsString"
            type="text"
            class="input-field"
            placeholder="例如: --server.port=8081 -Dmy.param=value"
          />
        </div>

        <!-- 默认 Profile -->
        <div class="form-group">
          <label>初始 Profile (多个以逗号分隔，如 dev,local；default 表示不显式指定)</label>
          <input
            v-model="profilesString"
            type="text"
            class="input-field"
            placeholder="例如: dev 或 dev,local"
          />
        </div>

        <!-- 高级折叠选项 -->
        <div class="collapsible">
          <div class="collapsible-header" @click="showAdvanced = !showAdvanced">
            <span>{{ showAdvanced ? '▼ 收起高级设置' : '▶ 展开高级设置' }}</span>
          </div>
          <div v-if="showAdvanced" class="collapsible-body">
            <div class="form-row">
              <div class="form-group flex-1">
                <label>显示端口号 (仅用于界面标识)</label>
                <input
                  v-model.number="form.port"
                  type="number"
                  class="input-field"
                  placeholder="例如: 8080"
                />
              </div>
              <div class="form-group flex-1">
                <label>正常停止超时 (秒，默认 10)</label>
                <input
                  v-model.number="form.stopTimeoutSecs"
                  type="number"
                  class="input-field"
                  placeholder="10"
                />
              </div>
            </div>
          </div>
        </div>

        <div v-if="errorMessage" class="error-banner">
          {{ errorMessage }}
        </div>
      </div>

      <div class="modal-footer">
        <button class="btn secondary" @click="close">取消</button>
        <button class="btn primary" @click="save">保存服务</button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { MainClassCandidate, MavenProjectInfo, ServiceConfig } from '../types';

const props = defineProps<{
  isOpen: boolean;
  serviceData?: ServiceConfig | null;
}>();

const emit = defineEmits<{
  (e: 'close'): void;
  (e: 'saved', service: ServiceConfig): void;
}>();

const isEdit = ref(false);
const showAdvanced = ref(false);
const scanning = ref(false);
const picking = ref(false);
const detectedProfiles = ref<string[]>([]);
const detectedModules = ref<string[]>([]);
const mavenInfo = ref<MavenProjectInfo | null>(null);
/** 最近一次模块探测对应的工作目录，用于目录切换时清理失效的模块配置 */
const detectedForDir = ref('');
const errorMessage = ref('');

const argsString = ref('');
const profilesString = ref('dev');

// 构建+JAR 运行 / 源码主类直启的辅助状态
const locatingJar = ref(false);
const locatedJarMessage = ref('');
const locatedJarFound = ref(false);
const detectingMain = ref(false);
const detectedMainClasses = ref<MainClassCandidate[]>([]);
const detectedMainHint = ref('');

const form = ref<ServiceConfig>({
  id: '',
  name: '',
  workDir: '',
  launchType: 'maven-wrapper',
  jarPath: '',
  customProgram: '',
  args: [],
  envVars: {},
  port: undefined,
  javaPath: '',
  selectedProfiles: ['dev'],
  mavenModule: '',
  stopTimeoutSecs: 10,
  injectProfileCustom: false,
  buildGoal: 'package',
  skipTests: true,
  mainClass: '',
});

const isMavenBasedLaunch = computed(() =>
  ['maven-wrapper', 'maven-build-jar', 'source-main-class'].includes(form.value.launchType)
);

const buildGoalValue = computed<'package' | 'install'>(() => {
  const goal = (form.value.buildGoal || '').trim();
  return goal === 'install' ? 'install' : 'package';
});

const moduleOptions = computed(() => {
  const options = [...detectedModules.value];
  const current = (form.value.mavenModule || '').trim();
  if (current && !options.includes(current)) {
    options.push(current);
  }
  return options;
});

watch(
  () => props.isOpen,
  (val) => {
    if (val) {
      errorMessage.value = '';
      if (props.serviceData) {
        isEdit.value = true;
        form.value = JSON.parse(JSON.stringify(props.serviceData));
        form.value.mavenModule = form.value.mavenModule || '';
        form.value.mainClass = form.value.mainClass || '';
        form.value.jarPath = form.value.jarPath || '';
        argsString.value = (form.value.args || []).join(' ');
        profilesString.value = (form.value.selectedProfiles || []).join(',');
      } else {
        isEdit.value = false;
        form.value = {
          id: 'svc-' + Date.now(),
          name: '',
          workDir: '',
          launchType: 'maven-wrapper',
          jarPath: '',
          customProgram: '',
          args: [],
          envVars: {},
          port: undefined,
          javaPath: '',
          selectedProfiles: ['dev'],
          mavenModule: '',
          stopTimeoutSecs: 10,
          injectProfileCustom: false,
          buildGoal: 'package',
          skipTests: true,
          mainClass: '',
        };
        argsString.value = '';
        profilesString.value = 'dev';
      }
      detectedProfiles.value = [];
      detectedModules.value = [];
      mavenInfo.value = null;
      detectedForDir.value = '';
      locatedJarMessage.value = '';
      locatedJarFound.value = false;
      detectedMainClasses.value = [];
      detectedMainHint.value = '';
      if (form.value.workDir) {
        // 编辑已有服务时，先探测一次模块信息，便于核对/调整启动模块
        void detectModules();
      }
    }
  }
);

function close() {
  emit('close');
}

/** 取路径最后一级目录名（同时兼容 `\` 与 `/` 分隔符） */
function lastSegment(path: string): string {
  const trimmed = (path || '').trim().replace(/[\\/]+$/, '');
  if (!trimmed) return '';
  const parts = trimmed.split(/[\\/]/);
  return parts[parts.length - 1] || '';
}

/** 打开系统目录选择框；选定后目录名默认作为服务名称 */
async function pickWorkDir() {
  if (picking.value) return;
  picking.value = true;
  errorMessage.value = '';
  try {
    const picked = await invoke<string | null>('pick_directory', {
      title: '选择已存在的项目目录',
      initialDir: form.value.workDir || '',
    });
    if (!picked) return;

    const previousDir = form.value.workDir.trim();
    const previousBase = lastSegment(previousDir);
    const trimmedName = form.value.name.trim();
    // 名称仍为空 / 仍等于上一个目录名时，跟随新目录自动更新；用户手动改过则保留
    const shouldRename = !trimmedName || trimmedName === previousBase;

    form.value.workDir = picked;
    if (shouldRename) {
      form.value.name = lastSegment(picked);
    }
    // 目录变了，旧目录下的相对模块路径不再有效
    if (previousDir && previousDir !== picked) {
      form.value.mavenModule = '';
    }

    detectedProfiles.value = [];
    await detectModules();
    await scanProfiles();
  } catch (err: any) {
    errorMessage.value = '选择目录失败: ' + (err?.message || err);
  } finally {
    picking.value = false;
  }
}

/** 探测 Maven 多模块结构，给出可启动模块候选 */
async function detectModules() {
  const workDir = form.value.workDir.trim();
  if (!workDir) {
    detectedModules.value = [];
    mavenInfo.value = null;
    return;
  }
  try {
    const info = await invoke<MavenProjectInfo>('detect_maven_modules', { workDir });
    mavenInfo.value = info;
    detectedModules.value = info.modules || [];
    detectedForDir.value = workDir;
    if (!form.value.mavenModule && info.suggestedModule) {
      form.value.mavenModule = info.suggestedModule;
    }
  } catch (err) {
    console.error('Detect maven modules error:', err);
    mavenInfo.value = null;
    detectedModules.value = [];
  }
}

async function onWorkDirBlur() {
  const workDir = form.value.workDir.trim();
  if (!workDir) return;
  if (!form.value.name.trim()) {
    form.value.name = lastSegment(workDir);
  }
  // 手动修改了工作目录时，旧的相对模块路径失效
  if (detectedForDir.value && detectedForDir.value !== workDir) {
    form.value.mavenModule = '';
  }
  await detectModules();
  if (detectedProfiles.value.length === 0) {
    await scanProfiles();
  }
}

async function scanProfiles() {
  if (!form.value.workDir.trim()) return;
  scanning.value = true;
  try {
    const list = await invoke<string[]>('detect_profiles', {
      workDir: form.value.workDir.trim(),
      module: form.value.mavenModule || '',
    });
    detectedProfiles.value = list;
  } catch (err) {
    console.error('Scan profiles error:', err);
  } finally {
    scanning.value = false;
  }
}

function addDetectedProfile(p: string) {
  const current = profilesString.value
    .split(',')
    .map((s) => s.trim())
    .filter(Boolean);
  if (!current.includes(p)) {
    current.push(p);
    profilesString.value = current.join(',');
  }
}

/** 定位工程（优先启动模块）target 下的现有 JAR 产物 */
async function locateJar() {
  if (!form.value.workDir.trim()) return;
  locatingJar.value = true;
  locatedJarMessage.value = '';
  try {
    const jar = await invoke<string | null>('locate_built_jar', {
      workDir: form.value.workDir.trim(),
      module: form.value.mavenModule || '',
    });
    if (jar) {
      locatedJarFound.value = true;
      locatedJarMessage.value = `✓ 已定位 JAR: ${jar}`;
      if (!form.value.jarPath?.trim()) {
        form.value.jarPath = jar;
      }
    } else {
      locatedJarFound.value = false;
      locatedJarMessage.value = '⚠ 尚未发现构建产物，启动时会先执行构建再自动定位';
    }
  } catch (err: any) {
    locatedJarFound.value = false;
    locatedJarMessage.value = '⚠ ' + (err?.message || err);
  } finally {
    locatingJar.value = false;
  }
}

/** 探测工程源码中的可执行主类（@SpringBootApplication 优先） */
async function detectMainClasses() {
  if (!form.value.workDir.trim()) return;
  detectingMain.value = true;
  detectedMainClasses.value = [];
  detectedMainHint.value = '';
  try {
    const list = await invoke<MainClassCandidate[]>('detect_main_classes', {
      workDir: form.value.workDir.trim(),
      module: form.value.mavenModule || '',
    });
    if (list.length === 0) {
      detectedMainHint.value =
        '未扫描到含 @SpringBootApplication 或 main 方法的类，请检查目录/模块或手动填写全限定类名';
    } else {
      detectedMainClasses.value = list;
      // 若当前主类为空且只有一个候选，直接自动填入
      if (!form.value.mainClass?.trim() && list.length >= 1) {
        selectMainClass(list[0].fqcn);
      }
    }
  } catch (err: any) {
    detectedMainHint.value = '探测失败: ' + (err?.message || err);
  } finally {
    detectingMain.value = false;
  }
}

function selectMainClass(fqcn: string) {
  form.value.mainClass = fqcn;
}

function save() {
  errorMessage.value = '';
  if (!form.value.name.trim()) {
    errorMessage.value = '请输入服务名称';
    return;
  }
  if (!form.value.workDir.trim()) {
    errorMessage.value = '请输入工作目录';
    return;
  }
  if (form.value.launchType === 'jar' && !form.value.jarPath?.trim()) {
    errorMessage.value = 'JAR 启动模式下必须指定 JAR 文件路径';
    return;
  }
  if (form.value.launchType === 'custom' && !form.value.customProgram?.trim()) {
    errorMessage.value = '自定义命令模式下必须指定可执行程序路径';
    return;
  }
  if (
    form.value.launchType === 'source-main-class' &&
    !(form.value.mainClass || '').trim()
  ) {
    errorMessage.value = '源码主类直启模式下必须指定启动主类，可点击「探测主类」自动识别';
    return;
  }

  // 解析 args
  const rawArgs = argsString.value.trim();
  form.value.args = rawArgs ? rawArgs.split(/\s+/).filter(Boolean) : [];

  // 解析 selectedProfiles
  const rawProfiles = profilesString.value
    .split(',')
    .map((p) => p.trim())
    .filter(Boolean);
  form.value.selectedProfiles = rawProfiles.length > 0 ? rawProfiles : ['default'];

  // 规范化 Maven 模块与新模式字段：非对应启动方式不保留无关配置
  const isMavenLaunch = isMavenBasedLaunch.value;
  const normalizedModule = isMavenLaunch ? (form.value.mavenModule || '').trim() : '';
  const normalizedBuildGoal =
    form.value.launchType === 'maven-build-jar' ? buildGoalValue.value : null;
  const normalizedSkipTests =
    form.value.launchType === 'maven-build-jar' ? !!form.value.skipTests : false;
  const normalizedMainClass =
    form.value.launchType === 'source-main-class' ? (form.value.mainClass || '').trim() : null;

  const payload: ServiceConfig = JSON.parse(JSON.stringify(form.value));
  payload.mavenModule = normalizedModule || null;
  payload.buildGoal = normalizedBuildGoal;
  payload.skipTests = normalizedSkipTests;
  payload.mainClass = normalizedMainClass;
  // 构建+JAR 模式下 jarPath 允许留空（自动定位），其余非 JAR 模式清空
  if (form.value.launchType !== 'maven-build-jar' && form.value.launchType !== 'jar') {
    payload.jarPath = '';
  }

  emit('saved', payload);
  close();
}
</script>

<style scoped>
.modal-backdrop {
  position: fixed;
  top: 0;
  left: 0;
  width: 100vw;
  height: 100vh;
  background-color: rgba(0, 0, 0, 0.65);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
  backdrop-filter: blur(2px);
}

.modal-card {
  width: 620px;
  max-width: 90vw;
  max-height: 85vh;
  background-color: #21252b;
  border: 1px solid #3e4451;
  border-radius: 6px;
  box-shadow: 0 12px 32px rgba(0, 0, 0, 0.5);
  display: flex;
  flex-direction: column;
}

.modal-header {
  padding: 14px 18px;
  border-bottom: 1px solid #2c313a;
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.modal-title {
  font-size: 15px;
  font-weight: 600;
  color: #d7dae0;
}

.close-btn {
  background: none;
  border: none;
  color: #abb2bf;
  font-size: 16px;
  cursor: pointer;
  padding: 4px;
}
.close-btn:hover {
  color: #e06c75;
}

.modal-body {
  padding: 16px 18px;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.form-group {
  display: flex;
  flex-direction: column;
  gap: 5px;
}

.form-group label {
  font-size: 12px;
  color: #abb2bf;
  font-weight: 500;
}

.required {
  color: #e06c75;
}

.input-field {
  background-color: #1b1d23;
  border: 1px solid #3b4048;
  color: #d7dae0;
  padding: 7px 10px;
  border-radius: 4px;
  font-size: 13px;
  outline: none;
}
.input-field:focus {
  border-color: #528bff;
}

.input-with-action {
  display: flex;
  gap: 8px;
}
.input-with-action input {
  flex: 1;
}

.type-capsules {
  display: flex;
  gap: 6px;
  flex-wrap: wrap;
}

.goal-capsules {
  display: flex;
  gap: 6px;
}

.goal-btn {
  flex: 1;
  padding: 6px 12px;
  font-size: 12px;
  font-family: 'Consolas', 'Cascadia Code', 'Courier New', monospace;
  border: 1px solid #3b4048;
  background-color: #1b1d23;
  color: #abb2bf;
  border-radius: 4px;
  cursor: pointer;
  transition: all 0.15s ease;
}

.goal-btn.active {
  background-color: #2c384e;
  border-color: #528bff;
  color: #ffffff;
  font-weight: 500;
}

.hint-info {
  color: #9cdcfe;
}

.capsule-btn {
  flex: 1 1 auto;
  padding: 7px 10px;
  font-size: 12px;
  border: 1px solid #3b4048;
  background-color: #1b1d23;
  color: #abb2bf;
  border-radius: 4px;
  cursor: pointer;
  transition: all 0.15s ease;
  white-space: nowrap;
}

.capsule-btn.active {
  background-color: #2c384e;
  border-color: #528bff;
  color: #ffffff;
  font-weight: 500;
}

.detect-tip {
  font-size: 11px;
  color: #98c379;
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
  margin-top: 3px;
}

.profile-tag {
  background-color: rgba(152, 195, 121, 0.15);
  border: 1px solid rgba(152, 195, 121, 0.3);
  padding: 2px 6px;
  border-radius: 3px;
  cursor: pointer;
}
.profile-tag:hover {
  background-color: rgba(152, 195, 121, 0.3);
}

.module-hint {
  font-size: 11px;
  margin-top: 2px;
  line-height: 1.5;
}

.hint-ok {
  color: #98c379;
}

.hint-warn {
  color: #e5c07b;
}

.checkbox-row label {
  display: flex;
  align-items: center;
  gap: 6px;
  cursor: pointer;
}

.collapsible {
  border: 1px dashed #3e4451;
  border-radius: 4px;
  padding: 8px 10px;
}

.collapsible-header {
  font-size: 12px;
  color: #528bff;
  cursor: pointer;
}

.collapsible-body {
  margin-top: 8px;
}

.form-row {
  display: flex;
  gap: 12px;
}

.flex-1 {
  flex: 1;
}

.error-banner {
  background-color: rgba(224, 108, 117, 0.15);
  border: 1px solid #e06c75;
  color: #e06c75;
  padding: 8px 12px;
  border-radius: 4px;
  font-size: 12px;
}

.modal-footer {
  padding: 12px 18px;
  border-top: 1px solid #2c313a;
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}

.btn {
  padding: 6px 14px;
  border-radius: 4px;
  font-size: 12px;
  cursor: pointer;
  border: none;
  transition: opacity 0.15s;
}
.btn:hover {
  opacity: 0.9;
}
.btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
.btn.primary {
  background-color: #528bff;
  color: #ffffff;
}
.btn.secondary {
  background-color: #353b45;
  color: #d7dae0;
}
</style>
