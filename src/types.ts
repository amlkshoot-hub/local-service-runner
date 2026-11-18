export type LaunchType =
  | 'jar'
  | 'maven-wrapper'
  | 'gradle-wrapper'
  | 'custom'
  | 'maven-build-jar'
  | 'source-main-class';

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
  /** maven-build-jar：构建目标（package / install），留空默认 package */
  buildGoal?: 'package' | 'install' | null;
  /** maven-build-jar：构建时是否追加 -DskipTests */
  skipTests: boolean;
  /** source-main-class：主类全限定名 */
  mainClass?: string | null;
}

export interface MavenProjectInfo {
  isMavenProject: boolean;
  rootDeclaresSpringBootPlugin: boolean;
  hasModules: boolean;
  modules: string[];
  suggestedModule?: string | null;
}

export interface MainClassCandidate {
  fqcn: string;
  isSpringBootEntry: boolean;
}

export type ServiceStatus =
  | { type: 'stopped' }
  | { type: 'starting' }
  | { type: 'running'; pid: number; activeProfiles: string[]; startedAt: string }
  | { type: 'stopping' }
  | { type: 'building'; pid: number; goal: string }
  | { type: 'failed'; exitCode?: number; error: string };

export type MavenTaskStatus =
  | { type: 'idle' }
  | { type: 'running'; pid: number; goal: string }
  | { type: 'success'; goal: string; exitCode: number }
  | { type: 'failed'; goal: string; exitCode: number };

export interface MavenTaskStatusEvent {
  serviceId: string;
  status: MavenTaskStatus;
}

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
