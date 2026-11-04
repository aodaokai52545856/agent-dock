//! Installers belong to the application, never to a dialog or an IPC request.
use crate::{
    app_update,
    state::AppSettings,
    tools::{self, ToolId},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    panic::{catch_unwind, AssertUnwindSafe},
    sync::{Arc, Mutex},
};
use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Action {
    Upgrade,
    Uninstall,
    Download,
    Apply,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    Queued,
    Running,
    Succeeded,
    Failed,
    Ready,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: String,
    pub batch_id: String,
    pub target: String,
    pub action: Action,
    pub status: Status,
    pub stage: String,
    pub log: String,
    pub local_version: String,
    pub downloaded: u64,
    pub total: Option<u64>,
    pub started_at: i64,
    pub finished_at: Option<i64>,
    pub restart: bool,
}

#[derive(Clone, Default, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub revision: u64,
    pub batch_id: String,
    pub ready_version: Option<String>,
    pub ready_tools: std::collections::BTreeMap<String, String>,
    pub tasks: Vec<Task>,
}
impl Snapshot {
    pub fn running(&self) -> bool {
        self.tasks
            .iter()
            .any(|task| matches!(task.status, Status::Queued | Status::Running))
    }
}

pub struct Hub {
    state: Mutex<Snapshot>,
    prepared: Mutex<Option<app_update::PreparedUpdate>>,
    publish: Box<dyn Fn(Snapshot) + Send + Sync>,
}

struct Outcome {
    status: Status,
    log: String,
    version: String,
    restart: bool,
}

impl Hub {
    pub fn new(app: AppHandle) -> Arc<Self> {
        Self::with_publisher(move |snapshot| {
            let _ = app.emit("version-tasks", snapshot);
        })
    }
    fn with_publisher(publish: impl Fn(Snapshot) + Send + Sync + 'static) -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(Snapshot::default()),
            prepared: Mutex::new(None),
            publish: Box::new(publish),
        })
    }
    pub fn snapshot(&self) -> Snapshot {
        let mut snapshot=self.state
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .clone();
        if let Some(p)=tools::omp_update::pending(){snapshot.ready_tools.insert("omp".into(),p.version);}
        snapshot
    }
    pub fn running(&self) -> bool {
        self.snapshot().running()
    }

    pub fn start(
        self: &Arc<Self>,
        app: AppHandle,
        settings: AppSettings,
        targets: Vec<String>,
        action: Action,
        proxy: Option<String>,
    ) -> Result<Snapshot, String> {
        let worker_app = app.clone();
        self.start_with(targets, action, move |task, hub| {
            hub.execute(&worker_app, &settings, proxy.as_deref(), task)
        })
    }

    fn start_with(
        self: &Arc<Self>,
        targets: Vec<String>,
        action: Action,
        execute: impl Fn(&Task, &Hub) -> Result<Outcome, String> + Send + 'static,
    ) -> Result<Snapshot, String> {
        if targets.is_empty() || targets.len() > tools::update::version_tools(None).len() + 1 {
            return Err(format!("请选择 1–{} 项升级任务。",tools::update::version_tools(None).len()+1));
        }
        let mut seen = HashSet::new();
        for target in &targets {
            if !seen.insert(target) {
                return Err("同一工具不能重复加入任务。".into());
            }
            match (target.as_str(), action) {
                ("omp", Action::Apply) if targets.len() == 1 && tools::omp_update::pending().is_some() => {}
                ("app", Action::Download | Action::Apply) if targets.len() == 1 => {}
                (_, Action::Upgrade | Action::Uninstall) if ToolId::parse(target).is_some() => {}
                _ => return Err("升级任务类型无效。".into()),
            }
        }
        if action == Action::Apply && targets[0] == "app"
            && self
                .prepared
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .is_none()
        {
            return Err("请先下载客户端更新。".into());
        }
        let jobs;
        let initial;
        {
            let mut state = self.state.lock().unwrap_or_else(|err| err.into_inner());
            if state.running() {
                return Err("已有升级任务正在后台运行，请等待完成。".into());
            }
            if action == Action::Download {
                *self.prepared.lock().unwrap_or_else(|err| err.into_inner()) = None;
                state.ready_version = None;
            }
            let batch = Uuid::new_v4().to_string();
            jobs = targets
                .into_iter()
                .map(|target| Task {
                    id: Uuid::new_v4().to_string(),
                    batch_id: batch.clone(),
                    target,
                    action,
                    status: Status::Queued,
                    stage: "等待执行".into(),
                    log: String::new(),
                    local_version: String::new(),
                    downloaded: 0,
                    total: None,
                    started_at: 0,
                    finished_at: None,
                    restart: false,
                })
                .collect::<Vec<_>>();
            let remove = state.tasks.len().saturating_sub(40 - jobs.len());
            state.tasks.drain(..remove);
            state.tasks.extend(jobs.clone());
            state.batch_id = batch;
            state.revision += 1;
            initial = state.clone();
        }
        (self.publish)(initial.clone());
        let hub = Arc::clone(self);
        let failed_jobs = jobs.clone();
        if let Err(err) = std::thread::Builder::new()
            .name("version-updates".into())
            .spawn(move || {
                for task in jobs {
                    hub.change(&task.id, |row| {
                        row.status = Status::Running;
                        row.started_at = chrono::Utc::now().timestamp_millis();
                        row.stage = match row.action {
                            Action::Upgrade => "正在安装或升级",
                            Action::Uninstall => "正在卸载",
                            Action::Download => "检查客户端更新",
                            Action::Apply => "启动安装程序",
                        }
                        .into();
                    });
                    let result = catch_unwind(AssertUnwindSafe(|| execute(&task, &hub)))
                        .unwrap_or_else(|_| Err("升级任务意外结束，请查看日志后重试。".into()));
                    hub.change(&task.id, |row| {
                        row.finished_at = Some(chrono::Utc::now().timestamp_millis());
                        match result {
                            Ok(outcome) => {
                                row.status = outcome.status;
                                row.log = outcome.log;
                                row.local_version = outcome.version;
                                row.restart = outcome.restart;
                                row.stage = match row.status {
                                    Status::Ready if row.target=="omp" => "更新已准备好，等待应用",
                                    Status::Ready => "下载完成，等待安装",
                                    Status::Failed => "执行失败",
                                    _ => "已完成",
                                }
                                .into();
                            }
                            Err(err) => {
                                row.status = Status::Failed;
                                row.stage = "执行失败".into();
                                row.log = err;
                            }
                        }
                    });
                }
            })
        {
            for task in failed_jobs {
                self.change(&task.id, |row| {
                    row.status = Status::Failed;
                    row.stage = "无法启动任务".into();
                    row.log = err.to_string();
                    row.finished_at = Some(chrono::Utc::now().timestamp_millis());
                });
            }
            return Err(format!("无法启动后台升级任务：{err}"));
        }
        Ok(initial)
    }

    fn change(&self, id: &str, update: impl FnOnce(&mut Task)) {
        let next = {
            let mut state = self.state.lock().unwrap_or_else(|err| err.into_inner());
            let mut ready = None;
            if let Some(row) = state.tasks.iter_mut().find(|row| row.id == id) {
                update(row);
                if row.target == "app" && row.action == Action::Download && row.status == Status::Ready {
                    ready = Some(Some(row.local_version.clone()));
                } else if row.target == "app" && row.action == Action::Apply && row.status == Status::Succeeded {
                    ready = Some(None);
                }
            }
            if let Some(version) = ready {
                state.ready_version = version;
            }
            if let Some(p) = tools::omp_update::pending() { state.ready_tools.insert("omp".into(), p.version); }
            else { state.ready_tools.remove("omp"); }
            state.revision += 1;
            state.clone()
        };
        (self.publish)(next);
    }

    fn execute(
        &self,
        app: &AppHandle,
        settings: &AppSettings,
        proxy: Option<&str>,
        task: &Task,
    ) -> Result<Outcome, String> {
        if task.target == "omp" && matches!(task.action, Action::Upgrade | Action::Apply) {
            let hub = crate::omp_rpc::global();
            let prepared = if task.action == Action::Apply {
                tools::omp_update::pending().ok_or("OMP 更新已失效，请重新检测更新")?
            } else {
                tools::omp_update::prepare(settings, proxy, &mut |bytes,total| self.change(&task.id, |row| {
                    row.stage = "正在下载 OMP".into(); row.downloaded=bytes; row.total=total;
                }))?
            };
            let terminals=app.state::<Arc<crate::pty::PtyHub>>();
            let cli_sessions=terminals.list().into_iter().filter(|p|p.tool_id==ToolId::Omp).collect::<Vec<_>>();
            if task.action == Action::Upgrade && (!hub.list().is_empty() || !cli_sessions.is_empty()) {
                return Ok(Outcome { status:Status::Ready,version:prepared.version,restart:false,
                    log:"OMP 更新已准备好。点击应用更新后将关闭并恢复 OMP 会话，不会自动重发任务。".into() });
            }
            let active=hub.list();
            for cli in &cli_sessions {terminals.kill(&cli.pty_id)?;}
            // Acknowledged application: close only OMP, then restore saved session identities.
            for live in &active { if let Some(id)=&live.session_id { let _=hub.request(&live.pty_id,serde_json::json!({"type":"abort"}));
                self.change(&task.id, |row|row.stage=format!("保存会话 {id}")); }
                hub.close(&live.pty_id)?;
            }
            let applied=tools::omp_update::apply(&prepared,proxy);
            let state=crate::state::load_state(app)?;
            let mut restore_errors=Vec::new();
            for live in active { if live.session_id.is_some(){ if let Err(e)=hub.open(app.clone(),&state,&live.project_id,live.session_id.clone(),live.account.map(|a|a.id),live.title) { restore_errors.push(e); }}}
            for live in cli_sessions {
                if let Some(id) = &live.session_id {
                    let restored = (|| -> Result<(), String> {
                        let project=crate::state::find_project(&state,&live.project_id)?;
                        let profile=tools::omp::split_id(id)?.0;
                        let args=tools::omp::native_args(&project.path,Some(id),&profile)?;
                        terminals.open(app.clone(),&state,&live.project_id,ToolId::Omp,Some(id.clone()),live.title,120,32,Some(&state.settings.ui_theme),Some(args))?;
                        Ok(())
                    })();
                    if let Err(error)=restored {restore_errors.push(error);}
                }
            }
            let mut log=applied?;
            if !restore_errors.is_empty(){log.push_str(&format!("\n部分会话需手动恢复：{}",restore_errors.join("；")));}
            return Ok(Outcome{status:Status::Succeeded,log,version:prepared.version,restart:false});
        }
        match task.action {
            Action::Upgrade | Action::Uninstall => {
                let tool = ToolId::parse(&task.target).ok_or("无效的工具类型")?;
                if tool==ToolId::Omp && app.state::<Arc<crate::pty::PtyHub>>().list().iter().any(|p|p.tool_id==ToolId::Omp){return Err("请先关闭 OMP 原生会话和登录终端再卸载".into());}
                let result = if task.action == Action::Uninstall {
                    tools::update::uninstall_tool(tool, settings)
                } else {
                    tools::update::upgrade_tool(tool, settings, proxy)
                };
                Ok(Outcome {
                    status: if result.ok {
                        Status::Succeeded
                    } else {
                        Status::Failed
                    },
                    log: result.log,
                    version: result.local_version,
                    restart: false,
                })
            }
            Action::Download => {
                let prepared = app_update::prepare_update(proxy, &mut |stage, bytes, total| {
                    self.change(&task.id, |row| {
                        row.stage = stage.into();
                        row.downloaded = bytes;
                        row.total = total;
                    })
                })?;
                let version = prepared
                    .as_ref()
                    .map(|p| p.version.clone())
                    .unwrap_or_default();
                let ready = prepared.is_some();
                *self.prepared.lock().unwrap_or_else(|err| err.into_inner()) = prepared;
                Ok(Outcome {
                    status: if ready {
                        Status::Ready
                    } else {
                        Status::Succeeded
                    },
                    version,
                    restart: false,
                    log: if ready {
                        "安装包已下载并校验完整性。准备好后点击安装，下载完成不会自动关闭客户端。"
                    } else {
                        "当前客户端已经是最新版本。"
                    }
                    .into(),
                })
            }
            Action::Apply => {
                let mut prepared = self.prepared.lock().unwrap_or_else(|err| err.into_inner());
                let package = prepared.as_ref().ok_or("安装包已失效，请重新下载更新。")?;
                let result = app_update::apply_prepared(package)?;
                let version = package.version.clone();
                if result.ok {
                    *prepared = None;
                }
                if result.restart {
                    app_update::schedule_exit(app.clone());
                }
                Ok(Outcome {
                    status: if result.ok {
                        Status::Succeeded
                    } else {
                        Status::Failed
                    },
                    log: result.log,
                    version,
                    restart: result.restart,
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::mpsc, time::Duration};
    fn done() -> Result<Outcome, String> {
        Ok(Outcome {
            status: Status::Succeeded,
            log: "done".into(),
            version: "2.0".into(),
            restart: false,
        })
    }
    #[test]
    fn every_registered_tool_including_omp_fits_one_batch() {
        let hub=Hub::with_publisher(|_|{});
        let targets=tools::update::version_tools(None).into_iter().map(|t|t.as_str().to_string()).collect::<Vec<_>>();
        assert!(targets.len()>6);
        hub.start_with(targets,Action::Upgrade,|_,_|done()).unwrap();
        let final_state=wait(&hub);
        assert_eq!(final_state.tasks.len(),tools::update::version_tools(None).len());
        assert!(final_state.tasks.iter().all(|t|t.status==Status::Succeeded));
    }
    fn wait(hub: &Hub) -> Snapshot {
        for _ in 0..200 {
            let state = hub.snapshot();
            if !state.running() {
                return state;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        panic!("worker did not finish");
    }
    #[test]
    fn jobs_survive_request_drop_and_reject_duplicate_batches() {
        let hub = Hub::with_publisher(|_| {});
        let (tx, rx) = mpsc::channel();
        let request = hub
            .start_with(
                vec!["claude".into(), "pi".into()],
                Action::Upgrade,
                move |_, _| {
                    rx.recv().unwrap();
                    done()
                },
            )
            .unwrap();
        drop(request); // There is no dialog or retained IPC future driving this queue.
        assert!(hub
            .start_with(vec!["kimi".into()], Action::Upgrade, |_, _| done())
            .is_err());
        tx.send(()).unwrap();
        tx.send(()).unwrap();
        let final_state = wait(&hub);
        assert!(final_state
            .tasks
            .iter()
            .all(|row| row.status == Status::Succeeded));
        assert_eq!(final_state.tasks.len(), 2);
        assert!(final_state.revision >= 5);
    }
    #[test]
    fn failures_and_panics_do_not_cancel_remaining_tools() {
        let hub = Hub::with_publisher(|_| {});
        hub.start_with(
            vec!["claude".into(), "kimi".into(), "pi".into()],
            Action::Upgrade,
            |task, _| match task.target.as_str() {
                "claude" => Err("install failed".into()),
                "kimi" => panic!("installer panic"),
                _ => done(),
            },
        )
        .unwrap();
        let state = wait(&hub);
        assert_eq!(
            state.tasks.iter().map(|row| row.status).collect::<Vec<_>>(),
            vec![Status::Failed, Status::Failed, Status::Succeeded]
        );
        assert!(state.tasks.iter().all(|row| row.finished_at.is_some()));
    }
    #[test]
    fn validates_targets_and_requires_download_before_apply() {
        let hub = Hub::with_publisher(|_| {});
        for (targets, action) in [
            (vec![], Action::Upgrade),
            (vec!["claude".into(), "claude".into()], Action::Upgrade),
            (vec!["unknown".into()], Action::Upgrade),
            (vec!["app".into()], Action::Uninstall),
            (vec!["app".into()], Action::Apply),
        ] {
            assert!(hub.start_with(targets, action, |_, _| done()).is_err());
        }
        assert_eq!(hub.snapshot().revision, 0);
    }

    #[test]
    fn prepared_version_survives_history_retention() {
        let hub = Hub::with_publisher(|_| {});
        hub.start_with(vec!["app".into()], Action::Download, |_, _| {
            Ok(Outcome {
                status: Status::Ready,
                version: "0.1.20".into(),
                log: "downloaded".into(),
                restart: false,
            })
        })
        .unwrap();
        wait(&hub);
        for _ in 0..41 {
            hub.start_with(vec!["pi".into()], Action::Upgrade, |_, _| done())
                .unwrap();
            wait(&hub);
        }
        let state = hub.snapshot();
        assert_eq!(state.tasks.len(), 40);
        assert!(!state.tasks.iter().any(|task| task.target == "app"));
        assert_eq!(state.ready_version.as_deref(), Some("0.1.20"));
    }
}
