use anyhow::{anyhow, Context, Result};
use lazy_static::lazy_static;
use regex::Regex;
use serde_json::Value;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use super::metrics::{AudioStats, FileMetrics};

#[derive(Debug, Clone)]
pub struct ProcessingConfig {
    pub ffmpeg_path: PathBuf,
    pub ffprobe_path: Option<PathBuf>,
    pub command_timeout: Duration,
    pub process_limiter: ProcessLimiter,
}

#[derive(Debug, Clone)]
pub struct ProcessLimiter {
    max_parallel: usize,
    state: Arc<(Mutex<usize>, Condvar)>,
}

impl ProcessLimiter {
    pub fn new(max_parallel: usize) -> Self {
        Self {
            max_parallel: max_parallel.max(1),
            state: Arc::new((Mutex::new(0), Condvar::new())),
        }
    }

    fn acquire(&self) -> ProcessPermit {
        let (lock, cv) = &*self.state;
        let mut running = lock.lock().expect("process limiter mutex poisoned");
        while *running >= self.max_parallel {
            running = cv
                .wait(running)
                .expect("process limiter condvar wait failed");
        }
        *running += 1;
        ProcessPermit {
            state: Arc::clone(&self.state),
        }
    }
}

#[derive(Debug)]
struct ProcessPermit {
    state: Arc<(Mutex<usize>, Condvar)>,
}

impl Drop for ProcessPermit {
    fn drop(&mut self) {
        let (lock, cv) = &*self.state;
        if let Ok(mut running) = lock.lock() {
            *running = running.saturating_sub(1);
            cv.notify_one();
        }
    }
}

#[derive(Debug, Default, Clone)]
struct ProbeData {
    sample_rate_hz: Option<u32>,
    bitrate_kbps: Option<u32>,
    channels: Option<u32>,
    codec_name: Option<String>,
    container_format: Option<String>,
    duration_seconds: Option<f64>,
}

#[derive(Debug, Default, Clone)]
struct Ebur128Stats {
    lra: Option<f64>,
    integrated_loudness_lufs: Option<f64>,
    true_peak_dbtp: Option<f64>,
}

#[derive(Debug)]
struct CommandOutput {
    status_ok: bool,
    stdout: String,
    stderr: String,
    status_text: String,
}

lazy_static! {
    static ref EBUR128_LRA_REGEX: Regex = Regex::new(r"LRA:\s*([0-9.+-]+)").unwrap();
    static ref EBUR128_SUMMARY_LRA_REGEX: Regex =
        Regex::new(r"(?m)^\s*LRA:\s*([0-9.+-]+)\s*LU\s*$").unwrap();
    static ref EBUR128_SUMMARY_I_REGEX: Regex =
        Regex::new(r"(?m)^\s*I:\s*([0-9.+-]+)\s*LUFS\s*$").unwrap();
    static ref EBUR128_SUMMARY_TP_REGEX: Regex =
        Regex::new(r"(?m)^\s*Peak:\s*([0-9.+-]+)\s*dBFS\s*$").unwrap();
    static ref EBUR128_STREAM_TPK_REGEX: Regex = Regex::new(r"TPK:\s*([0-9.+-]+)").unwrap();
    static ref OVERALL_STATS_REGEX: Regex =
        Regex::new(r"(?s)Overall.*?Peak level dB:\s*([-\d.]+).*?RMS level dB:\s*([-\d.]+)")
            .unwrap();
    static ref HIGHPASS_ASTATS_REGEX: Regex =
        Regex::new(r"(?s)Overall.*?RMS level dB:\s*([-\d.]+)").unwrap();
    static ref ERROR_CODE_REGEX: Regex = Regex::new(r"\[(E_[A-Z0-9_]+)\]").unwrap();
}

fn run_command(mut command: Command, config: &ProcessingConfig) -> Result<CommandOutput> {
    let _permit = config.process_limiter.acquire();

    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = command.spawn().context("[E_EXEC_SPAWN] 启动外部命令失败")?;
    let stdout_pipe = child
        .stdout
        .take()
        .ok_or_else(|| anyhow!("[E_EXEC_STDOUT] 无法捕获 stdout"))?;
    let stderr_pipe = child
        .stderr
        .take()
        .ok_or_else(|| anyhow!("[E_EXEC_STDERR] 无法捕获 stderr"))?;

    let stdout_thread = thread::spawn(move || -> Result<Vec<u8>> {
        let mut reader = stdout_pipe;
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf)?;
        Ok(buf)
    });

    let stderr_thread = thread::spawn(move || -> Result<Vec<u8>> {
        let mut reader = stderr_pipe;
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf)?;
        Ok(buf)
    });

    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().context("[E_EXEC_WAIT] 等待子进程失败")? {
            break status;
        }

        if start.elapsed() > config.command_timeout {
            let _ = child.kill();
            let _ = child.wait();

            let _ = stdout_thread.join();
            let _ = stderr_thread.join();
            return Err(anyhow!(
                "[E_TIMEOUT] 外部命令执行超时 (>{}s)",
                config.command_timeout.as_secs()
            ));
        }

        thread::sleep(Duration::from_millis(25));
    };

    let stdout_bytes = stdout_thread
        .join()
        .map_err(|_| anyhow!("[E_EXEC_STDOUT] 读取 stdout 线程崩溃"))??;
    let stderr_bytes = stderr_thread
        .join()
        .map_err(|_| anyhow!("[E_EXEC_STDERR] 读取 stderr 线程崩溃"))??;

    Ok(CommandOutput {
        status_ok: status.success(),
        stdout: String::from_utf8_lossy(&stdout_bytes).to_string(),
        stderr: String::from_utf8_lossy(&stderr_bytes).to_string(),
        status_text: status.to_string(),
    })
}

fn run_command_and_get_stderr(command: Command, config: &ProcessingConfig) -> Result<String> {
    let output = run_command(command, config)?;
    if !output.status_ok {
        let preview = output.stderr.chars().take(500).collect::<String>();
        return Err(anyhow!(
            "[E_EXEC_FAILED] 命令执行失败 (status: {}): {}",
            output.status_text,
            preview
        ));
    }
    Ok(output.stderr)
}

fn get_ebur128_stats(path: &Path, config: &ProcessingConfig) -> Result<Ebur128Stats> {
    let mut command = Command::new(&config.ffmpeg_path);
    command
        .arg("-i")
        .arg(path)
        .arg("-filter_complex")
        .arg("ebur128=peak=true")
        .arg("-f")
        .arg("null")
        .arg("-");

    let stderr = run_command_and_get_stderr(command, config)?;

    let lra = EBUR128_SUMMARY_LRA_REGEX
        .captures(&stderr)
        .and_then(|caps| caps.get(1))
        .and_then(|m| parse_float_token(m.as_str()))
        .or_else(|| {
            EBUR128_LRA_REGEX
                .captures_iter(&stderr)
                .filter_map(|caps| caps.get(1).and_then(|m| parse_float_token(m.as_str())))
                .last()
        });

    let integrated_loudness_lufs = EBUR128_SUMMARY_I_REGEX
        .captures(&stderr)
        .and_then(|caps| caps.get(1))
        .and_then(|m| parse_float_token(m.as_str()));

    let true_peak_dbtp = EBUR128_SUMMARY_TP_REGEX
        .captures(&stderr)
        .and_then(|caps| caps.get(1))
        .and_then(|m| parse_float_token(m.as_str()))
        .or_else(|| {
            EBUR128_STREAM_TPK_REGEX
                .captures_iter(&stderr)
                .filter_map(|caps| caps.get(1).and_then(|m| parse_float_token(m.as_str())))
                .last()
        });

    if lra.is_none() || integrated_loudness_lufs.is_none() {
        return Err(anyhow!("[E_PARSE_EBUR128] 无法完整解析 ebur128 输出"));
    }

    Ok(Ebur128Stats {
        lra,
        integrated_loudness_lufs,
        true_peak_dbtp,
    })
}

fn parse_float_token(token: &str) -> Option<f64> {
    let text = token.trim().to_ascii_lowercase();
    match text.as_str() {
        "inf" | "+inf" => Some(f64::INFINITY),
        "-inf" => Some(f64::NEG_INFINITY),
        "nan" => None,
        _ => text.parse::<f64>().ok(),
    }
}

fn get_stats_ffmpeg(path: &Path, config: &ProcessingConfig) -> Result<AudioStats> {
    let mut command = Command::new(&config.ffmpeg_path);
    command
        .arg("-i")
        .arg(path)
        .arg("-filter:a")
        .arg("astats=metadata=1")
        .arg("-f")
        .arg("null")
        .arg("-");

    let stderr = run_command_and_get_stderr(command, config)?;

    OVERALL_STATS_REGEX
        .captures(&stderr)
        .map(|caps| {
            let peak_db = caps.get(1).and_then(|m| m.as_str().parse::<f64>().ok());
            let rms_db = caps.get(2).and_then(|m| m.as_str().parse::<f64>().ok());
            AudioStats { peak_db, rms_db }
        })
        .ok_or_else(|| anyhow!("[E_PARSE_STATS] 无法解析峰值/RMS"))
}

fn get_highpass_rms_ffmpeg(path: &Path, freq: u32, config: &ProcessingConfig) -> Result<f64> {
    let mut command = Command::new(&config.ffmpeg_path);
    let filter_str = format!("highpass=f={freq},astats=metadata=1");
    command
        .arg("-i")
        .arg(path)
        .arg("-filter:a")
        .arg(filter_str)
        .arg("-f")
        .arg("null")
        .arg("-");

    let stderr = run_command_and_get_stderr(command, config)?;

    HIGHPASS_ASTATS_REGEX
        .captures(&stderr)
        .and_then(|caps| caps.get(1))
        .and_then(|m| m.as_str().parse::<f64>().ok())
        .ok_or_else(|| anyhow!("[E_PARSE_HIGHPASS] 无法解析高通 RMS (freq: {freq})"))
}

fn get_probe_data(path: &Path, config: &ProcessingConfig) -> Result<ProbeData> {
    let ffprobe = match &config.ffprobe_path {
        Some(path) => path,
        None => return Ok(ProbeData::default()),
    };

    let mut command = Command::new(ffprobe);
    command
        .arg("-v")
        .arg("error")
        .arg("-select_streams")
        .arg("a:0")
        .arg("-show_entries")
        .arg("stream=codec_name,sample_rate,channels,bit_rate:format=format_name,bit_rate,duration")
        .arg("-of")
        .arg("json")
        .arg(path);

    let output = run_command(command, config)?;
    if !output.status_ok {
        let preview = output.stderr.chars().take(300).collect::<String>();
        return Err(anyhow!(
            "[E_FFPROBE_FAILED] ffprobe 执行失败 (status: {}): {}",
            output.status_text,
            preview
        ));
    }

    parse_probe_json(&output.stdout)
}

fn parse_probe_json(text: &str) -> Result<ProbeData> {
    let value: Value = serde_json::from_str(text)
        .map_err(|_| anyhow!("[E_PARSE_FFPROBE] ffprobe JSON 解析失败"))?;

    let stream = value
        .get("streams")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .cloned()
        .unwrap_or(Value::Null);

    let format = value.get("format").cloned().unwrap_or(Value::Null);

    let sample_rate_hz = parse_u32(stream.get("sample_rate"));
    let channels = parse_u32(stream.get("channels"));
    let codec_name = parse_string(stream.get("codec_name"));
    let container_format = parse_string(format.get("format_name"));
    let duration_seconds = parse_f64(format.get("duration"));

    let stream_bitrate = parse_u64(stream.get("bit_rate"));
    let format_bitrate = parse_u64(format.get("bit_rate"));
    let bitrate_kbps = stream_bitrate
        .or(format_bitrate)
        .map(|bps| ((bps as f64) / 1000.0).round() as u32);

    Ok(ProbeData {
        sample_rate_hz,
        bitrate_kbps,
        channels,
        codec_name,
        container_format,
        duration_seconds,
    })
}

fn parse_u32(value: Option<&Value>) -> Option<u32> {
    parse_u64(value).and_then(|v| u32::try_from(v).ok())
}

fn parse_u64(value: Option<&Value>) -> Option<u64> {
    match value {
        Some(Value::Number(num)) => num.as_u64(),
        Some(Value::String(s)) => s.parse::<u64>().ok(),
        _ => None,
    }
}

fn parse_f64(value: Option<&Value>) -> Option<f64> {
    match value {
        Some(Value::Number(num)) => num.as_f64(),
        Some(Value::String(s)) => s.parse::<f64>().ok(),
        _ => None,
    }
}

fn parse_string(value: Option<&Value>) -> Option<String> {
    value.and_then(|v| v.as_str()).map(ToOwned::to_owned)
}

fn extract_error_code(err: &anyhow::Error, fallback: &str) -> String {
    let msg = err.to_string();
    ERROR_CODE_REGEX
        .captures(&msg)
        .and_then(|caps| caps.get(1).map(|m| m.as_str().to_owned()))
        .unwrap_or_else(|| fallback.to_owned())
}

pub fn process_file(path: &Path, config: &ProcessingConfig) -> Result<FileMetrics> {
    let start_time = Instant::now();
    let file_size_bytes = path.metadata()?.len();

    let (ebur_res, (stats_res, (rms_16k_res, (rms_18k_res, rms_20k_res)))) = rayon::join(
        || get_ebur128_stats(path, config),
        || {
            rayon::join(
                || get_stats_ffmpeg(path, config),
                || {
                    rayon::join(
                        || get_highpass_rms_ffmpeg(path, 16000, config),
                        || {
                            rayon::join(
                                || get_highpass_rms_ffmpeg(path, 18000, config),
                                || get_highpass_rms_ffmpeg(path, 20000, config),
                            )
                        },
                    )
                },
            )
        },
    );

    let probe_res = get_probe_data(path, config);
    let processing_time_ms = start_time.elapsed().as_millis() as u64;

    let mut error_codes = Vec::new();

    let (lra, integrated_loudness_lufs, true_peak_dbtp) = match ebur_res {
        Ok(stats) => (
            stats.lra,
            stats.integrated_loudness_lufs,
            stats.true_peak_dbtp,
        ),
        Err(err) => {
            error_codes.push(extract_error_code(&err, "E_EBUR128"));
            (None, None, None)
        }
    };

    let (peak_amplitude_db, overall_rms_db) = match stats_res {
        Ok(stats) => (stats.peak_db, stats.rms_db),
        Err(err) => {
            error_codes.push(extract_error_code(&err, "E_STATS"));
            (None, None)
        }
    };

    let rms_db_above_16k = match rms_16k_res {
        Ok(value) => Some(value),
        Err(err) => {
            error_codes.push(extract_error_code(&err, "E_RMS16K"));
            None
        }
    };

    let rms_db_above_18k = match rms_18k_res {
        Ok(value) => Some(value),
        Err(err) => {
            error_codes.push(extract_error_code(&err, "E_RMS18K"));
            None
        }
    };

    let rms_db_above_20k = match rms_20k_res {
        Ok(value) => Some(value),
        Err(err) => {
            error_codes.push(extract_error_code(&err, "E_RMS20K"));
            None
        }
    };

    let probe = match probe_res {
        Ok(probe) => probe,
        Err(err) => {
            error_codes.push(extract_error_code(&err, "E_FFPROBE"));
            ProbeData::default()
        }
    };

    error_codes.sort();
    error_codes.dedup();

    Ok(FileMetrics {
        file_path: path.to_string_lossy().into_owned(),
        file_size_bytes,
        lra,
        peak_amplitude_db,
        overall_rms_db,
        rms_db_above_16k,
        rms_db_above_18k,
        rms_db_above_20k,
        integrated_loudness_lufs,
        true_peak_dbtp,
        processing_time_ms,
        sample_rate_hz: probe.sample_rate_hz,
        bitrate_kbps: probe.bitrate_kbps,
        channels: probe.channels,
        codec_name: probe.codec_name,
        container_format: probe.container_format,
        duration_seconds: probe.duration_seconds,
        cache_hit: false,
        content_sha256: None,
        error_codes,
    })
}
