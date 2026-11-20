<template>
  <section class="terminal-panel" aria-label="项目终端" @keydown.stop>
    <header class="terminal-toolbar">
      <strong><AppIcon name="terminal" :size="13" /> PowerShell</strong>
      <span v-if="message" class="terminal-message" role="status">{{ message }}</span>
      <div class="terminal-actions">
        <button v-if="!running && !starting" type="button" @click="start">重新打开</button>
        <button type="button" @click="emit('collapse')">收起</button>
        <button type="button" @click="emit('close')">结束会话</button>
      </div>
    </header>
    <div ref="surface" class="terminal-surface"></div>
  </section>
</template>

<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { Channel, invoke } from '@tauri-apps/api/core';
import { Terminal } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import '@xterm/xterm/css/xterm.css';
import AppIcon from './AppIcon.vue';

const props = defineProps<{ serviceId: string; visible: boolean }>();
const emit = defineEmits<{ collapse: []; close: [] }>();
const surface = ref<HTMLDivElement>();
const running = ref(false);
const starting = ref(false);
const message = ref('');
const fit = new FitAddon();
const terminal = new Terminal({
  fontFamily: 'Cascadia Mono, Consolas, monospace',
  fontSize: 13,
  cursorBlink: true,
});
let observer: ResizeObserver | undefined;
let sessionId: string | undefined;

function reportError(error: unknown) {
  message.value = String(error);
}

function sendInput(data: string) {
  if (running.value && sessionId) {
    void invoke('write_embedded_terminal', { sessionId, data }).catch(reportError);
  }
}

// 只在面板可见时计算尺寸，收起后继续接收终端输出。
function fitVisibleTerminal() {
  if (props.visible && surface.value?.clientHeight && surface.value.clientWidth) fit.fit();
}

async function start() {
  if (running.value || starting.value) return;
  starting.value = true;
  message.value = '正在启动…';
  const id = crypto.randomUUID();
  sessionId = id;
  terminal.reset();
  fitVisibleTerminal();
  let exited = false;
  const output = new Channel<ArrayBuffer>((data) => {
    if (sessionId !== id) return;
    if (data.byteLength) {
      // 直接写入 UTF-8 字节，避免中文字符跨数据块时被拆坏。
      terminal.write(new Uint8Array(data));
    } else {
      exited = true;
      running.value = false;
      if (!starting.value) message.value = '会话已结束';
    }
  });

  try {
    await invoke('start_embedded_terminal', {
      id: props.serviceId, sessionId: id, cols: terminal.cols, rows: terminal.rows, output,
    });
    // 面板可能在 Shell 启动完成前已被关闭。
    if (sessionId !== id) {
      await invoke('close_embedded_terminal', { sessionId: id });
      return;
    }
    running.value = !exited;
    message.value = exited ? '会话已结束' : '';
    if (running.value) {
      void invoke('resize_embedded_terminal', { sessionId: id, cols: terminal.cols, rows: terminal.rows })
        .catch(reportError);
      if (props.visible) terminal.focus();
    }
  } catch (error) {
    if (sessionId === id) {
      sessionId = undefined;
      reportError(error);
    }
  } finally {
    starting.value = false;
  }
}

onMounted(() => {
  const surfaceStyle = getComputedStyle(surface.value!);
  // xterm 独立渲染文字和光标，使用应用的浅色底和高对比 ANSI 前景色。
  terminal.options.theme = {
    background: surfaceStyle.backgroundColor,
    foreground: surfaceStyle.color,
    cursor: surfaceStyle.getPropertyValue('--accent-text').trim(),
    cursorAccent: surfaceStyle.backgroundColor,
    selectionBackground: '#dbe5ff',
    black: '#1b2434',
    red: '#b4232c',
    green: '#17643d',
    yellow: '#805000',
    blue: '#254caa',
    magenta: '#74378b',
    cyan: '#006878',
    white: '#38455d',
    brightBlack: '#59677d',
    brightRed: '#c62035',
    brightGreen: '#187844',
    brightYellow: '#905800',
    brightBlue: '#3a56d4',
    brightMagenta: '#8d409c',
    brightCyan: '#007487',
    brightWhite: '#26334b',
  };
  terminal.loadAddon(fit);
  terminal.open(surface.value!);
  terminal.onData(sendInput);
  terminal.onResize(({ cols, rows }) => {
    if (running.value && sessionId) {
      void invoke('resize_embedded_terminal', { sessionId, cols, rows }).catch(reportError);
    }
  });
  terminal.attachCustomKeyEventHandler((event) => {
    if (event.type !== 'keydown' || !event.ctrlKey || !event.shiftKey) return true;
    const key = event.key.toLowerCase();
    if (key === 'c') {
      event.preventDefault();
      if (terminal.hasSelection()) {
        void navigator.clipboard.writeText(terminal.getSelection()).catch(reportError);
      }
      return false;
    }
    if (key === 'v') {
      event.preventDefault();
      void navigator.clipboard.readText().then((text) => {
        if (running.value) terminal.paste(text);
      }).catch(reportError);
      return false;
    }
    return true;
  });
  observer = new ResizeObserver(fitVisibleTerminal);
  observer.observe(surface.value!);
  void start();
});

watch(() => props.visible, async (visible) => {
  if (!visible) return;
  await nextTick();
  fitVisibleTerminal();
  terminal.focus();
});

onBeforeUnmount(() => {
  const id = sessionId;
  sessionId = undefined;
  running.value = false;
  if (id) void invoke('close_embedded_terminal', { sessionId: id }).catch(console.error);
  observer?.disconnect();
  terminal.dispose();
});
</script>

<style scoped>
.terminal-panel {
  flex: 0 0 clamp(140px, 34%, 320px);
  display: flex;
  flex-direction: column;
  min-height: 0;
  border-top: 1px solid var(--line-2);
  background: var(--bg-console);
}
.terminal-toolbar {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 5px 12px;
  min-height: 36px;
  font-size: 12px;
  background: var(--bg-panel);
  border-bottom: 1px solid var(--line-1);
}
.terminal-toolbar strong, .terminal-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  white-space: nowrap;
}
.terminal-actions {
  margin-left: auto;
}
.terminal-actions button {
  padding: 3px 7px;
  font: inherit;
  color: inherit;
  border: 1px solid var(--line-2);
  border-radius: 5px;
  background: transparent;
  cursor: pointer;
}
.terminal-actions button:hover {
  background: var(--bg-hover);
}
.terminal-message {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.terminal-surface {
  flex: 1;
  overflow: hidden;
  min-height: 0;
  padding: 7px 10px;
  background: var(--bg-console);
  color: var(--text-1);
}
</style>
