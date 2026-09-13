//! Classify and plan cleanup for `dsh web` processes.
//!
//! Agent Dock children (bootstrap cmdline contains `AD_DSH_BIN`) may be reaped
//! when their owner is gone. A user-started `dsh web` is never killed; opening
//! then fails with a conflict message.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DshProcKind {
    AgentDock,
    Foreign,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proc {
    pub pid: u32,
    pub parent_pid: u32,
    pub command: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictReason {
    Foreign,
    OtherDock,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conflict {
    pub pid: u32,
    pub reason: ConflictReason,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Plan {
    pub reap: Vec<u32>,
    pub conflicts: Vec<Conflict>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PidRecord {
    pub pid: u32,
    pub owner_pid: u32,
}

pub const CONFLICT_MARK: &str = "DeepSeek Web 进程冲突";

pub fn classify_command(command: &str) -> Option<DshProcKind> {
    if command.contains("AD_DSH_BIN") {
        return Some(DshProcKind::AgentDock);
    }
    if is_foreign_dsh_web(command) {
        return Some(DshProcKind::Foreign);
    }
    None
}

fn is_foreign_dsh_web(command: &str) -> bool {
    let normalized = command.replace('\\', "/").to_ascii_lowercase();
    if !has_web_arg(&normalized) {
        return false;
    }
    if normalized.contains("@deepseek-ai/dsh") || normalized.contains("/dsh/lib/bin.js") {
        return true;
    }
    launcher_followed_by_web(&normalized)
}

fn has_web_arg(command: &str) -> bool {
    command.split_whitespace().any(|token| {
        let token = token.trim_matches('"');
        token == "web" || token.starts_with("web-")
    })
}

fn launcher_followed_by_web(command: &str) -> bool {
    let tokens: Vec<&str> = command.split_whitespace().collect();
    tokens.windows(2).any(|pair| {
        let exe = pair[0]
            .trim_matches('"')
            .rsplit('/')
            .next()
            .unwrap_or(pair[0]);
        matches!(exe, "dsh" | "dsh.cmd" | "dsh.exe" | "dsh.ps1") && pair[1].trim_matches('"') == "web"
    })
}

pub fn plan(procs: &[Proc], our_pid: u32, live_child_pids: &[u32], running: &[u32]) -> Plan {
    let mut next = Plan::default();
    for proc in procs {
        let Some(kind) = classify_command(&proc.command) else {
            continue;
        };
        if live_child_pids.contains(&proc.pid) {
            continue;
        }
        match kind {
            DshProcKind::AgentDock => {
                let parent_alive = running.contains(&proc.parent_pid);
                if proc.parent_pid == our_pid || !parent_alive {
                    next.reap.push(proc.pid);
                } else {
                    next.conflicts.push(Conflict {
                        pid: proc.pid,
                        reason: ConflictReason::OtherDock,
                    });
                }
            }
            DshProcKind::Foreign => next.conflicts.push(Conflict {
                pid: proc.pid,
                reason: ConflictReason::Foreign,
            }),
        }
    }
    next
}

pub fn stale_owned_pids(records: &[PidRecord], running: &[u32]) -> Vec<u32> {
    records
        .iter()
        .filter(|record| running.contains(&record.pid) && !running.contains(&record.owner_pid))
        .map(|record| record.pid)
        .collect()
}

#[cfg(test)]
pub fn keep_live_records(records: &[PidRecord], running: &[u32]) -> Vec<PidRecord> {
    records
        .iter()
        .filter(|record| running.contains(&record.pid))
        .cloned()
        .collect()
}

pub fn parse_proc_table(text: &str) -> Vec<Proc> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                return None;
            }
            let mut parts = line.splitn(3, '\t');
            let pid = parts.next()?.trim().parse().ok()?;
            let parent_pid = parts.next()?.trim().parse().ok()?;
            let command = parts.next().unwrap_or("").to_string();
            classify_command(&command)?;
            Some(Proc {
                pid,
                parent_pid,
                command,
            })
        })
        .collect()
}

pub fn conflict_text(conflicts: &[Conflict]) -> Option<String> {
    let first = conflicts.first()?;
    let pids = conflicts
        .iter()
        .map(|item| item.pid.to_string())
        .collect::<Vec<_>>()
        .join("、");
    Some(match first.reason {
        ConflictReason::Foreign => format!(
            "{CONFLICT_MARK}：系统里已有你自己启动的 dsh web（PID {pids}）。请先在那个窗口或终端里关掉它，再从 Agent Dock 打开。Agent Dock 不会结束你自己开的进程。"
        ),
        ConflictReason::OtherDock => format!(
            "{CONFLICT_MARK}：另一个 Agent Dock 窗口正在使用 DeepSeek Web（PID {pids}）。请先关掉那个窗口。"
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOOTSTRAP: &str = r#"C:\pi-node\node.exe --input-type=module -e "
import { pathToFileURL } from 'node:url'
const bin = process.env.AD_DSH_BIN
const args = JSON.parse(process.env.AD_DSH_ARGS || '[]')
""#;

    fn proc(pid: u32, parent: u32, command: &str) -> Proc {
        Proc {
            pid,
            parent_pid: parent,
            command: command.to_string(),
        }
    }

    #[test]
    fn parse_proc_table_keeps_dsh_rows() {
        let table = "10\t1\tnode -e AD_DSH_BIN\n11\t1\tnode vite.js\n20\t8\tdsh.cmd web\n";
        let procs = parse_proc_table(table);
        assert_eq!(procs.len(), 2);
        assert_eq!(procs[0].pid, 10);
        assert_eq!(procs[1].pid, 20);
    }

    #[test]
    fn bootstrap_cmdline_is_ours() {
        assert_eq!(classify_command(BOOTSTRAP), Some(DshProcKind::AgentDock));
    }

    #[test]
    fn official_bin_js_web_is_foreign() {
        let cmd = r#"C:\nodejs\node.exe C:\Users\PS\AppData\Roaming\npm\node_modules\@deepseek-ai\dsh\lib\bin.js web --no-open --port 3080"#;
        assert_eq!(classify_command(cmd), Some(DshProcKind::Foreign));
    }

    #[test]
    fn dsh_cmd_web_is_foreign() {
        assert_eq!(
            classify_command(r#"C:\Users\PS\AppData\Roaming\npm\dsh.cmd web"#),
            Some(DshProcKind::Foreign)
        );
        assert_eq!(
            classify_command(r#""C:\npm\dsh.exe" web --no-open"#),
            Some(DshProcKind::Foreign)
        );
    }

    #[test]
    fn unrelated_node_is_ignored() {
        assert_eq!(classify_command("node vite.js"), None);
        assert_eq!(
            classify_command("HTTP_PROXY=http://127.0.0.1:7890 node server.js"),
            None
        );
        assert_eq!(
            classify_command(r#"C:\nodejs\node.exe C:\Users\PS\AppData\Roaming\npm\node_modules\@deepseek-ai\dsh\lib\bin.js headless"#),
            None
        );
    }

    #[test]
    fn live_child_of_this_hub_is_left_alone() {
        let procs = [proc(10, 1, BOOTSTRAP)];
        let plan = plan(&procs, 1, &[10], &[1, 10]);
        assert!(plan.reap.is_empty());
        assert!(plan.conflicts.is_empty());
    }

    #[test]
    fn orphan_agent_dock_child_is_reaped() {
        let procs = [proc(10, 99, BOOTSTRAP)];
        let plan = plan(&procs, 1, &[], &[1, 10]);
        assert_eq!(plan.reap, vec![10]);
        assert!(plan.conflicts.is_empty());
    }

    #[test]
    fn stale_child_of_this_process_not_in_hub_is_reaped() {
        let procs = [proc(10, 1, BOOTSTRAP)];
        let plan = plan(&procs, 1, &[], &[1, 10]);
        assert_eq!(plan.reap, vec![10]);
        assert!(plan.conflicts.is_empty());
    }

    #[test]
    fn user_started_dsh_web_is_conflict_not_reap() {
        let cmd = r#"C:\nodejs\node.exe C:\npm\node_modules\@deepseek-ai\dsh\lib\bin.js web"#;
        let procs = [proc(20, 8, cmd)];
        let plan = plan(&procs, 1, &[], &[1, 8, 20]);
        assert!(plan.reap.is_empty());
        assert_eq!(
            plan.conflicts,
            vec![Conflict {
                pid: 20,
                reason: ConflictReason::Foreign
            }]
        );
    }

    #[test]
    fn other_agent_dock_window_is_conflict_not_reap() {
        let procs = [proc(30, 7, BOOTSTRAP)];
        let plan = plan(&procs, 1, &[], &[1, 7, 30]);
        assert!(plan.reap.is_empty());
        assert_eq!(
            plan.conflicts,
            vec![Conflict {
                pid: 30,
                reason: ConflictReason::OtherDock
            }]
        );
    }

    #[test]
    fn reaps_orphans_and_still_reports_foreign_conflict() {
        let official = r#"dsh.cmd web"#;
        let procs = [proc(10, 99, BOOTSTRAP), proc(20, 8, official)];
        let plan = plan(&procs, 1, &[], &[1, 8, 10, 20]);
        assert_eq!(plan.reap, vec![10]);
        assert_eq!(plan.conflicts.len(), 1);
        assert_eq!(plan.conflicts[0].pid, 20);
        assert_eq!(plan.conflicts[0].reason, ConflictReason::Foreign);
    }

    #[test]
    fn lock_file_reaps_when_owner_is_dead() {
        let records = [
            PidRecord {
                pid: 10,
                owner_pid: 99,
            },
            PidRecord {
                pid: 11,
                owner_pid: 1,
            },
        ];
        assert_eq!(stale_owned_pids(&records, &[1, 10, 11]), vec![10]);
        assert_eq!(
            keep_live_records(&records, &[1, 11]),
            vec![PidRecord {
                pid: 11,
                owner_pid: 1
            }]
        );
    }

    #[test]
    fn conflict_text_tells_user_we_will_not_kill_their_process() {
        let text = conflict_text(&[Conflict {
            pid: 20,
            reason: ConflictReason::Foreign,
        }])
        .expect("message");
        assert!(text.contains(CONFLICT_MARK));
        assert!(text.contains("20"));
        assert!(text.contains("自己"));
        assert!(text.contains("不会"));
    }

    #[test]
    fn conflict_text_for_other_dock_asks_to_close_that_window() {
        let text = conflict_text(&[Conflict {
            pid: 30,
            reason: ConflictReason::OtherDock,
        }])
        .expect("message");
        assert!(text.contains(CONFLICT_MARK));
        assert!(text.contains("30"));
        assert!(text.contains("另一个"));
    }
}
