use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Write};
use std::mem::{size_of, zeroed};
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::Path;
use std::ptr::{null, null_mut};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use tauri::ipc::{Channel, Response};
use windows_sys::Win32::System::{
    Console::{COORD, ClosePseudoConsole, CreatePseudoConsole, HPCON, ResizePseudoConsole},
    JobObjects::{
        CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JobObjectExtendedLimitInformation, SetInformationJobObject,
    },
    Pipes::CreatePipe,
    Threading::{
        CreateProcessW, DeleteProcThreadAttributeList, EXTENDED_STARTUPINFO_PRESENT, INFINITE,
        InitializeProcThreadAttributeList, PROC_THREAD_ATTRIBUTE_JOB_LIST,
        PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE, PROCESS_INFORMATION, STARTF_USESTDHANDLES,
        STARTUPINFOEXW, UpdateProcThreadAttribute, WaitForSingleObject,
    },
};

#[derive(Default)]
pub struct TerminalManager {
    // 用会话 ID 区分同一项目重新打开的终端，避免旧事件影响新会话。
    sessions: Arc<Mutex<HashMap<String, TerminalSession>>>,
}

struct TerminalSession {
    // 先关闭 Job，终端命令派生的进程也会随会话清理。
    job: OwnedHandle,
    console: PseudoConsole,
    input: mpsc::SyncSender<String>,
}

struct PseudoConsole(HPCON);

impl Drop for PseudoConsole {
    fn drop(&mut self) {
        // ConPTY 关闭时可能等待输出管道排空，不能阻塞 UI 线程。
        let handle = self.0;
        thread::spawn(move || unsafe { ClosePseudoConsole(handle) });
    }
}

impl TerminalManager {
    /// 在服务工作目录启动一个内嵌 PowerShell 会话。
    pub fn open(
        &self,
        session_id: String,
        dir: &Path,
        cols: u16,
        rows: u16,
        output: Channel<Response>,
    ) -> Result<(), String> {
        if !dir.is_dir() {
            return Err(format!("终端工作目录不存在（{}）", dir.display()));
        }
        let size = terminal_size(cols, rows)?;
        let mut sessions = self.sessions.lock().unwrap();
        if sessions.contains_key(&session_id) {
            return Err("终端会话已存在".into());
        }

        let (console_input, mut input_writer) = pipe()?;
        let (mut output_reader, console_output) = pipe()?;
        let mut handle = 0;
        let result = unsafe {
            CreatePseudoConsole(
                size,
                console_input.as_raw_handle(),
                console_output.as_raw_handle(),
                0,
                &mut handle,
            )
        };
        if result < 0 {
            return Err(format!("创建 Windows 伪控制台失败（0x{result:08X}）"));
        }
        let console = PseudoConsole(handle);
        // 始终排空输出管道，窗口隐藏或 WebView 断开时也允许 ConPTY 正常退出。
        thread::spawn(move || {
            let mut buffer = [0; 8192];
            while let Ok(size) = output_reader.read(&mut buffer) {
                if size == 0 {
                    break;
                }
                let _ = output.send(Response::new(buffer[..size].to_vec()));
            }
            let _ = output.send(Response::new(Vec::<u8>::new()));
        });
        let job = terminal_job()?;
        let process = console.start_shell(dir, &job)?;
        drop((console_input, console_output));

        // 输入与输出分别处理，避免粘贴大量内容时阻塞界面或输出。
        let (sender, receiver) = mpsc::sync_channel::<String>(64);
        thread::spawn(move || {
            while let Ok(data) = receiver.recv() {
                if input_writer.write_all(data.as_bytes()).is_err() {
                    break;
                }
            }
        });
        sessions.insert(
            session_id.clone(),
            TerminalSession {
                job,
                console,
                input: sender,
            },
        );
        let sessions = self.sessions.clone();
        thread::spawn(move || {
            // exit 由 Shell 发起时也要关闭 ConPTY，输出线程才能发出结束信号。
            unsafe { WaitForSingleObject(process.as_raw_handle(), INFINITE) };
            sessions.lock().unwrap().remove(&session_id);
        });
        Ok(())
    }

    /// 将键盘输入转发给对应的终端会话。
    pub fn write(&self, session_id: &str, data: String) -> Result<(), String> {
        let sessions = self.sessions.lock().unwrap();
        let session = sessions.get(session_id).ok_or("终端会话已关闭")?;
        session.input.try_send(data).map_err(|err| match err {
            mpsc::TrySendError::Full(_) => "终端输入繁忙，请稍后重试".into(),
            mpsc::TrySendError::Disconnected(_) => "终端会话已关闭".into(),
        })
    }

    /// 同步终端面板的行列数，使命令行程序正确换行。
    pub fn resize(&self, session_id: &str, cols: u16, rows: u16) -> Result<(), String> {
        let size = terminal_size(cols, rows)?;
        let sessions = self.sessions.lock().unwrap();
        let session = sessions.get(session_id).ok_or("终端会话已关闭")?;
        let result = unsafe { ResizePseudoConsole(session.console.0, size) };
        if result < 0 {
            return Err(format!("调整终端大小失败（0x{result:08X}）"));
        }
        Ok(())
    }

    /// 关闭指定终端并释放伪控制台。
    pub fn close(&self, session_id: &str) {
        self.sessions.lock().unwrap().remove(session_id);
    }

    /// Runner 退出时关闭仍在运行的终端。
    pub fn close_all(&self) {
        // 真正退出前同步关闭会话，确保子进程收到关闭通知。
        let sessions = std::mem::take(&mut *self.sessions.lock().unwrap());
        for (_, session) in sessions {
            let TerminalSession {
                job,
                console,
                input,
            } = session;
            drop((job, input));
            let handle = console.0;
            std::mem::forget(console);
            unsafe { ClosePseudoConsole(handle) };
        }
    }
}

impl PseudoConsole {
    fn start_shell(&self, dir: &Path, job: &OwnedHandle) -> Result<OwnedHandle, String> {
        let mut cwd: Vec<u16> = dir.as_os_str().encode_wide().collect();
        if cwd.contains(&0) {
            return Err("终端工作目录含有非法字符".into());
        }
        cwd.push(0);
        let shell = std::env::var_os("SystemRoot").ok_or("找不到 Windows 系统目录")?;
        let shell = Path::new(&shell).join("System32/WindowsPowerShell/v1.0/powershell.exe");
        let executable: Vec<u16> = shell.as_os_str().encode_wide().chain([0]).collect();
        let mut command: Vec<u16> = "\"powershell.exe\" -NoLogo"
            .encode_utf16()
            .chain([0])
            .collect();
        unsafe {
            let mut bytes = 0;
            InitializeProcThreadAttributeList(null_mut(), 2, 0, &mut bytes);
            // 按指针大小对齐 Windows 进程属性表。
            let mut buffer = vec![0usize; bytes.div_ceil(size_of::<usize>())];
            let attributes = buffer.as_mut_ptr().cast();
            if InitializeProcThreadAttributeList(attributes, 2, 0, &mut bytes) == 0 {
                return Err(last_error("初始化终端进程属性失败"));
            }
            let result = (|| {
                if UpdateProcThreadAttribute(
                    attributes,
                    0,
                    PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE as usize,
                    self.0 as _,
                    size_of::<HPCON>(),
                    null_mut(),
                    null(),
                ) == 0
                {
                    return Err(last_error("绑定伪控制台失败"));
                }
                let jobs = [job.as_raw_handle()];
                if UpdateProcThreadAttribute(
                    attributes,
                    0,
                    PROC_THREAD_ATTRIBUTE_JOB_LIST as usize,
                    jobs.as_ptr().cast(),
                    size_of_val(&jobs),
                    null_mut(),
                    null(),
                ) == 0
                {
                    return Err(last_error("绑定终端进程组失败"));
                }
                let mut startup: STARTUPINFOEXW = zeroed();
                startup.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
                // 父进程的标准流可能被重定向，清空句柄让 Shell 使用 ConPTY。
                startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
                startup.lpAttributeList = attributes;
                let mut process: PROCESS_INFORMATION = zeroed();
                if CreateProcessW(
                    executable.as_ptr(),
                    command.as_mut_ptr(),
                    null(),
                    null(),
                    0,
                    EXTENDED_STARTUPINFO_PRESENT,
                    null(),
                    cwd.as_ptr(),
                    &startup.StartupInfo,
                    &mut process,
                ) == 0
                {
                    return Err(last_error("启动内嵌 PowerShell 失败"));
                }
                drop(OwnedHandle::from_raw_handle(process.hThread));
                Ok(OwnedHandle::from_raw_handle(process.hProcess))
            })();
            DeleteProcThreadAttributeList(attributes);
            result
        }
    }
}

fn terminal_size(cols: u16, rows: u16) -> Result<COORD, String> {
    if cols == 0 || rows == 0 || cols > i16::MAX as u16 || rows > i16::MAX as u16 {
        return Err("终端行列数超出有效范围".into());
    }
    Ok(COORD {
        X: cols as i16,
        Y: rows as i16,
    })
}

fn pipe() -> Result<(File, File), String> {
    let (mut reader, mut writer) = (null_mut(), null_mut());
    if unsafe { CreatePipe(&mut reader, &mut writer, null(), 0) } == 0 {
        return Err(last_error("创建终端管道失败"));
    }
    Ok(unsafe { (File::from_raw_handle(reader), File::from_raw_handle(writer)) })
}

fn terminal_job() -> Result<OwnedHandle, String> {
    unsafe {
        let handle = CreateJobObjectW(null(), null());
        if handle.is_null() {
            return Err(last_error("创建终端进程组失败"));
        }
        let job = OwnedHandle::from_raw_handle(handle);
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = zeroed();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        if SetInformationJobObject(
            handle,
            JobObjectExtendedLimitInformation,
            (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            size_of_val(&limits) as u32,
        ) == 0
        {
            return Err(last_error("设置终端进程组失败"));
        }
        Ok(job)
    }
}

fn last_error(message: &str) -> String {
    format!("{message}（{}）", std::io::Error::last_os_error())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};
    use tauri::ipc::InvokeResponseBody;

    #[test]
    fn powershell_uses_project_directory_and_accepts_ctrl_c() {
        let manager = TerminalManager::default();
        let (sender, receiver) = mpsc::channel();
        let channel = Channel::new(move |message| {
            if let InvokeResponseBody::Raw(bytes) = message {
                let _ = sender.send(bytes);
            }
            Ok(())
        });
        let id = "powershell-test";
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        manager.open(id.into(), dir, 120, 24, channel).unwrap();
        let mut output = Vec::new();
        let mut wait_for = |marker: &str| {
            let deadline = Instant::now() + Duration::from_secs(15);
            loop {
                if let Some(position) = output
                    .windows(marker.len())
                    .position(|part| part == marker.as_bytes())
                {
                    output.drain(..position + marker.len());
                    break;
                }
                let chunk = receiver
                    .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                    .unwrap_or_else(|_| {
                        panic!(
                            "未收到 {marker}，终端输出：{}",
                            String::from_utf8_lossy(&output)
                        )
                    });
                assert!(!chunk.is_empty(), "终端提前退出");
                output.extend(chunk);
            }
        };

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            wait_for("PS ");
            manager.resize(id, 132, 28).unwrap();
            manager
                .write(
                    id,
                    "Write-Output ('目录=' + (Get-Location).Path); Start-Sleep -Seconds 60\r"
                        .into(),
                )
                .unwrap();
            wait_for(&format!("目录={}", dir.display()));
            manager.write(id, "\u{3}".into()).unwrap();
            wait_for("PS ");
            manager
                .write(id, "Write-Output ('TERMINAL-' + 'RESUMED')\r".into())
                .unwrap();
            wait_for("TERMINAL-RESUMED");
            manager.write(id, "exit\r".into()).unwrap();
            while !receiver
                .recv_timeout(Duration::from_secs(10))
                .unwrap()
                .is_empty()
            {}
            assert!(manager.write(id, "ignored".into()).is_err());
        }));
        manager.close_all();
        result.unwrap();
    }
}
