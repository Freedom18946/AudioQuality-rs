// ----------------------------------------------------------------
// 项目: 音频质量分析器 (Audio Quality Analyzer)
// 模块: analyzer/ffmpeg.rs
// 描述: 此模块封装了所有与 FFmpeg 的交互逻辑。它通过调用 FFmpeg 命令行工具，
//      使用不同的滤波器（如 ebur128, astats）来提取音频的技术指标。
//      为了提高效率，多个独立的分析任务被设计为可以并行执行。
// ----------------------------------------------------------------

use anyhow::{anyhow, Result}; // anyhow 用于提供更具上下文的错误处理
use lazy_static::lazy_static; // lazy_static 用于在程序首次访问时才初始化静态变量，非常适合预编译正则表达式
use regex::Regex; // regex 库用于从 FFmpeg 的文本输出中解析和提取数值
use std::path::Path; // Path 用于处理文件系统路径
use std::process::{Command, Stdio}; // Command 和 Stdio 用于创建和管理子进程（即 FFmpeg）

// 从同级模块 `metrics` 中导入所需的数据结构
use super::metrics::{AudioStats, FileMetrics};

// --- 预编译的、经过性能优化的正则表达式 ---
// 使用 `lazy_static!` 宏可以确保正则表达式只被编译一次，并在整个程序生命周期内复用。
// 这避免了在函数调用中反复编译正则表达式带来的性能开销。
lazy_static! {
    // 用于从 ebur128 滤波器的实时输出中提取 LRA (Loudness Range, 响度范围)。
    // FFmpeg 在处理时会不断输出当前的 LRA 值，我们通常关心的是最后那个值。
    // 示例: "LRA: 11.7"
    static ref EBUR128_LRA_REGEX: Regex = Regex::new(r"LRA:\s*([0-9.-]+)").unwrap();

    // 用于从 ebur128 滤波器的最终汇总报告中提取 LRA。
    // 这通常比从实时输出中获取更可靠。`(?m)` 是多行模式标志。
    // 示例 (在多行输出的末尾):
    // "LRA:               11.7 LU"
    static ref EBUR128_SUMMARY_LRA_REGEX: Regex = Regex::new(r"(?m)^LRA:\s*([0-9.-]+)\s*LU\s*$").unwrap();

    // 用于从 astats 滤波器的 "Overall" 统计块中一次性提取峰值和 RMS 电平。
    // `(?s)` 是单行模式标志，它允许 `.` 匹配换行符，这对于跨越多行的文本块匹配至关重要。
    // 这个正则表达式会寻找 "Overall" 块，然后分别捕获 "Peak level dB" 和 "RMS level dB" 的值。
    static ref OVERALL_STATS_REGEX: Regex = Regex::new(
        r"(?s)\[Parsed_astats_0 @ [^]]+\] Overall.*?Peak level dB:\s*([-\d.]+).*?RMS level dB:\s*([-\d.]+)"
    ).unwrap();

    // 用于从经过高通滤波器 (highpass) 处理后的 astats 输出中提取 RMS 值。
    // 当链式使用滤波器时 (如 highpass, then astats)，astats 的标识符可能会改变 (如 `Parsed_astats_1`)。
    // 这个正则专门匹配这种情况下的 RMS 值。
    static ref HIGHPASS_ASTATS_REGEX: Regex = Regex::new(
        r"(?s)\[Parsed_astats_1 @ [^]]+\] Overall.*?RMS level dB:\s*([-\d.]+)"
    ).unwrap();
}

/// 一个辅助函数，用于运行一个配置好的 `Command` 并捕获其 `stderr` 输出。
/// FFmpeg 经常将其报告和统计信息输出到 stderr 而不是 stdout。
///
/// # 参数
/// - `command`: 一个已经配置好参数的 `std::process::Command` 实例。
///
/// # 返回
/// - `Result<String>`: 如果命令成功执行，返回 `Ok` 包含 `stderr` 的内容；如果失败，返回 `Err` 包含错误信息。
fn run_command_and_get_stderr(mut command: Command) -> Result<String> {
    // `stdin(Stdio::null())` 和 `stdout(Stdio::null())` 关闭了标准输入和输出流，
    // 因为我们只关心标准错误流 `stderr`。
    let output = command.stdin(Stdio::null()).stdout(Stdio::null()).output()?;

    if !output.status.success() {
        // 如果 FFmpeg 命令执行失败（例如，文件损坏或参数错误），
        // 我们构造一个包含退出状态码和部分 stderr 内容的错误信息，以便于调试。
        let stderr_preview = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!(
            "FFmpeg command failed with status: {}. Stderr: {}",
            output.status,
            stderr_preview.chars().take(500).collect::<String>() // 只截取前500个字符以避免错误信息过长
        ));
    }
    // 如果命令成功，将 stderr 的字节流转换为 String 并返回。
    Ok(String::from_utf8_lossy(&output.stderr).to_string())
}

/// 使用 FFmpeg 的 `ebur128` 滤波器获取音频的 LRA (响度范围)。
fn get_lra_ebur128(path: &Path, ffmpeg_path: &Path) -> Result<f64> {
    let mut command = Command::new(ffmpeg_path);
    command
        .arg("-i") // 输入文件
        .arg(path)
        .arg("-filter_complex") // 使用复杂的滤波器图
        .arg("ebur128") // 指定 ebur128 滤波器
        .arg("-f")
        .arg("null") // 不输出任何媒体文件，我们只关心分析报告
        .arg("-"); // 表示输出到 stdout/stderr

    let stderr = run_command_and_get_stderr(command)?;

    // 优先尝试从最终的汇总报告中解析 LRA，因为这通常是最准确的。
    if let Some(caps) = EBUR128_SUMMARY_LRA_REGEX.captures(&stderr) {
        if let Some(lra_str) = caps.get(1) {
            // 如果匹配成功，解析字符串为 f64 并返回。
            return lra_str.as_str().parse::<f64>().map_err(|e| anyhow!(e));
        }
    }

    // 如果无法从汇总报告中找到 LRA（某些旧版本的 FFmpeg 可能没有），
    // 则退而求其次，从实时的流式输出中找到最后一个出现的 LRA 值。
    EBUR128_LRA_REGEX
        .captures_iter(&stderr)
        .filter_map(|caps| caps.get(1).and_then(|m| m.as_str().parse::<f64>().ok()))
        .last() // 获取最后一个有效的 LRA 值
        .ok_or_else(|| anyhow!("无法从 ebur128 输出中解析 LRA 值"))
}

/// 使用 FFmpeg 的 `astats` 滤波器获取音频的总体峰值和 RMS。
fn get_stats_ffmpeg(path: &Path, ffmpeg_path: &Path) -> Result<AudioStats> {
    let mut command = Command::new(ffmpeg_path);
    command
        .arg("-i")
        .arg(path)
        .arg("-filter:a") // 指定音频滤波器
        .arg("astats=metadata=1") // 使用 astats 滤波器，并启用元数据输出
        .arg("-f")
        .arg("null")
        .arg("-");

    let stderr = run_command_and_get_stderr(command)?;

    // 使用正则表达式从 stderr 中捕获峰值和 RMS 值。
    OVERALL_STATS_REGEX
        .captures(&stderr)
        .map(|caps| {
            // caps.get(1) 对应第一个捕获组 (峰值)
            let peak_db = caps.get(1).and_then(|m| m.as_str().parse::<f64>().ok());
            // caps.get(2) 对应第二个捕获组 (RMS)
            let rms_db = caps.get(2).and_then(|m| m.as_str().parse::<f64>().ok());
            AudioStats { peak_db, rms_db }
        })
        .ok_or_else(|| anyhow!("无法从 astats 输出中解析峰值/RMS"))
}

/// 使用 `highpass` 和 `astats` 滤波器链，获取指定频率以上频段的 RMS。
/// 这是判断“假无损”的关键步骤之一。
fn get_highpass_rms_ffmpeg(path: &Path, freq: u32, ffmpeg_path: &Path) -> Result<f64> {
    let mut command = Command::new(ffmpeg_path);
    // 将 `highpass` 和 `astats` 滤波器链接在一起。
    // 音频数据首先通过高通滤波器，滤掉 `freq` Hz 以下的频率，然后将结果送入 `astats` 进行分析。
    let filter_str = format!("highpass=f={freq},astats=metadata=1");
    command
        .arg("-i")
        .arg(path)
        .arg("-filter:a")
        .arg(&filter_str)
        .arg("-f")
        .arg("null")
        .arg("-");

    let stderr = run_command_and_get_stderr(command)?;

    // 从输出中解析出 RMS 值。
    HIGHPASS_ASTATS_REGEX
        .captures(&stderr)
        .and_then(|caps| caps.get(1)) // 获取第一个捕获组
        .and_then(|m| m.as_str().parse::<f64>().ok()) // 解析为 f64
        .ok_or_else(|| anyhow!("无法从 highpass+astats 输出中解析 RMS (freq: {})", freq))
}

/// 并行处理单个音频文件，提取所有需要的指标。
///
/// 这是此模块向外暴露的核心公共函数。它精心编排了多个独立的 FFmpeg 分析任务，
/// 并使用 `rayon::join` 来实现最大程度的并行化，从而显著缩短总体分析时间。
///
/// # 参数
/// - `path`: 要分析的音频文件的路径。
/// - `ffmpeg_path`: FFmpeg 可执行文件的路径。
///
/// # 返回
/// - `Result<FileMetrics>`: 如果成功，返回一个包含所有已提取指标的 `FileMetrics` 结构体。
pub fn process_file(path: &Path, ffmpeg_path: &Path) -> Result<FileMetrics> {
    let start_time = std::time::Instant::now();
    let file_size_bytes = path.metadata()?.len();

    // `rayon::join` 是实现并行的核心。它会接收多个闭包（匿名函数），
    // 并尝试将它们调度到不同的线程上同时执行。
    // 这里通过嵌套的 `join` 调用，将5个独立的 FFmpeg 任务并行化。
    let (lra_res, (stats_res, (rms_16k_res, (rms_18k_res, rms_20k_res)))) = rayon::join(
        || get_lra_ebur128(path, ffmpeg_path), // 任务1: 获取 LRA
        || {
            rayon::join(
                || get_stats_ffmpeg(path, ffmpeg_path), // 任务2: 获取总体峰值和 RMS
                || {
                    rayon::join(
                        || get_highpass_rms_ffmpeg(path, 16000, ffmpeg_path), // 任务3: 获取 >16kHz RMS
                        || {
                            rayon::join(
                                || get_highpass_rms_ffmpeg(path, 18000, ffmpeg_path), // 任务4: 获取 >18kHz RMS
                                || get_highpass_rms_ffmpeg(path, 20000, ffmpeg_path),  // 任务5: 获取 >20kHz RMS
                            )
                        },
                    )
                },
            )
        },
    );

    let processing_time_ms = start_time.elapsed().as_millis() as u64;

    // 将所有并行任务的结果组合成最终的 `FileMetrics` 结构体。
    // 每个任务返回的都是 `Result` 类型，我们使用 `.ok()` 方法将其转换为 `Option` 类型。
    // 如果任务成功，`ok()` 返回 `Some(value)`；如果失败，返回 `None`。
    // 这种方式使得即使部分分析失败，程序也能继续运行并报告成功提取的指标。
    Ok(FileMetrics {
        file_path: path.to_string_lossy().into_owned(),
        file_size_bytes,
        lra: lra_res.ok(),
        peak_amplitude_db: stats_res.as_ref().ok().and_then(|s| s.peak_db),
        overall_rms_db: stats_res.as_ref().ok().and_then(|s| s.rms_db),
        rms_db_above_16k: rms_16k_res.ok(),
        rms_db_above_18k: rms_18k_res.ok(),
        rms_db_above_20k: rms_20k_res.ok(),
        processing_time_ms,
    })
}
