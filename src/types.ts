export type LaunchType = 'jar' | 'maven-wrapper' | 'gradle-wrapper' | 'custom';

export interface ServiceConfig {
  id: string;
  name: string;
  workDir: string;
  launchType: LaunchType;
  jarPath?: string;
  customProgram?: string;
  args: string[];
  envVars: Record<string, string>;
  port?: number;
  javaPath?: string;
  selectedProfiles: string[];
  /** 多模块 Maven 工程的启动模块（相对工作目录）；留空表示自动检测 */
  mavenModule?: string | null;
  stopTimeoutSecs: number;
  injectProfileCustom: boolean;
}

export interface MavenProjectInfo {
  isMavenProject: boolean;
  rootDeclaresSpringBootPlugin: boolean;
  hasModules: boolean;
  modules: string[];
  suggestedModule?: string | null;
}

export type ServiceStatus =
  | { type: 'stopped' }
  | { type: 'starting' }
  | { type: 'running'; pid: number; activeProfiles: string[]; startedAt: string }
  | { type: 'stopping' }
  | { type: 'failed'; exitCode?: number; error: string };

export interface LogEntry {
  timestamp: string;
  level: string; // 'INFO' | 'WARN' | 'ERROR' | 'DEBUG' | 'DEFAULT'
  content: string;
  source: string; // 'stdout' | 'stderr' | 'system'
}

export interface ServiceLogEvent {
  serviceId: string;
  entry: LogEntry;
}

export interface StatusChangeEvent {
  serviceId: string;
  status: ServiceStatus;
}
