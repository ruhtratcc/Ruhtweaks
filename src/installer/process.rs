use crate::{Result, fail};
use std::{
    io::{BufRead, BufReader, Read},
    process::{Child, ExitStatus},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};
enum Event {
    Line(String),
    Eof,
    Exited(std::io::Result<ExitStatus>),
}
fn reader(input: impl Read + Send + 'static, sender: mpsc::SyncSender<Event>) {
    thread::spawn(move || {
        let mut input = BufReader::new(input);
        let mut line = Vec::with_capacity(2048);
        while let Ok(chunk) = input.fill_buf() {
            if chunk.is_empty() {
                if !line.is_empty() {
                    let _ = sender.send(Event::Line(String::from_utf8_lossy(&line).into()));
                }
                break;
            }
            let boundary = chunk.iter().position(|b| matches!(b, b'\n' | b'\r'));
            let used = boundary.map_or(chunk.len(), |position| position + 1);
            let payload = boundary.unwrap_or(used);
            let keep = payload.min(8192usize.saturating_sub(line.len()));
            line.extend_from_slice(&chunk[..keep]);
            input.consume(used);
            if boundary.is_some() {
                if !line.is_empty()
                    && sender
                        .send(Event::Line(String::from_utf8_lossy(&line).into()))
                        .is_err()
                {
                    return;
                }
                line.clear();
            }
        }
        let _ = sender.send(Event::Eof);
    });
}
pub fn collect(
    mut child: Child,
    install: bool,
    cancel: &AtomicBool,
    log: &mut dyn FnMut(String),
) -> Result<(i32, String)> {
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| fail("stdout indisponível"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| fail("stderr indisponível"))?;
    let (tx, rx) = mpsc::sync_channel(128);
    reader(stdout, tx.clone());
    reader(stderr, tx.clone());
    let mut query = if install {
        thread::spawn(move || {
            let _ = tx.send(Event::Exited(child.wait()));
        });
        None
    } else {
        drop(tx);
        Some(child)
    };
    let start = Instant::now();
    let mut exit = None;
    let mut eof = 0;
    let mut output = String::new();
    loop {
        // Installs block on pipe/process notifications. Only cancelable queries
        // use a bounded 200 ms timeout, and they cannot survive past 120 seconds.
        let event = if install {
            rx.recv().ok()
        } else {
            match rx.recv_timeout(Duration::from_millis(200)) {
                Ok(event) => Some(event),
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    thread::park_timeout(Duration::from_millis(200));
                    None
                }
                Err(mpsc::RecvTimeoutError::Timeout) => None,
            }
        };
        match event {
            Some(Event::Line(line)) => {
                let remaining = (128usize * 1024).saturating_sub(output.len());
                if remaining > 0 {
                    let mut keep = line.len().min(remaining.saturating_sub(1));
                    while !line.is_char_boundary(keep) {
                        keep -= 1;
                    }
                    output.push_str(&line[..keep]);
                    output.push('\n');
                }
                log(line);
            }
            Some(Event::Eof) => eof += 1,
            Some(Event::Exited(status)) => exit = Some(status?),
            None if install => {
                return Err(fail(
                    "Monitoramento do processo interrompido; revalide antes de repetir",
                ));
            }
            None => {}
        }
        if let Some(child) = query.as_mut() {
            if let Some(status) = child.try_wait()? {
                exit = Some(status);
            }
            if cancel.load(Ordering::Relaxed) || start.elapsed() > Duration::from_secs(120) {
                if exit.is_none() {
                    let _ = child.kill();
                    let _ = child.wait();
                }
                return Err(fail("Consulta WinGet cancelada ou excedeu 120 segundos"));
            }
        }
        if eof == 2
            && let Some(status) = exit
        {
            return Ok((status.code().unwrap_or(-1), output));
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::process::{Command, Stdio};
    #[test]
    fn fixture() {
        if std::env::var_os("RUHTWEAKS_PIPE_FIXTURE").is_some() {
            println!("{}", "x".repeat(512 * 1024));
            println!("PIPE-OK");
            eprintln!("PIPE-ERR");
        }
    }
    #[test]
    fn process_notifications_keep_exit_and_bounded_output() {
        let child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "installer::process::tests::fixture",
                "--nocapture",
            ])
            .env("RUHTWEAKS_PIPE_FIXTURE", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let (code, text) = collect(child, true, &AtomicBool::new(false), &mut |_| {}).unwrap();
        assert_eq!(code, 0);
        assert!(text.contains("PIPE-OK"));
        assert!(text.contains("PIPE-ERR"));
        assert!(text.len() <= 128 * 1024);
    }
}
