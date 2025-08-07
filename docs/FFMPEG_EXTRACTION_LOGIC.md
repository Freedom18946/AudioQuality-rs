# FFmpeg 音频指标提取核心逻辑

本文档详细阐述了 `AudioQuality-rs` 项目如何利用 `ffmpeg` 命令行工具来提取音频文件的各项技术指标。核心逻辑封装在 `src/analyzer/ffmpeg.rs` 模块中。

## 核心设计理念

为了最大化分析效率，系统采用了以下两个核心设计：

1.  **并行处理**: 多个独立的 `ffmpeg` 分析任务（如响度范围、峰值电平、高频 RMS 等）被设计为可以同时执行。通过 `rayon::join` 将这些任务调度到不同的 CPU 核心上，极大地缩短了单个文件的总处理时间。

2.  **预编译正则表达式**: 所有用于解析 `ffmpeg` `stderr` 输出的正则表达式都使用 `lazy_static!` 宏进行预编译。这确保了正则表达式的编译开销在程序生命周期内只发生一次，避免了在每次函数调用时重复编译，从而提高了性能。

## 指标提取详解

以下是每项关键指标的提取方法、所用的 `ffmpeg` 命令及输出解析逻辑。

### 1. 响度范围 (Loudness Range - LRA)

-   **目的**: 测量音频的宏观动态范围，即最响和最静部分之间的差异。
-   **FFmpeg 滤波器**: `ebur128`
-   **命令示例**:
    ```bash
    ffmpeg -i "path/to/audio.flac" -filter_complex ebur128 -f null -
    ```
-   **输出解析**:
    `ebur128` 滤波器会在其 `stderr` 输出的最终摘要部分报告 LRA 值。我们使用正则表达式来捕获这个值。

    -   **摘要正则**:
        ```rust
        // 匹配 "LRA:               11.7 LU" 这样的行
        static ref EBUR128_SUMMARY_LRA_REGEX: Regex = Regex::new(r"(?m)^LRA:\s*([0-9.-]+)\s*LU\s*$").unwrap();
        ```
    -   **备用实时正则**:
        如果摘要中未找到（兼容旧版 `ffmpeg`），则会从实时输出中捕获最后一个出现的 LRA 值。
        ```rust
        // 匹配 "LRA: 11.7"
        static ref EBUR128_LRA_REGEX: Regex = Regex::new(r"LRA:\s*([0-9.-]+)").unwrap();
        ```
-   **Rust 实现**:
    ```rust
    fn get_lra_ebur128(path: &Path, ffmpeg_path: &Path) -> Result<f64> {
        // ... (构建并执行命令)
        let stderr = run_command_and_get_stderr(command)?;

        // 优先从摘要中解析
        if let Some(caps) = EBUR128_SUMMARY_LRA_REGEX.captures(&stderr) {
            if let Some(lra_str) = caps.get(1) {
                return lra_str.as_str().parse::<f64>().map_err(|e| anyhow!(e));
            }
        }

        // 备用方案：从实时输出中获取最后一个值
        EBUR128_LRA_REGEX
            .captures_iter(&stderr)
            .filter_map(|caps| caps.get(1).and_then(|m| m.as_str().parse::<f64>().ok()))
            .last()
            .ok_or_else(|| anyhow!("无法从 ebur128 输出中解析 LRA 值"))
    }
    ```

### 2. 总体峰值 (Peak) 和均方根 (RMS) 电平

-   **目的**: 测量整个音轨的最大采样电平（峰值）和平均功率（RMS）。
-   **FFmpeg 滤波器**: `astats`
-   **命令示例**:
    ```bash
    ffmpeg -i "path/to/audio.flac" -filter:a "astats=metadata=1" -f null -
    ```
-   **输出解析**:
    `astats` 滤波器会输出一个 "Overall" 统计块，其中包含了峰值和 RMS 值。我们使用一个能跨越多行匹配的正则表达式来同时捕获这两个值。

    -   **正则表达式**:
        ```rust
        // (?s) 标志允许 `.` 匹配换行符
        static ref OVERALL_STATS_REGEX: Regex = Regex::new(
            r"(?s)\[Parsed_astats_0 @ [^]]+\] Overall.*?Peak level dB:\s*([-\d.]+).*?RMS level dB:\s*([-\d.]+)"
        ).unwrap();
        ```
-   **Rust 实现**:
    ```rust
    fn get_stats_ffmpeg(path: &Path, ffmpeg_path: &Path) -> Result<AudioStats> {
        // ... (构建并执行命令)
        let stderr = run_command_and_get_stderr(command)?;

        OVERALL_STATS_REGEX
            .captures(&stderr)
            .map(|caps| {
                let peak_db = caps.get(1).and_then(|m| m.as_str().parse::<f64>().ok());
                let rms_db = caps.get(2).and_then(|m| m.as_str().parse::<f64>().ok());
                AudioStats { peak_db, rms_db }
            })
            .ok_or_else(|| anyhow!("无法从 astats 输出中解析峰值/RMS"))
    }
    ```

### 3. 高频段 RMS 电平 (用于“假无损”检测)

-   **目的**: 测量特定高频段（如 >16kHz, >18kHz, >20kHz）的能量。如果高频段能量异常低，可能意味着音频是经过有损压缩后伪造的无损文件。
-   **FFmpeg 滤波器链**: `highpass` + `astats`
-   **命令示例 (以 16kHz 为例)**:
    ```bash
    ffmpeg -i "path/to/audio.flac" -filter:a "highpass=f=16000,astats=metadata=1" -f null -
    ```
-   **输出解析**:
    当滤波器链接在一起时，`astats` 的标识符可能会改变（例如，`Parsed_astats_1`）。因此，需要一个专门的正则表达式来处理这种情况。

    -   **正则表达式**:
        ```rust
        static ref HIGHPASS_ASTATS_REGEX: Regex = Regex::new(
            r"(?s)\[Parsed_astats_1 @ [^]]+\] Overall.*?RMS level dB:\s*([-\d.]+)"
        ).unwrap();
        ```
-   **Rust 实现**:
    ```rust
    fn get_highpass_rms_ffmpeg(path: &Path, freq: u32, ffmpeg_path: &Path) -> Result<f64> {
        let filter_str = format!("highpass=f={},astats=metadata=1", freq);
        // ... (构建并执行命令)
        let stderr = run_command_and_get_stderr(command)?;

        HIGHPASS_ASTATS_REGEX
            .captures(&stderr)
            .and_then(|caps| caps.get(1))
            .and_then(|m| m.as_str().parse::<f64>().ok())
            .ok_or_else(|| anyhow!("无法从 highpass+astats 输出中解析 RMS (freq: {})", freq))
    }
    ```

## 总结

通过精心设计的 `ffmpeg` 命令和高效的并行处理及文本解析，`AudioQuality-rs` 能够快速、准确地从音频文件中提取出一系列有价值的技术指标，为后续的质量评估和分数计算提供了坚实的数据基础。
