use std::process::Stdio;
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt};

pub const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);
pub const MAX_OUTPUT_BYTES: usize = 4 * 1024 * 1024;

pub async fn run(host: &str, args: Vec<String>) -> Result<String, String> {
    let mut child = tokio::process::Command::new("glab")
        .args(&args)
        .env("GITLAB_HOST", host)
        .current_dir(std::env::temp_dir())
        .kill_on_drop(true)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| format!("glab not runnable: {err}"))?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let wait_all = async {
        let (out_res, err_res, status_res) = tokio::join!(
            read_capped(stdout, MAX_OUTPUT_BYTES),
            read_capped(stderr, MAX_OUTPUT_BYTES),
            child.wait()
        );
        let (out_bytes, out_trunc) = out_res?;
        let (err_bytes, err_trunc) = err_res?;
        let status = status_res.map_err(|err| format!("glab wait failed: {err}"))?;
        Ok::<_, String>((out_bytes, out_trunc, err_bytes, err_trunc, status))
    };

    let (out_bytes, out_trunc, err_bytes, err_trunc, status) =
        tokio::time::timeout(COMMAND_TIMEOUT, wait_all)
            .await
            .map_err(|_| "glab timed out after 30s".to_string())??;

    let text = combine_output(&out_bytes, out_trunc, &err_bytes, err_trunc, MAX_OUTPUT_BYTES);
    if !status.success() {
        return Err(if text.is_empty() {
            format!("glab exited {status}")
        } else {
            text
        });
    }
    Ok(text)
}

async fn read_capped<R: AsyncRead + Unpin>(
    reader: Option<R>,
    limit: usize,
) -> Result<(Vec<u8>, bool), String> {
    let Some(reader) = reader else {
        return Ok((Vec::new(), false));
    };
    let mut buf = Vec::new();
    let cap = (limit as u64).saturating_add(1);
    reader
        .take(cap)
        .read_to_end(&mut buf)
        .await
        .map_err(|err| format!("glab read failed: {err}"))?;
    let truncated = buf.len() > limit;
    if truncated {
        buf.truncate(limit);
    }
    Ok((buf, truncated))
}

fn combine_output(
    stdout: &[u8],
    out_trunc: bool,
    stderr: &[u8],
    err_trunc: bool,
    limit: usize,
) -> String {
    let mut text = String::from_utf8_lossy(stdout).into_owned();
    let mut truncated = out_trunc || err_trunc;
    if !stderr.is_empty() {
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(&String::from_utf8_lossy(stderr));
    }
    if text.len() > limit {
        let mut end = limit;
        while end > 0 && !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
        truncated = true;
    }
    if truncated {
        text.push_str("\n… output truncated at 4 MB limit …");
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combine_output_caps_combined_bytes() {
        let out = combine_output(b"hello", false, b"world!", false, 8);
        assert!(out.contains("truncated at 4 MB"));
        assert!(out.starts_with("hello\nwo"));
    }
}
