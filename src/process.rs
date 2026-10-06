use std::{
    io::{self, Read},
    process::{Child, Command, Output, Stdio},
    sync::{
        mpsc::{self, Sender},
        Arc, Mutex, PoisonError,
    },
    thread,
    time::{Duration, Instant},
};
use wait_timeout::ChildExt;

pub(crate) struct Limits {
    pub(crate) deadline: Instant,
    pub(crate) stdout: u64,
    pub(crate) stderr: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Stream {
    Stdout,
    Stderr,
}

#[derive(Debug)]
pub(crate) enum Error {
    Spawn(io::Error),
    TimedOut,
    TooLarge(Stream),
    Io(io::Error),
}

type Buffer = Arc<Mutex<Vec<u8>>>;

const GRACE: Duration = Duration::from_millis(100);

pub(crate) fn run(mut command: Command, limits: Limits) -> Result<Output, Error> {
    let pipe = |limit: u64| {
        if limit > 0 {
            Stdio::piped()
        } else {
            Stdio::null()
        }
    };
    command
        .stdout(pipe(limits.stdout))
        .stderr(pipe(limits.stderr));
    let mut child = command.spawn().map_err(Error::Spawn)?;
    let (sender, receiver) = mpsc::channel();
    let stdout = child
        .stdout
        .take()
        .map(|source| read(source, limits.stdout, sender.clone()));
    let stderr = child
        .stderr
        .take()
        .map(|source| read(source, limits.stderr, sender));
    let remaining = || limits.deadline.saturating_duration_since(Instant::now());
    let status = match child.wait_timeout(remaining()) {
        Ok(Some(status)) => status,
        Ok(None) => {
            stop(&mut child);
            return Err(Error::TimedOut);
        }
        Err(error) => {
            stop(&mut child);
            return Err(Error::Io(error));
        }
    };
    let drained = Instant::now() + GRACE.min(remaining());
    for _ in 0..usize::from(stdout.is_some()) + usize::from(stderr.is_some()) {
        match receiver.recv_timeout(drained.saturating_duration_since(Instant::now())) {
            Ok(result) => result.map_err(Error::Io)?,
            Err(_) => break,
        }
    }
    Ok(Output {
        status,
        stdout: take(stdout, limits.stdout, Stream::Stdout)?,
        stderr: take(stderr, limits.stderr, Stream::Stderr)?,
    })
}

fn read(
    mut source: impl Read + Send + 'static,
    limit: u64,
    sender: Sender<io::Result<()>>,
) -> Buffer {
    let buffer = Buffer::default();
    let shared = Arc::clone(&buffer);
    thread::spawn(move || {
        let mut chunk = [0; 8192];
        let result = loop {
            match source.read(&mut chunk) {
                Ok(0) => break Ok(()),
                Ok(count) => {
                    let mut bytes = shared.lock().unwrap_or_else(PoisonError::into_inner);
                    bytes.extend_from_slice(&chunk[..count]);
                    if bytes.len() as u64 > limit {
                        break Ok(());
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => break Err(error),
            }
        };
        let _ = sender.send(result);
    });
    buffer
}

fn take(buffer: Option<Buffer>, limit: u64, stream: Stream) -> Result<Vec<u8>, Error> {
    let bytes = buffer.map_or_else(Vec::new, |buffer| {
        std::mem::take(&mut *buffer.lock().unwrap_or_else(PoisonError::into_inner))
    });
    if bytes.len() as u64 > limit {
        return Err(Error::TooLarge(stream));
    }
    Ok(bytes)
}

fn stop(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(test)]
mod tests {
    use super::{run, Error, Limits, Stream};
    use std::{
        process::Command,
        time::{Duration, Instant},
    };

    fn shell(script: &str) -> Command {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", script]);
        command
    }

    fn limits(stdout: u64, stderr: u64) -> Limits {
        Limits {
            deadline: Instant::now() + Duration::from_secs(5),
            stdout,
            stderr,
        }
    }

    #[test]
    fn output_larger_than_a_pipe_buffer_finishes_before_the_deadline() {
        let started = Instant::now();
        let output = run(
            shell("dd if=/dev/zero bs=200000 count=1 2>/dev/null"),
            limits(1 << 20, 0),
        )
        .unwrap();
        assert_eq!(output.stdout.len(), 200_000);
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn endless_output_stops_at_the_limit() {
        let started = Instant::now();
        let error = run(shell("yes >&2"), limits(0, 1024)).unwrap_err();
        assert!(
            matches!(error, Error::TooLarge(Stream::Stderr)),
            "{error:?}"
        );
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn background_children_holding_the_pipe_do_not_delay_the_result() {
        let started = Instant::now();
        let output = run(shell("sleep 10 & echo done"), limits(1024, 0)).unwrap();
        assert_eq!(output.stdout, b"done\n");
        assert!(started.elapsed() < Duration::from_secs(1));
    }
}
