use super::metrics::FileMetrics;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScoringProfile {
    #[serde(rename = "pop")]
    Pop,
    #[serde(rename = "broadcast")]
    Broadcast,
    #[serde(rename = "archive")]
    Archive,
}

impl ScoringProfile {
    pub fn as_str(self) -> &'static str {
        match self {
            ScoringProfile::Pop => "pop",
            ScoringProfile::Broadcast => "broadcast",
            ScoringProfile::Archive => "archive",
        }
    }
}

impl FromStr for ScoringProfile {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "pop" | "kpop" | "jpop" | "apop" => Ok(ScoringProfile::Pop),
            "broadcast" => Ok(ScoringProfile::Broadcast),
            "archive" => Ok(ScoringProfile::Archive),
            _ => Err(format!(
                "不支持的 profile: {s}，可选: pop/broadcast/archive"
            )),
        }
    }
}

#[derive(Debug, Clone)]
struct ProfileConfig {
    target_lufs: f64,
    loudness_soft_range_low: f64,
    loudness_soft_range_high: f64,
    true_peak_warn: f64,
    true_peak_critical: f64,
    spectrum_fake_threshold: f64,
    spectrum_processed_threshold: f64,
    spectrum_good_threshold: f64,
    lra_poor_max: f64,
    lra_low_max: f64,
    lra_excellent_min: f64,
    lra_excellent_max: f64,
    lra_acceptable_max: f64,
    lra_too_high: f64,
    bitrate_low_kbps: u32,
    bitrate_high_kbps: u32,
}

impl ProfileConfig {
    fn from_profile(profile: ScoringProfile) -> Self {
        match profile {
            ScoringProfile::Pop => Self {
                target_lufs: -9.0,
                loudness_soft_range_low: -13.0,
                loudness_soft_range_high: -6.0,
                true_peak_warn: 0.1,
                true_peak_critical: 1.0,
                spectrum_fake_threshold: -85.0,
                spectrum_processed_threshold: -80.0,
                spectrum_good_threshold: -70.0,
                lra_poor_max: 3.0,
                lra_low_max: 5.0,
                lra_excellent_min: 5.5,
                lra_excellent_max: 10.0,
                lra_acceptable_max: 14.0,
                lra_too_high: 18.0,
                bitrate_low_kbps: 192,
                bitrate_high_kbps: 256,
            },
            ScoringProfile::Broadcast => Self {
                target_lufs: -23.0,
                loudness_soft_range_low: -25.0,
                loudness_soft_range_high: -22.0,
                true_peak_warn: -2.0,
                true_peak_critical: -1.0,
                spectrum_fake_threshold: -88.0,
                spectrum_processed_threshold: -82.0,
                spectrum_good_threshold: -72.0,
                lra_poor_max: 4.0,
                lra_low_max: 6.0,
                lra_excellent_min: 6.0,
                lra_excellent_max: 15.0,
                lra_acceptable_max: 20.0,
                lra_too_high: 24.0,
                bitrate_low_kbps: 192,
                bitrate_high_kbps: 256,
            },
            ScoringProfile::Archive => Self {
                target_lufs: -18.0,
                loudness_soft_range_low: -24.0,
                loudness_soft_range_high: -10.0,
                true_peak_warn: -0.5,
                true_peak_critical: -0.1,
                spectrum_fake_threshold: -85.0,
                spectrum_processed_threshold: -80.0,
                spectrum_good_threshold: -70.0,
                lra_poor_max: 2.5,
                lra_low_max: 4.0,
                lra_excellent_min: 5.0,
                lra_excellent_max: 14.0,
                lra_acceptable_max: 20.0,
                lra_too_high: 24.0,
                bitrate_low_kbps: 160,
                bitrate_high_kbps: 256,
            },
        }
    }
}

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
    #[serde(rename = "真峰值风险")]
    TruePeakRisk,
    #[serde(rename = "响度偏离目标")]
    LoudnessOffTarget,
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
            QualityStatus::TruePeakRisk => "真峰值风险",
            QualityStatus::LoudnessOffTarget => "响度偏离目标",
            QualityStatus::SeverelyCompressed => "严重压缩",
            QualityStatus::LowDynamic => "低动态",
            QualityStatus::LowBitrate => "低码率",
            QualityStatus::LowSampleRate => "低采样率",
            QualityStatus::Mono => "单声道",
        };
        write!(f, "{status_str}")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityAnalysis {
    #[serde(rename = "filePath")]
    pub file_path: String,
    #[serde(rename = "质量分")]
    pub quality_score: i32,
    #[serde(rename = "状态")]
    pub status: QualityStatus,
    #[serde(rename = "备注")]
    pub notes: String,
    #[serde(rename = "profile")]
    pub profile: String,
    #[serde(rename = "confidence")]
    pub confidence: f64,
    #[serde(flatten)]
    pub metrics: FileMetrics,
}

pub struct QualityScorer {
    profile: ScoringProfile,
    config: ProfileConfig,
}

impl QualityScorer {
    pub fn new() -> Self {
        Self::with_profile(ScoringProfile::Pop)
    }

    pub fn with_profile(profile: ScoringProfile) -> Self {
        Self {
            profile,
            config: ProfileConfig::from_profile(profile),
        }
    }

    pub fn analyze_file(&self, metrics: &FileMetrics) -> QualityAnalysis {
        let status = self.determine_status(metrics);
        let notes = self.generate_notes(metrics, &status);
        let quality_score = self.calculate_quality_score(metrics, &status);
        let confidence = self.estimate_confidence(metrics);

        QualityAnalysis {
            file_path: metrics.file_path.clone(),
            quality_score,
            status,
            notes,
            profile: self.profile.as_str().to_string(),
            confidence,
            metrics: metrics.clone(),
        }
    }

    pub fn analyze_files(&self, metrics_list: &[FileMetrics]) -> Vec<QualityAnalysis> {
        use rayon::prelude::*;

        if metrics_list.len() < 10 {
            metrics_list.iter().map(|m| self.analyze_file(m)).collect()
        } else {
            metrics_list
                .par_iter()
                .map(|m| self.analyze_file(m))
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
    fn determine_status(&self, metrics: &FileMetrics) -> QualityStatus {
        let critical_fields_missing = self.count_missing_critical_fields(metrics);
        if critical_fields_missing >= 2 {
            return QualityStatus::Incomplete;
        }

        if let Some(rms_18k) = metrics.rms_db_above_18k {
            if self.is_lossless(metrics) && rms_18k < self.config.spectrum_fake_threshold {
                return QualityStatus::Suspicious;
            }
            if rms_18k < self.config.spectrum_processed_threshold {
                return QualityStatus::Processed;
            }
        }

        if let Some(tp) = metrics.true_peak_dbtp {
            if tp >= self.config.true_peak_critical {
                return QualityStatus::Clipped;
            }
            if tp >= self.config.true_peak_warn {
                return QualityStatus::TruePeakRisk;
            }
        } else if matches!(metrics.peak_amplitude_db, Some(peak) if peak >= -0.1) {
            return QualityStatus::Clipped;
        }

        if let Some(i_lufs) = metrics.integrated_loudness_lufs {
            if i_lufs < self.config.loudness_soft_range_low
                || i_lufs > self.config.loudness_soft_range_high
            {
                return QualityStatus::LoudnessOffTarget;
            }
        }

        if self.is_lossy(metrics)
            && matches!(metrics.bitrate_kbps, Some(bitrate) if bitrate < self.config.bitrate_low_kbps)
        {
            return QualityStatus::LowBitrate;
        }

        if matches!(metrics.sample_rate_hz, Some(sr) if sr < 44_100) {
            return QualityStatus::LowSampleRate;
        }

        if matches!(metrics.channels, Some(ch) if ch < 2) {
            return QualityStatus::Mono;
        }

        if let Some(lra) = metrics.lra {
            if lra < self.config.lra_poor_max {
                return QualityStatus::SeverelyCompressed;
            }
            if lra < self.config.lra_low_max {
                return QualityStatus::LowDynamic;
            }
        }

        QualityStatus::Good
    }

    fn count_missing_critical_fields(&self, metrics: &FileMetrics) -> i32 {
        let mut missing_count = 0;

        if metrics.rms_db_above_18k.is_none() {
            missing_count += 1;
        }
        if metrics.lra.is_none() {
            missing_count += 1;
        }
        if metrics.integrated_loudness_lufs.is_none() {
            missing_count += 1;
        }
        if metrics.true_peak_dbtp.is_none() && metrics.peak_amplitude_db.is_none() {
            missing_count += 1;
        }

        missing_count
    }

    fn generate_notes(&self, metrics: &FileMetrics, status: &QualityStatus) -> String {
        let mut notes = Vec::new();
        notes.push(format!("评分档案: {}", self.profile.as_str()));

        match status {
            QualityStatus::Incomplete => {
                notes.push("关键数据缺失，分析置信度较低。".to_string());
            }
            QualityStatus::Suspicious => {
                notes.push("无损容器下高频能量异常，疑似有损升频来源。".to_string());
            }
            QualityStatus::Processed => {
                notes.push("高频能量偏低，可能存在软截止或后期处理。".to_string());
            }
            QualityStatus::Clipped => {
                if let Some(tp) = metrics.true_peak_dbtp {
                    notes.push(format!("真峰值过高 (TP: {tp:.2} dBTP)，存在削波风险。"));
                } else {
                    notes.push("峰值过高，存在削波风险。".to_string());
                }
            }
            QualityStatus::TruePeakRisk => {
                if let Some(tp) = metrics.true_peak_dbtp {
                    notes.push(format!("真峰值接近阈值 (TP: {tp:.2} dBTP)。"));
                }
            }
            QualityStatus::LoudnessOffTarget => {
                if let Some(i) = metrics.integrated_loudness_lufs {
                    notes.push(format!(
                        "综合响度偏离目标 (I: {i:.1} LUFS, target: {:.1} LUFS)。",
                        self.config.target_lufs
                    ));
                }
            }
            QualityStatus::SeverelyCompressed => {
                if let Some(lra) = metrics.lra {
                    notes.push(format!("动态范围极低 (LRA: {lra:.1} LU)。"));
                }
            }
            QualityStatus::LowDynamic => {
                if let Some(lra) = metrics.lra {
                    notes.push(format!("动态范围偏低 (LRA: {lra:.1} LU)。"));
                }
            }
            QualityStatus::LowBitrate => {
                if let Some(bitrate) = metrics.bitrate_kbps {
                    notes.push(format!("有损码率偏低 ({bitrate} kbps)。"));
                }
            }
            QualityStatus::LowSampleRate => {
                if let Some(sr) = metrics.sample_rate_hz {
                    notes.push(format!("采样率偏低 ({sr} Hz)。"));
                }
            }
            QualityStatus::Mono => {
                notes.push("当前文件为单声道。".to_string());
            }
            QualityStatus::Good => {
                notes.push("关键技术指标在目标范围内。".to_string());
            }
        }

        notes.join(" | ")
    }

    fn calculate_quality_score(&self, metrics: &FileMetrics, status: &QualityStatus) -> i32 {
        let compliance_score = self.calculate_compliance_score(metrics); // 35
        let dynamics_score = self.calculate_dynamics_score(metrics); // 20
        let spectrum_score = self.calculate_spectrum_score(metrics); // 25
        let authenticity_score = self.calculate_authenticity_score(metrics); // 10
        let integrity_score = self.calculate_integrity_score(metrics); // 10

        let mut total_score = compliance_score
            + dynamics_score
            + spectrum_score
            + authenticity_score
            + integrity_score;

        if self.is_lossy(metrics)
            && matches!(metrics.bitrate_kbps, Some(bitrate) if bitrate < self.config.bitrate_low_kbps)
        {
            total_score -= 12.0;
        }

        if self.is_lossy(metrics)
            && matches!(metrics.bitrate_kbps, Some(bitrate) if bitrate > self.config.bitrate_high_kbps)
            && matches!(metrics.rms_db_above_18k, Some(rms_18k) if rms_18k < self.config.spectrum_processed_threshold)
        {
            total_score -= 8.0;
        }

        if matches!(metrics.sample_rate_hz, Some(sr) if sr < 44_100) {
            total_score -= 10.0;
        }
        if matches!(metrics.channels, Some(ch) if ch < 2) {
            total_score -= 3.0;
        }

        match status {
            QualityStatus::Suspicious => total_score = total_score.min(25.0),
            QualityStatus::Incomplete => total_score = total_score.min(45.0),
            QualityStatus::Clipped => total_score = total_score.min(85.0),
            QualityStatus::TruePeakRisk => total_score = total_score.min(92.0),
            _ => {}
        }

        // Elite gate: 保留 90+ 的准入门槛，但对非 elite 曲目使用软压缩而非硬钉在 89。
        if total_score > 90.0 && !self.qualifies_for_elite_90(metrics, status) {
            total_score = self.compress_non_elite_high_score(total_score, metrics);
        }

        const HARD_MAX_SCORE: i32 = 99;
        (total_score.clamp(0.0, HARD_MAX_SCORE as f64).round() as i32).clamp(0, HARD_MAX_SCORE)
    }

    fn qualifies_for_elite_90(&self, metrics: &FileMetrics, status: &QualityStatus) -> bool {
        if *status != QualityStatus::Good {
            return false;
        }

        let Some(i_lufs) = metrics.integrated_loudness_lufs else {
            return false;
        };
        let Some(tp) = metrics.true_peak_dbtp else {
            return false;
        };
        let Some(lra) = metrics.lra else {
            return false;
        };
        let Some(rms18) = metrics.rms_db_above_18k else {
            return false;
        };

        let (elite_loudness_min, elite_loudness_max) = self.elite_loudness_range();
        let loudness_ok = (elite_loudness_min..=elite_loudness_max).contains(&i_lufs);

        let true_peak_ok = tp <= self.elite_true_peak_max();

        let (elite_lra_min, elite_lra_max) = self.elite_lra_range();
        let lra_ok = (elite_lra_min..=elite_lra_max).contains(&lra);

        let spectrum_ok = rms18 >= self.config.spectrum_processed_threshold;
        let bitrate_ok = if self.is_lossy(metrics) {
            matches!(metrics.bitrate_kbps, Some(b) if b >= self.config.bitrate_high_kbps)
        } else {
            true
        };

        loudness_ok && true_peak_ok && lra_ok && spectrum_ok && bitrate_ok
    }

    fn compress_non_elite_high_score(&self, raw_score: f64, metrics: &FileMetrics) -> f64 {
        let high_band_progress = ((raw_score - 90.0) / 9.0).clamp(0.0, 1.0);
        let elite_readiness = self.estimate_elite_readiness(metrics);

        // 将原本集中在 89 的分数拉开到 85-89 区间，提升高分段区分度。
        let compressed = 85.0 + high_band_progress * 2.0 + elite_readiness * 2.0;
        compressed.clamp(85.0, 89.0)
    }

    fn estimate_elite_readiness(&self, metrics: &FileMetrics) -> f64 {
        let (elite_loudness_min, elite_loudness_max) = self.elite_loudness_range();
        let loudness_score = metrics
            .integrated_loudness_lufs
            .map(|value| {
                self.soft_band_score(
                    value,
                    elite_loudness_min,
                    elite_loudness_max,
                    self.config.loudness_soft_range_low,
                    self.config.loudness_soft_range_high,
                )
            })
            .unwrap_or(0.0);

        let tp_score = metrics
            .true_peak_dbtp
            .map(|tp| {
                let elite_tp_max = self.elite_true_peak_max();
                let soft_tp_max = self.config.true_peak_critical.max(elite_tp_max + 0.6);
                if tp <= elite_tp_max {
                    1.0
                } else if tp <= soft_tp_max {
                    self.map_to_score(tp, elite_tp_max, soft_tp_max, 1.0, 0.0)
                } else {
                    0.0
                }
            })
            .unwrap_or_else(|| {
                metrics
                    .peak_amplitude_db
                    .map(|peak| {
                        if peak <= -1.0 {
                            0.9
                        } else if peak <= 0.0 {
                            self.map_to_score(peak, -1.0, 0.0, 0.9, 0.0)
                        } else {
                            0.0
                        }
                    })
                    .unwrap_or(0.0)
            });

        let (elite_lra_min, elite_lra_max) = self.elite_lra_range();
        let lra_score = metrics
            .lra
            .map(|value| {
                self.soft_band_score(
                    value,
                    elite_lra_min,
                    elite_lra_max,
                    self.config.lra_low_max,
                    self.config.lra_acceptable_max,
                )
            })
            .unwrap_or(0.0);

        let spectrum_score = metrics
            .rms_db_above_18k
            .map(|value| {
                if value >= self.config.spectrum_processed_threshold {
                    self.map_to_score(
                        value,
                        self.config.spectrum_processed_threshold,
                        self.config.spectrum_good_threshold,
                        0.7,
                        1.0,
                    )
                } else if value >= self.config.spectrum_fake_threshold {
                    self.map_to_score(
                        value,
                        self.config.spectrum_fake_threshold,
                        self.config.spectrum_processed_threshold,
                        0.0,
                        0.7,
                    )
                } else {
                    0.0
                }
            })
            .unwrap_or(0.0);

        let bitrate_score = if self.is_lossy(metrics) {
            metrics
                .bitrate_kbps
                .map(|bitrate| {
                    let bitrate = bitrate as f64;
                    if bitrate >= self.config.bitrate_high_kbps as f64 {
                        1.0
                    } else if bitrate >= self.config.bitrate_low_kbps as f64 {
                        self.map_to_score(
                            bitrate,
                            self.config.bitrate_low_kbps as f64,
                            self.config.bitrate_high_kbps as f64,
                            0.35,
                            1.0,
                        )
                    } else {
                        0.0
                    }
                })
                .unwrap_or(0.0)
        } else {
            1.0
        };

        let readiness = loudness_score * 0.26
            + tp_score * 0.20
            + lra_score * 0.22
            + spectrum_score * 0.20
            + bitrate_score * 0.12;
        readiness.clamp(0.0, 1.0)
    }

    fn elite_loudness_range(&self) -> (f64, f64) {
        match self.profile {
            ScoringProfile::Pop => (-10.5, -7.5),
            ScoringProfile::Broadcast => (-24.0, -22.0),
            ScoringProfile::Archive => (-20.0, -12.0),
        }
    }

    fn elite_true_peak_max(&self) -> f64 {
        match self.profile {
            ScoringProfile::Pop => -0.2,
            ScoringProfile::Broadcast => -1.0,
            ScoringProfile::Archive => -0.3,
        }
    }

    fn elite_lra_range(&self) -> (f64, f64) {
        match self.profile {
            ScoringProfile::Pop => (4.5, 11.0),
            ScoringProfile::Broadcast => (6.0, 15.0),
            ScoringProfile::Archive => (4.0, 16.0),
        }
    }

    fn soft_band_score(
        &self,
        value: f64,
        preferred_min: f64,
        preferred_max: f64,
        soft_min: f64,
        soft_max: f64,
    ) -> f64 {
        if value >= preferred_min && value <= preferred_max {
            return 1.0;
        }

        if value < preferred_min {
            if preferred_min <= soft_min {
                return 0.0;
            }
            if value >= soft_min {
                return self.map_to_score(value, soft_min, preferred_min, 0.0, 1.0);
            }
            return 0.0;
        }

        if preferred_max >= soft_max {
            return 0.0;
        }
        if value <= soft_max {
            return self.map_to_score(value, preferred_max, soft_max, 1.0, 0.0);
        }

        0.0
    }

    fn calculate_compliance_score(&self, metrics: &FileMetrics) -> f64 {
        let loudness_score = if let Some(i_lufs) = metrics.integrated_loudness_lufs {
            let delta = (i_lufs - self.config.target_lufs).abs();
            if delta <= 1.0 {
                20.0
            } else if delta <= 3.0 {
                self.map_to_score(delta, 1.0, 3.0, 20.0, 12.0)
            } else if delta <= 6.0 {
                self.map_to_score(delta, 3.0, 6.0, 12.0, 2.0)
            } else {
                0.0
            }
        } else {
            0.0
        };

        let peak_score = if let Some(tp) = metrics.true_peak_dbtp {
            if tp <= self.config.true_peak_warn {
                15.0
            } else if tp <= self.config.true_peak_critical {
                self.map_to_score(
                    tp,
                    self.config.true_peak_warn,
                    self.config.true_peak_critical,
                    15.0,
                    4.0,
                )
            } else {
                0.0
            }
        } else if let Some(peak) = metrics.peak_amplitude_db {
            if peak <= -1.0 {
                8.0
            } else if peak <= 0.0 {
                self.map_to_score(peak, -1.0, 0.0, 8.0, 2.0)
            } else {
                0.0
            }
        } else {
            0.0
        };

        loudness_score + peak_score
    }

    fn calculate_dynamics_score(&self, metrics: &FileMetrics) -> f64 {
        let Some(lra) = metrics.lra else { return 0.0 };

        if lra >= self.config.lra_excellent_min && lra <= self.config.lra_excellent_max {
            return 20.0;
        }
        if lra >= self.config.lra_low_max && lra < self.config.lra_excellent_min {
            return self.map_to_score(
                lra,
                self.config.lra_low_max,
                self.config.lra_excellent_min,
                12.0,
                19.0,
            );
        }
        if lra > self.config.lra_excellent_max && lra <= self.config.lra_acceptable_max {
            return self.map_to_score(
                lra,
                self.config.lra_excellent_max,
                self.config.lra_acceptable_max,
                19.0,
                13.0,
            );
        }
        if lra >= self.config.lra_poor_max && lra < self.config.lra_low_max {
            return self.map_to_score(
                lra,
                self.config.lra_poor_max,
                self.config.lra_low_max,
                5.0,
                12.0,
            );
        }
        if lra > self.config.lra_too_high {
            return 10.0;
        }
        self.map_to_score(lra, 0.0, self.config.lra_poor_max, 0.0, 5.0)
    }

    fn calculate_spectrum_score(&self, metrics: &FileMetrics) -> f64 {
        let score_16k = metrics
            .rms_db_above_16k
            .map(|v| self.map_to_score(v, -95.0, -55.0, 0.0, 15.0))
            .unwrap_or(0.0);

        let score_18k = metrics
            .rms_db_above_18k
            .map(|v| {
                if v >= self.config.spectrum_good_threshold {
                    10.0
                } else if v >= self.config.spectrum_processed_threshold {
                    self.map_to_score(
                        v,
                        self.config.spectrum_processed_threshold,
                        self.config.spectrum_good_threshold,
                        6.0,
                        10.0,
                    )
                } else if v >= self.config.spectrum_fake_threshold {
                    self.map_to_score(
                        v,
                        self.config.spectrum_fake_threshold,
                        self.config.spectrum_processed_threshold,
                        2.0,
                        6.0,
                    )
                } else {
                    0.0
                }
            })
            .unwrap_or(0.0);

        score_16k + score_18k
    }

    fn calculate_authenticity_score(&self, metrics: &FileMetrics) -> f64 {
        let mut score: f64 = 10.0;
        if self.is_lossless(metrics)
            && matches!(metrics.rms_db_above_18k, Some(v) if v < self.config.spectrum_fake_threshold)
        {
            score = 0.0;
        } else if matches!(metrics.rms_db_above_18k, Some(v) if v < self.config.spectrum_processed_threshold)
        {
            score = 4.0;
        }

        if self.is_lossy(metrics)
            && matches!(metrics.bitrate_kbps, Some(b) if b >= self.config.bitrate_high_kbps)
            && matches!(metrics.rms_db_above_18k, Some(v) if v < self.config.spectrum_processed_threshold)
        {
            score -= 2.0;
        }

        score.max(0.0)
    }

    fn calculate_integrity_score(&self, metrics: &FileMetrics) -> f64 {
        let missing = self.count_missing_critical_fields(metrics) as f64;
        let mut score = (10.0 - missing * 3.0).max(0.0);
        if !metrics.error_codes.is_empty() {
            score = (score - 2.0_f64.min(metrics.error_codes.len() as f64)).max(0.0);
        }
        score
    }

    fn estimate_confidence(&self, metrics: &FileMetrics) -> f64 {
        let missing = self.count_missing_critical_fields(metrics) as f64;
        let mut confidence = 1.0 - missing * 0.18;
        if !metrics.error_codes.is_empty() {
            confidence -= 0.08 * metrics.error_codes.len() as f64;
        }
        confidence.clamp(0.1, 1.0)
    }

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

    fn create_test_metrics() -> FileMetrics {
        FileMetrics {
            file_path: "test.flac".to_string(),
            file_size_bytes: 1_000_000,
            lra: Some(8.5),
            peak_amplitude_db: Some(-1.5),
            overall_rms_db: Some(-18.0),
            rms_db_above_16k: Some(-60.0),
            rms_db_above_18k: Some(-75.0),
            rms_db_above_20k: Some(-85.0),
            integrated_loudness_lufs: Some(-9.5),
            true_peak_dbtp: Some(-1.2),
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
    fn test_profile_parse() {
        assert_eq!(
            ScoringProfile::from_str("pop").ok(),
            Some(ScoringProfile::Pop)
        );
        assert_eq!(
            ScoringProfile::from_str("kpop").ok(),
            Some(ScoringProfile::Pop)
        );
        assert!(ScoringProfile::from_str("unknown").is_err());
    }

    #[test]
    fn test_default_profile_is_pop() {
        let scorer = QualityScorer::new();
        assert_eq!(scorer.profile, ScoringProfile::Pop);
    }

    #[test]
    fn test_determine_status_good_quality() {
        let scorer = QualityScorer::new();
        let metrics = create_test_metrics();
        let status = scorer.determine_status(&metrics);
        assert_eq!(status, QualityStatus::Good);
    }

    #[test]
    fn test_determine_status_loudness_off_target() {
        let scorer = QualityScorer::new();
        let mut metrics = create_test_metrics();
        metrics.integrated_loudness_lufs = Some(-4.0);
        let status = scorer.determine_status(&metrics);
        assert_eq!(status, QualityStatus::LoudnessOffTarget);
    }

    #[test]
    fn test_determine_status_true_peak_risk() {
        let scorer = QualityScorer::new();
        let mut metrics = create_test_metrics();
        metrics.true_peak_dbtp = Some(0.3);
        let status = scorer.determine_status(&metrics);
        assert_eq!(status, QualityStatus::TruePeakRisk);
    }

    #[test]
    fn test_determine_status_clipped() {
        let scorer = QualityScorer::new();
        let mut metrics = create_test_metrics();
        metrics.true_peak_dbtp = Some(1.2);
        let status = scorer.determine_status(&metrics);
        assert_eq!(status, QualityStatus::Clipped);
    }

    #[test]
    fn test_determine_status_low_bitrate() {
        let scorer = QualityScorer::new();
        let mut metrics = create_test_metrics();
        metrics.file_path = "test.mp3".to_string();
        metrics.codec_name = Some("mp3".to_string());
        metrics.container_format = Some("mp3".to_string());
        metrics.bitrate_kbps = Some(128);
        metrics.integrated_loudness_lufs = Some(-9.5);
        metrics.true_peak_dbtp = Some(-2.0);
        let status = scorer.determine_status(&metrics);
        assert_eq!(status, QualityStatus::LowBitrate);
    }

    #[test]
    fn test_determine_status_incomplete() {
        let scorer = QualityScorer::new();
        let mut metrics = create_test_metrics();
        metrics.lra = None;
        metrics.integrated_loudness_lufs = None;
        let status = scorer.determine_status(&metrics);
        assert_eq!(status, QualityStatus::Incomplete);
    }

    #[test]
    fn test_calculate_quality_score() {
        let scorer = QualityScorer::new();
        let metrics = create_test_metrics();
        let status = QualityStatus::Good;
        let score = scorer.calculate_quality_score(&metrics, &status);
        assert!((70..=99).contains(&score));
    }

    #[test]
    fn test_non_elite_high_scores_are_soft_compressed() {
        let scorer = QualityScorer::new();
        let mut metrics = create_test_metrics();
        metrics.true_peak_dbtp = Some(0.3);
        let status = scorer.determine_status(&metrics);
        assert_eq!(status, QualityStatus::TruePeakRisk);

        let score = scorer.calculate_quality_score(&metrics, &status);
        assert!((85..=89).contains(&score));
    }

    #[test]
    fn test_elite_track_can_stay_in_90_plus() {
        let scorer = QualityScorer::new();
        let metrics = create_test_metrics();
        let status = scorer.determine_status(&metrics);
        assert_eq!(status, QualityStatus::Good);

        let score = scorer.calculate_quality_score(&metrics, &status);
        assert!(score >= 90);
    }

    #[test]
    fn test_analyze_file() {
        let scorer = QualityScorer::new();
        let metrics = create_test_metrics();
        let analysis = scorer.analyze_file(&metrics);

        assert_eq!(analysis.file_path, "test.flac");
        assert!(analysis.quality_score > 0);
        assert_eq!(analysis.status, QualityStatus::Good);
        assert_eq!(analysis.profile, "pop");
        assert!(analysis.confidence > 0.8);
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
}
