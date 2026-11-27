//! Running the command a person typed after `!` in the composer.
//!
//! One shell, in the chat's folder, with nobody at its keyboard: what it
//! prints comes back as text (`domain::shell_tape` keeps it to size), and
//! a command that never ends is ended for it.

use crate::domain::shell_tape::Tape;
use std::io::Read;
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc::RecvTimeoutError;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How often a running command is looked at.
const TICK: Duration = Duration::from_millis(20);

/// How long the output stays open after the shell itself is gone. A job
/// left in the background holds it open for as long as it lives, and the
/// answer is the shell's, not the job's.
const LINGER: Duration = Duration::from_millis(250);

/// How long a command asked to end gets to do it before it is killed.
const GRACE: Duration = Duration::from_secs(2);

/// A command still running. `pid` is what `stop` takes.
pub struct Running {
    pid: u32,
    child: Child,
    out: std::io::PipeReader,
}

/// How a command ended.
#[derive(Debug)]
pub struct Ended {
    /// Standard output and error, in the order they were written.
    pub output: String,
    /// Bytes of output left out of the middle.
    pub omitted: usize,
    /// The exit code; none when a signal ended it.
    pub code: Option<i32>,
    /// It ran past its budget and was ended.
    pub timed_out: bool,
}

/// Start `command` in `shell`, in `cwd`.
///
/// Nobody is at its keyboard: input is closed, so a command that asks a
/// question gets no answer instead of waiting for one, and pagers are
/// off. Output and errors share one pipe, which is what keeps their
/// order. The command leads a process group of its own, so ending it
/// ends what it started.
pub fn start(shell: &str, command: &str, cwd: &Path) -> std::io::Result<Running> {
    let (out, writer) = std::io::pipe()?;
    let mut cmd = Command::new(shell);
    cmd.arg("-c")
        .arg(command)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(writer.try_clone()?)
        .stderr(writer)
        .env("PAGER", "cat")
        .env("GIT_PAGER", "cat");
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let child = cmd.spawn()?;
    // The command holds the writing ends: while it lives, the reader
    // would never see the output end.
    drop(cmd);
    Ok(Running {
        pid: child.id(),
        child,
        out,
    })
}

/// End a running command and everything it started.
pub fn stop(pid: u32) {
    signal_tree(pid, false);
}

#[cfg(unix)]
fn signal_tree(pid: u32, hard: bool) {
    // A negative pid is the process group the command leads.
    let _ = Command::new("kill")
        .args([if hard { "-KILL" } else { "-TERM" }, "--", &format!("-{pid}")])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

#[cfg(not(unix))]
fn signal_tree(pid: u32, _hard: bool) {
    let _ = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

impl Running {
    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// Wait for the end, at most `budget`, keeping at most `max` bytes of
    /// what was printed. Nothing the command started outlives the wait.
    pub fn wait(self, budget: Duration, max: usize) -> Ended {
        let Running { pid, mut child, mut out } = self;
        let tape = Arc::new(Mutex::new(Tape::new(max)));
        let (ended, output_ended) = std::sync::mpsc::channel::<()>();
        let reader_tape = tape.clone();
        std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            while let Ok(n @ 1..) = out.read(&mut buf) {
                reader_tape.lock().unwrap().push(&buf[..n]);
            }
            let _ = ended.send(());
        });
        // True once the output closed (or its reader is gone).
        let closed = |wait: Duration| !matches!(output_ended.recv_timeout(wait), Err(RecvTimeoutError::Timeout));

        let deadline = Instant::now() + budget;
        let mut status: Option<(ExitStatus, Instant)> = None;
        let mut eof = false;
        let mut timed_out = false;
        loop {
            if status.is_none() {
                if let Ok(Some(done)) = child.try_wait() {
                    status = Some((done, Instant::now()));
                }
            }
            if let Some((_, at)) = status {
                if eof || at.elapsed() >= LINGER {
                    break;
                }
            }
            if Instant::now() >= deadline {
                timed_out = status.is_none();
                break;
            }
            if eof {
                std::thread::sleep(TICK);
            } else {
                eof = closed(TICK);
            }
        }

        if status.is_none() || !eof {
            signal_tree(pid, false);
            let until = Instant::now() + GRACE;
            while status.is_none() && Instant::now() < until {
                match child.try_wait() {
                    Ok(Some(done)) => status = Some((done, Instant::now())),
                    _ => std::thread::sleep(TICK),
                }
            }
            if status.is_none() {
                signal_tree(pid, true);
                let _ = child.kill();
                status = child.wait().ok().map(|done| (done, Instant::now()));
            }
            if !eof {
                // What it printed on the way out still counts.
                closed(LINGER);
            }
        }

        let kept = std::mem::replace(&mut *tape.lock().unwrap(), Tape::new(0)).finish();
        Ended {
            output: kept.text,
            omitted: kept.omitted,
            code: status.and_then(|(done, _)| done.code()),
            timed_out,
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::time::Instant;

    const LONG: Duration = Duration::from_secs(20);

    fn run_in(dir: &Path, command: &str) -> Ended {
        start("/bin/sh", command, dir).unwrap().wait(LONG, 10_000)
    }

    fn run(command: &str) -> Ended {
        run_in(tempfile::tempdir().unwrap().path(), command)
    }

    #[test]
    fn the_command_runs_in_the_folder_asked() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("notes.txt"), "").unwrap();
        let out = run_in(dir.path(), "ls");
        assert_eq!(out.output, "notes.txt\n");
        assert_eq!(out.code, Some(0));
        assert!(!out.timed_out);
    }

    #[test]
    fn output_and_errors_come_back_in_the_order_written() {
        let out = run("echo one; echo two >&2; echo three");
        assert_eq!(out.output, "one\ntwo\nthree\n");
    }

    #[test]
    fn a_failing_command_reports_its_code_with_what_it_printed() {
        let out = run("echo broken >&2; exit 3");
        assert_eq!(out.output, "broken\n");
        assert_eq!(out.code, Some(3));
    }

    #[test]
    fn a_command_that_reads_input_finds_none_and_ends() {
        let began = Instant::now();
        let out = run("cat; echo done");
        assert_eq!(out.output, "done\n");
        assert!(began.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn a_command_past_its_budget_is_ended_and_says_so() {
        let dir = tempfile::tempdir().unwrap();
        let began = Instant::now();
        let out = start("/bin/sh", "echo started; sleep 30; echo never", dir.path())
            .unwrap()
            .wait(Duration::from_millis(400), 10_000);
        assert!(out.timed_out);
        assert_eq!(out.output, "started\n");
        assert!(began.elapsed() < Duration::from_secs(10));
    }

    #[test]
    fn ending_it_ends_what_it_started_too() {
        let dir = tempfile::tempdir().unwrap();
        // The shell is not the one sleeping: its child is.
        let out = start("/bin/sh", "sleep 31 & echo $! > child.pid; wait", dir.path())
            .unwrap()
            .wait(Duration::from_millis(600), 10_000);
        assert!(out.timed_out);
        let child = std::fs::read_to_string(dir.path().join("child.pid")).unwrap();
        let alive = || {
            std::process::Command::new("kill")
                .args(["-0", child.trim()])
                .stderr(std::process::Stdio::null())
                .status()
                .unwrap()
                .success()
        };
        // A process just ended still answers until it is reaped.
        let until = Instant::now() + Duration::from_secs(3);
        while alive() && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(!alive(), "the child {child} outlived the command");
    }

    #[test]
    fn stopping_ends_a_command_before_its_budget() {
        let dir = tempfile::tempdir().unwrap();
        let running = start("/bin/sh", "echo started; sleep 30", dir.path()).unwrap();
        let pid = running.pid();
        let began = Instant::now();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            stop(pid);
        });
        let out = running.wait(LONG, 10_000);
        assert!(began.elapsed() < Duration::from_secs(10));
        assert!(!out.timed_out);
        assert_eq!(out.code, None);
        assert_eq!(out.output, "started\n");
    }

    #[test]
    fn a_job_left_in_the_background_does_not_hold_the_answer() {
        let began = Instant::now();
        let out = run("sleep 30 & echo back");
        assert_eq!(out.output, "back\n");
        assert_eq!(out.code, Some(0));
        assert!(began.elapsed() < Duration::from_secs(10));
    }

    #[test]
    fn a_flood_of_output_is_kept_to_size() {
        let dir = tempfile::tempdir().unwrap();
        let out = start("/bin/sh", "yes line | head -n 50000", dir.path())
            .unwrap()
            .wait(LONG, 1_000);
        assert!(out.output.len() < 1_100);
        assert_eq!(out.omitted, 50_000 * 5 - 1_000);
        assert!(out.output.starts_with("line\n"));
        assert!(out.output.ends_with("line\n"));
    }

    #[test]
    fn a_command_that_is_no_command_says_so_instead_of_failing() {
        let out = run("definitely-not-a-command-xyz");
        assert_ne!(out.code, Some(0));
        assert!(out.output.contains("not found"));
    }

    #[test]
    fn a_folder_that_is_gone_is_an_error_not_a_run_somewhere_else() {
        let dir = tempfile::tempdir().unwrap();
        let gone = dir.path().join("gone");
        assert!(start("/bin/sh", "pwd", &gone).is_err());
    }
}
