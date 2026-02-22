use anyhow::{Context, Result};
use csv::WriterBuilder;
use serde::Serialize;
use serde_json::json;
use std::path::Path;

use super::safe_io;
use super::scoring::QualityAnalysis;

pub struct ReportGenerator {
    safe_mode: bool,
}

impl ReportGenerator {
    pub fn new(safe_mode: bool) -> Self {
        Self { safe_mode }
    }

    pub fn generate_csv_report<P: AsRef<Path>>(
        &self,
        analyses: &[QualityAnalysis],
        output_path: P,
    ) -> Result<()> {
        let mut buffer: Vec<u8> = Vec::new();
        {
            let mut writer = WriterBuilder::new()
                .has_headers(true)
                .from_writer(&mut buffer);

            let mut sorted_analyses = analyses.to_vec();
            sorted_analyses.sort_by(|a, b| b.quality_score.cmp(&a.quality_score));

            for analysis in &sorted_analyses {
                let csv_record = CsvRecord::from_analysis(analysis);
                writer.serialize(&csv_record).context("写入CSV记录失败")?;
            }

            writer.flush().context("刷新CSV缓冲失败")?;
        }

        safe_io::atomic_write_bytes(output_path.as_ref(), &buffer, self.safe_mode)?;
        println!("✅ CSV报告已保存到: {}", output_path.as_ref().display());
        Ok(())
    }

    pub fn generate_jsonl_report<P: AsRef<Path>>(
        &self,
        analyses: &[QualityAnalysis],
        output_path: P,
    ) -> Result<()> {
        let mut output = String::new();
        for analysis in analyses {
            let line = serde_json::to_string(analysis).context("序列化JSONL记录失败")?;
            output.push_str(&line);
            output.push('\n');
        }

        safe_io::atomic_write_string(output_path.as_ref(), &output, self.safe_mode)?;
        println!("✅ JSONL报告已保存到: {}", output_path.as_ref().display());
        Ok(())
    }

    pub fn generate_sarif_report<P: AsRef<Path>>(
        &self,
        analyses: &[QualityAnalysis],
        output_path: P,
    ) -> Result<()> {
        let results: Vec<_> = analyses
            .iter()
            .map(|analysis| {
                json!({
                    "ruleId": format!("audioquality/{}", analysis.status),
                    "level": map_sarif_level(analysis.quality_score),
                    "message": { "text": format!("{} | 分数: {} | {}", analysis.status, analysis.quality_score, analysis.notes) },
                    "locations": [{
                        "physicalLocation": {
                            "artifactLocation": {
                                "uri": analysis.file_path
                            }
                        }
                    }],
                })
            })
            .collect();

        let sarif = json!({
            "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
            "version": "2.1.0",
            "runs": [{
                "tool": {
                    "driver": {
                        "name": "AudioQuality-rs",
                        "informationUri": "https://example.invalid/audioquality-rs",
                    }
                },
                "results": results
            }]
        });

        let content = serde_json::to_string_pretty(&sarif).context("序列化SARIF失败")?;
        safe_io::atomic_write_string(output_path.as_ref(), &content, self.safe_mode)?;
        println!("✅ SARIF报告已保存到: {}", output_path.as_ref().display());
        Ok(())
    }

    pub fn display_summary(&self, analyses: &[QualityAnalysis]) {
        if analyses.is_empty() {
            println!("没有可显示的分析结果。");
            return;
        }

        println!("\n--- 📊 质量分析摘要 ---");
        self.display_status_distribution(analyses);
        self.display_top_rankings(analyses, 10);
        self.display_statistics(analyses);
    }

    fn display_status_distribution(&self, analyses: &[QualityAnalysis]) {
        use std::collections::HashMap;

        let mut status_counts: HashMap<String, usize> = HashMap::new();
        for analysis in analyses {
            let status_str = analysis.status.to_string();
            *status_counts.entry(status_str).or_insert(0) += 1;
        }

        println!("\n📈 质量状态分布:");
        for (status, count) in &status_counts {
            let percentage = (*count as f64 / analyses.len() as f64) * 100.0;
            println!(" - {status}: {count} 个文件 ({percentage:.1}%)");
        }
    }

    fn display_top_rankings(&self, analyses: &[QualityAnalysis], top_n: usize) {
        let mut sorted_analyses = analyses.to_vec();
        sorted_analyses.sort_by(|a, b| b.quality_score.cmp(&a.quality_score));

        let display_count = top_n.min(sorted_analyses.len());
        println!("\n🏆 质量排名前 {display_count} 的文件:");

        for (i, analysis) in sorted_analyses.iter().take(display_count).enumerate() {
            let filename = Path::new(&analysis.file_path)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("Unknown");
            let filename = sanitize_for_terminal(filename);

            println!(
                " {}. [分数: {}] [状态: {}] {}",
                i + 1,
                analysis.quality_score,
                analysis.status,
                filename
            );
        }
    }

    fn display_statistics(&self, analyses: &[QualityAnalysis]) {
        let scores: Vec<i32> = analyses.iter().map(|a| a.quality_score).collect();

        if !scores.is_empty() {
            let total_files = analyses.len();
            let avg_score = scores.iter().sum::<i32>() as f64 / total_files as f64;
            let max_score = scores.iter().copied().max().unwrap_or(0);
            let min_score = scores.iter().copied().min().unwrap_or(0);

            println!("\n📊 分数统计:");
            println!(" - 总文件数: {total_files}");
            println!(" - 平均分数: {avg_score:.1}");
            println!(" - 最高分数: {max_score}");
            println!(" - 最低分数: {min_score}");
        }
    }
}

impl Default for ReportGenerator {
    fn default() -> Self {
        Self::new(true)
    }
}

#[derive(Debug, Serialize)]
struct CsvRecord {
    #[serde(rename = "质量分")]
    quality_score: i32,
    #[serde(rename = "状态")]
    status: String,
    #[serde(rename = "评分档案")]
    profile: String,
    #[serde(rename = "置信度")]
    confidence: f64,
    #[serde(rename = "文件路径")]
    file_path: String,
    #[serde(rename = "备注")]
    notes: String,
    #[serde(rename = "响度范围(LRA)")]
    lra: Option<f64>,
    #[serde(rename = "峰值电平(dB)")]
    peak_amplitude_db: Option<f64>,
    #[serde(rename = "整体RMS(dB)")]
    overall_rms_db: Option<f64>,
    #[serde(rename = "16kHz以上RMS(dB)")]
    rms_db_above_16k: Option<f64>,
    #[serde(rename = "18kHz以上RMS(dB)")]
    rms_db_above_18k: Option<f64>,
    #[serde(rename = "20kHz以上RMS(dB)")]
    rms_db_above_20k: Option<f64>,
    #[serde(rename = "综合响度(LUFS)")]
    integrated_loudness_lufs: Option<f64>,
    #[serde(rename = "真峰值(dBTP)")]
    true_peak_dbtp: Option<f64>,
    #[serde(rename = "采样率(Hz)")]
    sample_rate_hz: Option<u32>,
    #[serde(rename = "码率(kbps)")]
    bitrate_kbps: Option<u32>,
    #[serde(rename = "声道数")]
    channels: Option<u32>,
    #[serde(rename = "编码器")]
    codec_name: Option<String>,
    #[serde(rename = "容器格式")]
    container_format: Option<String>,
    #[serde(rename = "时长(秒)")]
    duration_seconds: Option<f64>,
    #[serde(rename = "缓存命中")]
    cache_hit: bool,
    #[serde(rename = "错误码")]
    error_codes: String,
    #[serde(rename = "文件大小(字节)")]
    file_size_bytes: u64,
    #[serde(rename = "处理时间(毫秒)")]
    processing_time_ms: u64,
}

impl CsvRecord {
    fn from_analysis(analysis: &QualityAnalysis) -> Self {
        Self {
            quality_score: analysis.quality_score,
            status: analysis.status.to_string(),
            profile: analysis.profile.clone(),
            confidence: analysis.confidence,
            file_path: analysis.file_path.clone(),
            notes: analysis.notes.clone(),
            lra: analysis.metrics.lra,
            peak_amplitude_db: analysis.metrics.peak_amplitude_db,
            overall_rms_db: analysis.metrics.overall_rms_db,
            rms_db_above_16k: analysis.metrics.rms_db_above_16k,
            rms_db_above_18k: analysis.metrics.rms_db_above_18k,
            rms_db_above_20k: analysis.metrics.rms_db_above_20k,
            integrated_loudness_lufs: analysis.metrics.integrated_loudness_lufs,
            true_peak_dbtp: analysis.metrics.true_peak_dbtp,
            sample_rate_hz: analysis.metrics.sample_rate_hz,
            bitrate_kbps: analysis.metrics.bitrate_kbps,
            channels: analysis.metrics.channels,
            codec_name: analysis.metrics.codec_name.clone(),
            container_format: analysis.metrics.container_format.clone(),
            duration_seconds: analysis.metrics.duration_seconds,
            cache_hit: analysis.metrics.cache_hit,
            error_codes: analysis.metrics.error_codes.join("|"),
            file_size_bytes: analysis.metrics.file_size_bytes,
            processing_time_ms: analysis.metrics.processing_time_ms,
        }
    }
}

fn map_sarif_level(score: i32) -> &'static str {
    if score >= 90 {
        "note"
    } else if score >= 70 {
        "warning"
    } else {
        "error"
    }
}

fn sanitize_for_terminal(input: &str) -> String {
    input
        .chars()
        .filter(|ch| {
            let c = *ch as u32;
            c == 0x09 || c == 0x20 || (0x21..=0x7e).contains(&c) || c >= 0xa0
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzer::metrics::FileMetrics;
    use crate::analyzer::scoring::QualityStatus;
    use tempfile::NamedTempFile;

    fn create_test_analysis() -> QualityAnalysis {
        let metrics = FileMetrics {
            file_path: "test.flac".to_string(),
            file_size_bytes: 1_000_000,
            lra: Some(8.5),
            peak_amplitude_db: Some(-3.0),
            overall_rms_db: Some(-18.0),
            rms_db_above_16k: Some(-60.0),
            rms_db_above_18k: Some(-75.0),
            rms_db_above_20k: Some(-85.0),
            integrated_loudness_lufs: Some(-14.2),
            true_peak_dbtp: Some(-1.2),
            processing_time_ms: 1000,
            sample_rate_hz: Some(44_100),
            bitrate_kbps: Some(320),
            channels: Some(2),
            codec_name: Some("flac".to_string()),
            container_format: Some("flac".to_string()),
            duration_seconds: Some(123.0),
            cache_hit: false,
            content_sha256: Some("abc".to_string()),
            error_codes: vec![],
        };

        QualityAnalysis {
            file_path: "test.flac".to_string(),
            quality_score: 85,
            status: QualityStatus::Good,
            notes: "未发现明显的硬性技术问题。".to_string(),
            profile: "pop".to_string(),
            confidence: 1.0,
            metrics,
        }
    }

    #[test]
    fn test_report_generator_creation() {
        let generator = ReportGenerator::new(true);
        assert_eq!(
            std::mem::size_of_val(&generator),
            std::mem::size_of::<ReportGenerator>()
        );
    }

    #[test]
    fn test_csv_record_from_analysis() {
        let analysis = create_test_analysis();
        let csv_record = CsvRecord::from_analysis(&analysis);

        assert_eq!(csv_record.quality_score, 85);
        assert_eq!(csv_record.status, "质量良好");
        assert_eq!(csv_record.file_path, "test.flac");
        assert_eq!(csv_record.lra, Some(8.5));
        assert_eq!(csv_record.peak_amplitude_db, Some(-3.0));
        assert_eq!(csv_record.sample_rate_hz, Some(44_100));
    }

    #[test]
    fn test_generate_csv_report() {
        let generator = ReportGenerator::new(true);
        let analyses = vec![create_test_analysis()];

        let temp_file = NamedTempFile::new().expect("failed to create temp file");
        let result = generator.generate_csv_report(&analyses, temp_file.path());

        assert!(result.is_ok());

        let content =
            std::fs::read_to_string(temp_file.path()).expect("failed to read generated csv");
        assert!(content.contains("质量分"));
        assert!(content.contains("状态"));
        assert!(content.contains("test.flac"));
        assert!(content.contains("采样率(Hz)"));
    }

    #[test]
    fn test_generate_jsonl_report() {
        let generator = ReportGenerator::new(true);
        let analyses = vec![create_test_analysis()];
        let temp_file = NamedTempFile::new().expect("failed to create temp file");

        let result = generator.generate_jsonl_report(&analyses, temp_file.path());
        assert!(result.is_ok());

        let content =
            std::fs::read_to_string(temp_file.path()).expect("failed to read generated jsonl");
        assert!(content.contains("\"质量分\":85"));
    }

    #[test]
    fn test_generate_sarif_report() {
        let generator = ReportGenerator::new(true);
        let analyses = vec![create_test_analysis()];
        let temp_file = NamedTempFile::new().expect("failed to create temp file");

        let result = generator.generate_sarif_report(&analyses, temp_file.path());
        assert!(result.is_ok());

        let content =
            std::fs::read_to_string(temp_file.path()).expect("failed to read generated sarif");
        assert!(content.contains("\"version\": \"2.1.0\""));
        assert!(content.contains("AudioQuality-rs"));
    }

    #[test]
    fn test_display_summary() {
        let generator = ReportGenerator::new(true);
        let analyses = vec![create_test_analysis()];
        generator.display_summary(&analyses);
    }

    #[test]
    fn test_display_summary_empty() {
        let generator = ReportGenerator::new(true);
        let analyses = vec![];
        generator.display_summary(&analyses);
    }
}
