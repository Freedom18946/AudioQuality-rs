// ================================================================
// 项目: 音频质量分析器 (AudioQuality-rs)
// 模块: analyzer/scoring.rs
// 作者: AudioQuality-rs 开发团队
// 版本: 4.0.0
// 描述: 音频质量评分算法核心实现模块
//
// 功能概述:
// - 实现三维质量评分体系（完整性、动态范围、频谱质量）
// - 提供音频质量状态自动检测和分类功能
// - 基于音频工程最佳实践的阈值配置系统
// - 与Python参考实现保持算法一致性，确保评分准确性
// - 支持批量处理和并行评分计算
// ================================================================

use super::metrics::FileMetrics;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// 质量评分阈值配置
/// 这些阈值与Python版本保持完全一致，确保评分结果的一致性
#[derive(Debug, Clone)]
pub struct QualityThresholds {
    // 频谱相关阈值
    pub spectrum_fake_threshold: f64, // -85.0 dB - 伪造音频检测阈值
    pub spectrum_processed_threshold: f64, // -80.0 dB - 处理音频检测阈值
    pub spectrum_good_threshold: f64, // -70.0 dB - 良好频谱阈值

    // 动态范围 (LRA) 相关阈值
    pub lra_poor_max: f64,       // 3.0 LU - 严重压缩上限
    pub lra_low_max: f64,        // 6.0 LU - 低动态上限
    pub lra_excellent_min: f64,  // 8.0 LU - 优秀动态下限
    pub lra_excellent_max: f64,  // 12.0 LU - 优秀动态上限
    pub lra_acceptable_max: f64, // 15.0 LU - 可接受动态上限
    pub lra_too_high: f64,       // 20.0 LU - 动态过高阈值

    // 峰值相关阈值
    pub peak_clipping_db: f64, // -0.1 dB - 削波检测阈值
    #[allow(dead_code)]
    pub peak_clipping_linear: f64, // 0.999 - 线性削波检测阈值（保留用于未来扩展）
    pub peak_good_db: f64,     // -6.0 dB - 良好峰值阈值
    pub peak_medium_db: f64,   // -3.0 dB - 中等峰值阈值
}

impl Default for QualityThresholds {
    fn default() -> Self {
        Self {
            spectrum_fake_threshold: -85.0,
            spectrum_processed_threshold: -80.0,
            spectrum_good_threshold: -70.0,
            lra_poor_max: 3.0,
            lra_low_max: 6.0,
            lra_excellent_min: 8.0,
            lra_excellent_max: 12.0,
            lra_acceptable_max: 15.0,
            lra_too_high: 20.0,
            peak_clipping_db: -0.1,
            peak_clipping_linear: 0.999,
            peak_good_db: -6.0,
            peak_medium_db: -3.0,
        }
    }
}

/// 音频质量状态枚举
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum QualityStatus {
    #[serde(rename = "质量良好")]
    Good,
    #[serde(rename = "数据不完整")]
    Incomplete,
    #[serde(rename = "可疑 (伪造)")]
    Suspicious,
    #[serde(rename = "疑似处理")]
    Processed,
    #[serde(rename = "已削波")]
    Clipped,
    #[serde(rename = "严重压缩")]
    SeverelyCompressed,
    #[serde(rename = "低动态")]
    LowDynamic,
    #[serde(rename = "低码率")]
    LowBitrate,
    #[serde(rename = "低采样率")]
    LowSampleRate,
    #[serde(rename = "单声道")]
    Mono,
}

impl std::fmt::Display for QualityStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let status_str = match self {
            QualityStatus::Good => "质量良好",
            QualityStatus::Incomplete => "数据不完整",
            QualityStatus::Suspicious => "可疑 (伪造)",
            QualityStatus::Processed => "疑似处理",
            QualityStatus::Clipped => "已削波",
            QualityStatus::SeverelyCompressed => "严重压缩",
            QualityStatus::LowDynamic => "低动态",
            QualityStatus::LowBitrate => "低码率",
            QualityStatus::LowSampleRate => "低采样率",
            QualityStatus::Mono => "单声道",
        };
        write!(f, "{status_str}")
    }
}

/// 音频质量分析结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityAnalysis {
    /// 文件路径
    #[serde(rename = "filePath")]
    pub file_path: String,

    /// 综合质量分数 (0-100)
    #[serde(rename = "质量分")]
    pub quality_score: i32,

    /// 质量状态
    #[serde(rename = "状态")]
    pub status: QualityStatus,

    /// 分析备注
    #[serde(rename = "备注")]
    pub notes: String,

    /// 原始指标数据
    #[serde(flatten)]
    pub metrics: FileMetrics,
}

/// 音频质量评分器
pub struct QualityScorer {
    thresholds: QualityThresholds,
}

impl QualityScorer {
    /// 创建新的质量评分器实例
    pub fn new() -> Self {
        Self {
            thresholds: QualityThresholds::default(),
        }
    }

    /// 使用自定义阈值创建评分器
    ///
    /// 此方法允许用户自定义评分阈值，用于特殊场景或实验性评分标准
    #[allow(dead_code)]
    pub fn with_thresholds(thresholds: QualityThresholds) -> Self {
        Self { thresholds }
    }

    /// 分析单个文件的质量
    pub fn analyze_file(&self, metrics: &FileMetrics) -> QualityAnalysis {
        let status = self.determine_status(metrics);
        let notes = self.generate_notes(metrics, &status);
        let quality_score = self.calculate_quality_score(metrics, &status);

        QualityAnalysis {
            file_path: metrics.file_path.clone(),
            quality_score,
            status,
            notes,
            metrics: metrics.clone(),
        }
    }

    /// 批量分析多个文件
    /// 对于大量文件，使用并行处理来提高性能
    pub fn analyze_files(&self, metrics_list: &[FileMetrics]) -> Vec<QualityAnalysis> {
        use rayon::prelude::*;

        // 对于少量文件，使用串行处理避免并行开销
        if metrics_list.len() < 10 {
            metrics_list
                .iter()
                .map(|metrics| self.analyze_file(metrics))
                .collect()
        } else {
            // 对于大量文件，使用并行处理
            metrics_list
                .par_iter()
                .map(|metrics| self.analyze_file(metrics))
                .collect()
        }
    }
}

impl Default for QualityScorer {
    fn default() -> Self {
        Self::new()
    }
}

impl QualityScorer {
    /// 确定音频文件的质量状态
    fn determine_status(&self, metrics: &FileMetrics) -> QualityStatus {
        // 检查关键数据完整性
        let critical_fields_missing = self.count_missing_critical_fields(metrics);
        if critical_fields_missing >= 2 {
            return QualityStatus::Incomplete;
        }

        // lossless + 高频异常 => 高可疑
        if let Some(rms_18k) = metrics.rms_db_above_18k {
            if self.is_lossless(metrics) && rms_18k < self.thresholds.spectrum_fake_threshold {
                return QualityStatus::Suspicious;
            }

            if rms_18k < self.thresholds.spectrum_processed_threshold {
                return QualityStatus::Processed;
            }
        }

        // 有损音频低码率
        if self.is_lossy(metrics) && matches!(metrics.bitrate_kbps, Some(bitrate) if bitrate < 192)
        {
            return QualityStatus::LowBitrate;
        }

        // 采样率过低
        if matches!(metrics.sample_rate_hz, Some(sr) if sr < 44_100) {
            return QualityStatus::LowSampleRate;
        }

        // 单声道提示
        if matches!(metrics.channels, Some(ch) if ch < 2) {
            return QualityStatus::Mono;
        }

        // 检查是否存在削波
        if let Some(peak_db) = metrics.peak_amplitude_db {
            if peak_db >= self.thresholds.peak_clipping_db {
                return QualityStatus::Clipped;
            }
        }

        // 检查动态范围问题
        if let Some(lra) = metrics.lra {
            if lra > 0.0 {
                if lra < self.thresholds.lra_poor_max {
                    return QualityStatus::SeverelyCompressed;
                }
                if lra < self.thresholds.lra_low_max {
                    return QualityStatus::LowDynamic;
                }
            }
        }

        QualityStatus::Good
    }

    /// 计算缺失的关键字段数量
    fn count_missing_critical_fields(&self, metrics: &FileMetrics) -> i32 {
        let mut missing_count = 0;

        // 检查关键字段: rmsDbAbove18k, lra, peakAmplitudeDb
        if metrics.rms_db_above_18k.is_none() || metrics.rms_db_above_18k == Some(0.0) {
            missing_count += 1;
        }
        if metrics.lra.is_none() || metrics.lra == Some(0.0) {
            missing_count += 1;
        }
        if metrics.peak_amplitude_db.is_none() {
            missing_count += 1;
        }

        missing_count
    }

    /// 生成分析备注
    fn generate_notes(&self, metrics: &FileMetrics, status: &QualityStatus) -> String {
        let mut notes = Vec::new();

        match status {
            QualityStatus::Incomplete => {
                notes.push("关键数据缺失，分析可能不准确。".to_string());
            }
            QualityStatus::Suspicious => {
                notes.push("频谱在约 18kHz 处存在硬性截止 (高度疑似伪造/升频)。".to_string());
            }
            QualityStatus::Processed => {
                notes.push("频谱在 18kHz 处能量较低，可能存在软性截止。".to_string());
            }
            QualityStatus::Clipped => {
                notes.push("存在严重数字削波风险 (峰值接近0dB)。".to_string());
            }
            QualityStatus::SeverelyCompressed => {
                if let Some(lra) = metrics.lra {
                    notes.push(format!("动态范围极低 (LRA: {lra:.1} LU)，严重过度压缩。"));
                }
            }
            QualityStatus::LowDynamic => {
                if let Some(lra) = metrics.lra {
                    notes.push(format!("动态范围过低 (LRA: {lra:.1} LU)，可能过度压缩。"));
                }
            }
            QualityStatus::LowBitrate => {
                if let Some(bitrate) = metrics.bitrate_kbps {
                    notes.push(format!("码率偏低 ({bitrate} kbps)，可能存在细节损失。"));
                }
            }
            QualityStatus::LowSampleRate => {
                if let Some(sample_rate) = metrics.sample_rate_hz {
                    notes.push(format!("采样率过低 ({sample_rate} Hz)，高频上限受限。"));
                }
            }
            QualityStatus::Mono => {
                notes.push("当前文件为单声道 (mono)。".to_string());
            }
            QualityStatus::Good => {
                // 检查是否有其他需要注意的问题
                if let Some(lra) = metrics.lra {
                    if lra > self.thresholds.lra_too_high {
                        notes.push(format!(
                            "动态范围过高 (LRA: {lra:.1} LU)，可能需要压缩处理。"
                        ));
                    }
                }
            }
        }

        if notes.is_empty() {
            "未发现明显的硬性技术问题。".to_string()
        } else {
            notes.join(" | ")
        }
    }

    /// 计算综合质量分数 (0-100)
    fn calculate_quality_score(&self, metrics: &FileMetrics, status: &QualityStatus) -> i32 {
        // 评分体系常量定义（用于文档和理解，实际计算中直接使用数值）
        #[allow(dead_code)]
        const MAX_SCORE_INTEGRITY: f64 = 40.0; // 完整性评分上限
        #[allow(dead_code)]
        const MAX_SCORE_DYNAMICS: f64 = 30.0; // 动态范围评分上限
        #[allow(dead_code)]
        const MAX_SCORE_SPECTRUM: f64 = 30.0; // 频谱评分上限

        let mut integrity_score = 0.0;
        let mut dynamics_score = 0.0;
        let mut spectrum_score = 0.0;

        // 计算完整性惩罚
        let completeness_penalty = self.count_missing_critical_fields(metrics) as f64 * 10.0;

        // 1. 完整性评分 (基于18kHz以上频段和峰值)
        integrity_score += self.calculate_integrity_score(metrics);

        // 2. 动态范围评分 (基于LRA)
        dynamics_score += self.calculate_dynamics_score(metrics);

        // 3. 频谱评分 (基于16kHz以上频段)
        spectrum_score += self.calculate_spectrum_score(metrics);

        // 计算总分
        let mut total_score =
            integrity_score + dynamics_score + spectrum_score - completeness_penalty;

        // 额外扣分规则（与 ffprobe 元数据联动）
        if self.is_lossy(metrics) && matches!(metrics.bitrate_kbps, Some(bitrate) if bitrate < 192)
        {
            total_score -= 30.0;
        }

        if self.is_lossy(metrics)
            && matches!(metrics.bitrate_kbps, Some(bitrate) if bitrate > 256)
            && matches!(metrics.rms_db_above_18k, Some(rms_18k) if rms_18k < self.thresholds.spectrum_processed_threshold)
        {
            total_score -= 25.0;
        }

        if matches!(metrics.sample_rate_hz, Some(sr) if sr < 44_100) {
            total_score -= 20.0;
        }

        if matches!(metrics.channels, Some(ch) if ch < 2) {
            total_score -= 5.0;
        }

        // 根据状态应用额外惩罚
        match status {
            QualityStatus::Suspicious => {
                total_score = total_score.min(20.0);
            }
            QualityStatus::Incomplete => {
                total_score = total_score.min(40.0);
            }
            _ => {}
        }

        // 确保分数在0-100范围内
        (total_score.max(0.0).round() as i32).min(100)
    }

    /// 计算完整性分数 (基于18kHz以上频段和峰值)
    fn calculate_integrity_score(&self, metrics: &FileMetrics) -> f64 {
        let mut score = 0.0;

        // 基于18kHz以上频段的评分
        if let Some(rms_18k) = metrics.rms_db_above_18k {
            if rms_18k != 0.0 {
                if rms_18k >= self.thresholds.spectrum_good_threshold {
                    score += 25.0;
                } else if rms_18k >= self.thresholds.spectrum_processed_threshold {
                    score += self.map_to_score(
                        rms_18k,
                        self.thresholds.spectrum_processed_threshold,
                        self.thresholds.spectrum_good_threshold,
                        15.0,
                        25.0,
                    );
                } else if rms_18k >= self.thresholds.spectrum_fake_threshold {
                    score += self.map_to_score(
                        rms_18k,
                        self.thresholds.spectrum_fake_threshold,
                        self.thresholds.spectrum_processed_threshold,
                        5.0,
                        15.0,
                    );
                }
            }
        }

        // 基于峰值的评分
        if let Some(peak_db) = metrics.peak_amplitude_db {
            if peak_db <= self.thresholds.peak_good_db {
                score += 15.0;
            } else if peak_db <= self.thresholds.peak_medium_db {
                score += self.map_to_score(
                    peak_db,
                    self.thresholds.peak_good_db,
                    self.thresholds.peak_medium_db,
                    15.0,
                    10.0,
                );
            } else if peak_db <= self.thresholds.peak_clipping_db {
                score += self.map_to_score(
                    peak_db,
                    self.thresholds.peak_medium_db,
                    self.thresholds.peak_clipping_db,
                    10.0,
                    3.0,
                );
            }
        }

        score
    }

    /// 计算动态范围分数 (基于LRA)
    fn calculate_dynamics_score(&self, metrics: &FileMetrics) -> f64 {
        if let Some(lra) = metrics.lra {
            if lra > 0.0 {
                // 理想动态范围 (8-12 LU)
                if lra >= self.thresholds.lra_excellent_min
                    && lra <= self.thresholds.lra_excellent_max
                {
                    return 30.0;
                }

                // 低可接受范围 (6-8 LU)
                if lra >= self.thresholds.lra_low_max && lra < self.thresholds.lra_excellent_min {
                    return self.map_to_score(
                        lra,
                        self.thresholds.lra_low_max,
                        self.thresholds.lra_excellent_min,
                        20.0,
                        28.0,
                    );
                }

                // 高可接受范围 (12-15 LU)
                if lra > self.thresholds.lra_excellent_max
                    && lra <= self.thresholds.lra_acceptable_max
                {
                    return self.map_to_score(
                        lra,
                        self.thresholds.lra_excellent_max,
                        self.thresholds.lra_acceptable_max,
                        28.0,
                        22.0,
                    );
                }

                // 低动态范围 (3-6 LU)
                if lra >= self.thresholds.lra_poor_max && lra < self.thresholds.lra_low_max {
                    return self.map_to_score(
                        lra,
                        self.thresholds.lra_poor_max,
                        self.thresholds.lra_low_max,
                        10.0,
                        20.0,
                    );
                }

                // 极低动态范围 (0-3 LU)
                if lra < self.thresholds.lra_poor_max {
                    return self.map_to_score(lra, 0.0, self.thresholds.lra_poor_max, 0.0, 10.0);
                }

                // 动态范围过高 (>15 LU)
                if lra > self.thresholds.lra_acceptable_max {
                    return 18.0;
                }
            }
        }
        0.0
    }

    /// 计算频谱分数 (基于16kHz以上频段)
    fn calculate_spectrum_score(&self, metrics: &FileMetrics) -> f64 {
        if let Some(rms_16k) = metrics.rms_db_above_16k {
            self.map_to_score(rms_16k, -90.0, -55.0, 0.0, 30.0)
        } else {
            0.0
        }
    }

    /// 将数值映射到指定分数范围
    fn map_to_score(
        &self,
        value: f64,
        in_min: f64,
        in_max: f64,
        out_min: f64,
        out_max: f64,
    ) -> f64 {
        if (in_max - in_min).abs() < f64::EPSILON {
            return out_min;
        }

        let clamped_value = value.clamp(in_min, in_max);
        out_min + (clamped_value - in_min) * (out_max - out_min) / (in_max - in_min)
    }

    fn is_lossless(&self, metrics: &FileMetrics) -> bool {
        let ext = Path::new(&metrics.file_path)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();

        let codec = metrics
            .codec_name
            .as_deref()
            .unwrap_or_default()
            .to_ascii_lowercase();

        let container = metrics
            .container_format
            .as_deref()
            .unwrap_or_default()
            .to_ascii_lowercase();

        let lossless_by_ext = matches!(ext.as_str(), "flac" | "alac" | "wav" | "aiff" | "aif");
        let lossless_by_codec = codec.starts_with("pcm_")
            || matches!(codec.as_str(), "flac" | "alac" | "wavpack" | "ape");
        let lossless_by_container =
            container.contains("flac") || container.contains("wav") || container.contains("aiff");

        lossless_by_ext || lossless_by_codec || lossless_by_container
    }

    fn is_lossy(&self, metrics: &FileMetrics) -> bool {
        if self.is_lossless(metrics) {
            return false;
        }

        let ext = Path::new(&metrics.file_path)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        let codec = metrics
            .codec_name
            .as_deref()
            .unwrap_or_default()
            .to_ascii_lowercase();

        let lossy_by_ext = matches!(ext.as_str(), "mp3" | "aac" | "m4a" | "ogg" | "opus" | "wma");
        let lossy_by_codec = matches!(
            codec.as_str(),
            "mp3" | "aac" | "vorbis" | "opus" | "wmav2" | "mp2" | "ac3"
        );

        lossy_by_ext || lossy_by_codec
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 创建测试用的FileMetrics实例
    fn create_test_metrics() -> FileMetrics {
        FileMetrics {
            file_path: "test.flac".to_string(),
            file_size_bytes: 1000000,
            lra: Some(8.5),
            peak_amplitude_db: Some(-3.0),
            overall_rms_db: Some(-18.0),
            rms_db_above_16k: Some(-60.0),
            rms_db_above_18k: Some(-75.0),
            rms_db_above_20k: Some(-85.0),
            processing_time_ms: 1000,
            sample_rate_hz: Some(44_100),
            bitrate_kbps: Some(900),
            channels: Some(2),
            codec_name: Some("flac".to_string()),
            container_format: Some("flac".to_string()),
            duration_seconds: Some(60.0),
            cache_hit: false,
            content_sha256: Some("abc".to_string()),
            error_codes: vec![],
        }
    }

    #[test]
    fn test_quality_thresholds_default() {
        let thresholds = QualityThresholds::default();
        assert_eq!(thresholds.spectrum_fake_threshold, -85.0);
        assert_eq!(thresholds.spectrum_processed_threshold, -80.0);
        assert_eq!(thresholds.spectrum_good_threshold, -70.0);
        assert_eq!(thresholds.lra_poor_max, 3.0);
        assert_eq!(thresholds.lra_excellent_min, 8.0);
        assert_eq!(thresholds.peak_clipping_db, -0.1);
    }

    #[test]
    fn test_quality_scorer_creation() {
        let scorer = QualityScorer::new();
        assert_eq!(scorer.thresholds.spectrum_fake_threshold, -85.0);

        let custom_thresholds = QualityThresholds {
            spectrum_fake_threshold: -90.0,
            ..Default::default()
        };
        let custom_scorer = QualityScorer::with_thresholds(custom_thresholds);
        assert_eq!(custom_scorer.thresholds.spectrum_fake_threshold, -90.0);
    }

    #[test]
    fn test_determine_status_good_quality() {
        let scorer = QualityScorer::new();
        let metrics = create_test_metrics();
        let status = scorer.determine_status(&metrics);
        assert_eq!(status, QualityStatus::Good);
    }

    #[test]
    fn test_determine_status_incomplete() {
        let scorer = QualityScorer::new();
        let mut metrics = create_test_metrics();
        metrics.lra = None;
        metrics.rms_db_above_18k = None;
        let status = scorer.determine_status(&metrics);
        assert_eq!(status, QualityStatus::Incomplete);
    }

    #[test]
    fn test_determine_status_suspicious() {
        let scorer = QualityScorer::new();
        let mut metrics = create_test_metrics();
        metrics.rms_db_above_18k = Some(-90.0); // Below fake threshold
        let status = scorer.determine_status(&metrics);
        assert_eq!(status, QualityStatus::Suspicious);
    }

    #[test]
    fn test_determine_status_clipped() {
        let scorer = QualityScorer::new();
        let mut metrics = create_test_metrics();
        metrics.peak_amplitude_db = Some(0.0); // At clipping threshold
        let status = scorer.determine_status(&metrics);
        assert_eq!(status, QualityStatus::Clipped);
    }

    #[test]
    fn test_determine_status_severely_compressed() {
        let scorer = QualityScorer::new();
        let mut metrics = create_test_metrics();
        metrics.lra = Some(2.0); // Below poor threshold
        let status = scorer.determine_status(&metrics);
        assert_eq!(status, QualityStatus::SeverelyCompressed);
    }

    #[test]
    fn test_count_missing_critical_fields() {
        let scorer = QualityScorer::new();
        let metrics = create_test_metrics();
        assert_eq!(scorer.count_missing_critical_fields(&metrics), 0);

        let mut incomplete_metrics = create_test_metrics();
        incomplete_metrics.lra = None;
        incomplete_metrics.rms_db_above_18k = None;
        assert_eq!(scorer.count_missing_critical_fields(&incomplete_metrics), 2);
    }

    #[test]
    fn test_map_to_score() {
        let scorer = QualityScorer::new();

        // Test normal mapping
        assert_eq!(scorer.map_to_score(5.0, 0.0, 10.0, 0.0, 100.0), 50.0);
        assert_eq!(scorer.map_to_score(0.0, 0.0, 10.0, 0.0, 100.0), 0.0);
        assert_eq!(scorer.map_to_score(10.0, 0.0, 10.0, 0.0, 100.0), 100.0);

        // Test clamping
        assert_eq!(scorer.map_to_score(-5.0, 0.0, 10.0, 0.0, 100.0), 0.0);
        assert_eq!(scorer.map_to_score(15.0, 0.0, 10.0, 0.0, 100.0), 100.0);

        // Test edge case where in_min == in_max
        assert_eq!(scorer.map_to_score(5.0, 5.0, 5.0, 0.0, 100.0), 0.0);
    }

    #[test]
    fn test_calculate_quality_score() {
        let scorer = QualityScorer::new();
        let metrics = create_test_metrics();
        let status = QualityStatus::Good;
        let score = scorer.calculate_quality_score(&metrics, &status);

        // Score should be reasonable for good quality audio
        assert!((70..=100).contains(&score));
    }

    #[test]
    fn test_analyze_file() {
        let scorer = QualityScorer::new();
        let metrics = create_test_metrics();
        let analysis = scorer.analyze_file(&metrics);

        assert_eq!(analysis.file_path, "test.flac");
        assert!(analysis.quality_score > 0);
        assert_eq!(analysis.status, QualityStatus::Good);
        assert!(!analysis.notes.is_empty());
    }

    #[test]
    fn test_analyze_files_batch() {
        let scorer = QualityScorer::new();
        let metrics_list = vec![create_test_metrics(), create_test_metrics()];
        let analyses = scorer.analyze_files(&metrics_list);

        assert_eq!(analyses.len(), 2);
        for analysis in &analyses {
            assert!(analysis.quality_score > 0);
        }
    }

    #[test]
    fn test_determine_status_low_bitrate() {
        let scorer = QualityScorer::new();
        let mut metrics = create_test_metrics();
        metrics.file_path = "test.mp3".to_string();
        metrics.codec_name = Some("mp3".to_string());
        metrics.container_format = Some("mp3".to_string());
        metrics.bitrate_kbps = Some(128);
        let status = scorer.determine_status(&metrics);
        assert_eq!(status, QualityStatus::LowBitrate);
    }

    #[test]
    fn test_determine_status_low_sample_rate() {
        let scorer = QualityScorer::new();
        let mut metrics = create_test_metrics();
        metrics.sample_rate_hz = Some(32_000);
        let status = scorer.determine_status(&metrics);
        assert_eq!(status, QualityStatus::LowSampleRate);
    }

    #[test]
    fn test_determine_status_mono() {
        let scorer = QualityScorer::new();
        let mut metrics = create_test_metrics();
        metrics.channels = Some(1);
        let status = scorer.determine_status(&metrics);
        assert_eq!(status, QualityStatus::Mono);
    }
}
