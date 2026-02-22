// ----------------------------------------------------------------
// 项目: 音频质量分析器 (Audio Quality Analyzer)
// 模块: analyzer/metrics.rs
// 描述: 此模块定义了项目中用于数据交换和存储的核心数据结构。
//      这些结构体被设计为可序列化和反序列化，以便与 JSON 格式兼容。
// ----------------------------------------------------------------

use serde::{Deserialize, Serialize};

/// `AudioStats` 结构体是一个辅助性的数据容器。
/// 它用于临时存储从 FFmpeg 的 `astats` 滤波器一次性返回的两个关键指标：
/// 峰值电平 (Peak level) 和均方根 (RMS) 电平。
///
/// `#[derive(...)]` 宏会自动为结构体实现一些有用的 trait：
/// - `Debug`: 允许使用 `{:?}` 格式化打印结构体，方便调试。
/// - `Default`: 允许创建此结构体的默认实例（所有字段为 `None`）。
/// - `Clone`, `Copy`: 允许高效地复制此结构体的实例。
#[derive(Debug, Default, Clone, Copy)]
pub struct AudioStats {
    /// 音频的峰值电平，单位是分贝 (dB)。
    /// `Option<f64>` 表示这个值可能不存在（例如，如果 FFmpeg 解析失败）。
    pub peak_db: Option<f64>,
    /// 音频的均方根 (RMS) 电平，单位是分贝 (dB)。
    /// 这反映了音频的平均功率。
    pub rms_db: Option<f64>,
}

/// `FileMetrics` 结构体是核心数据模型，用于存储从单个音频文件中提取的所有最终技术指标。
///
/// 这个结构体的字段和命名通过 `#[serde(rename = "...")]` 属性与最终的 `analysis_data.json`
/// 文件格式严格对应，确保可以被 `serde_json` 库正确地序列化。
///
/// `#[derive(...)]` 宏:
/// - `Debug`: 方便调试。
/// - `Serialize`, `Deserialize`: `serde` 的核心功能，使其能够与 JSON 等格式进行转换。
/// - `Default`: 方便创建空的或默认的实例。
/// - `Clone`: 允许复制实例。
#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct FileMetrics {
    /// 文件的完整路径。
    /// `#[serde(rename = "filePath")]` 指定在 JSON 中此字段的名称应为 "filePath"。
    #[serde(rename = "filePath")]
    pub file_path: String,

    /// 文件的大小，单位是字节 (bytes)。
    #[serde(rename = "fileSizeBytes")]
    pub file_size_bytes: u64,

    // --- 核心音频指标 ---
    // 所有指标都定义为 `Option<f64>`，因为 FFmpeg 在分析过程中可能因各种原因
    // (如文件损坏、格式不支持等) 而无法提取某个特定指标。这种设计增强了程序的健壮性。
    /// 响度范围 (Loudness Range, LRA)，单位是 LU (Loudness Units)。
    /// 它衡量了音频宏观动态范围的大小。
    #[serde(rename = "lra")]
    pub lra: Option<f64>,

    /// 峰值幅度 (Peak Amplitude)，单位是分贝 (dB)。
    /// 这是音频样本达到的最大绝对值。
    #[serde(rename = "peakAmplitudeDb")]
    pub peak_amplitude_db: Option<f64>,

    /// 整体均方根 (Overall RMS) 电平，单位是分贝 (dB)。
    /// 反映了整个文件的平均响度。
    #[serde(rename = "overallRmsDb")]
    pub overall_rms_db: Option<f64>,

    /// 16kHz 以上频段的 RMS 电平，单位是分贝 (dB)。
    /// 用于辅助判断高频信息的丰富程度。
    #[serde(rename = "rmsDbAbove16k")]
    pub rms_db_above_16k: Option<f64>,

    /// 18kHz 以上频段的 RMS 电平，单位是分贝 (dB)。
    #[serde(rename = "rmsDbAbove18k")]
    pub rms_db_above_18k: Option<f64>,

    /// 20kHz 以上频段的 RMS 电平，单位是分贝 (dB)。
    /// 这是判断“假无损”的重要参考指标之一。
    #[serde(rename = "rmsDbAbove20k")]
    pub rms_db_above_20k: Option<f64>,

    /// 处理单个文件所花费的时间，单位是毫秒 (ms)。
    /// 用于性能评估。
    #[serde(rename = "processingTimeMs")]
    pub processing_time_ms: u64,

    /// 采样率（Hz），来自 ffprobe 元数据。
    #[serde(rename = "sampleRateHz")]
    pub sample_rate_hz: Option<u32>,

    /// 码率（kbps），来自 ffprobe 元数据。
    #[serde(rename = "bitrateKbps")]
    pub bitrate_kbps: Option<u32>,

    /// 声道数，来自 ffprobe 元数据。
    #[serde(rename = "channels")]
    pub channels: Option<u32>,

    /// 音频编码器名称。
    #[serde(rename = "codecName")]
    pub codec_name: Option<String>,

    /// 容器格式名称。
    #[serde(rename = "containerFormat")]
    pub container_format: Option<String>,

    /// 音频时长（秒）。
    #[serde(rename = "durationSeconds")]
    pub duration_seconds: Option<f64>,

    /// 该条目是否来自增量缓存命中。
    #[serde(rename = "cacheHit", default)]
    pub cache_hit: bool,

    /// 文件内容 SHA-256（用于缓存一致性验证）。
    #[serde(rename = "contentSha256")]
    pub content_sha256: Option<String>,

    /// 风险/失败原因码（例如 E_TIMEOUT, E_PARSE_LRA）。
    #[serde(rename = "errorCodes", default)]
    pub error_codes: Vec<String>,
}
