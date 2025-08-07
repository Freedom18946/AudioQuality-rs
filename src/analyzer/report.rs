// ----------------------------------------------------------------
// 项目: 音频质量分析器 (Audio Quality Analyzer)
// 模块: analyzer/report.rs
// 描述: 此模块负责生成CSV报告和排名显示功能。
//      它将质量分析结果格式化为用户友好的报告格式。
// ----------------------------------------------------------------

use anyhow::{Context, Result};
use csv::WriterBuilder;
use serde::Serialize;
use std::fs::File;
use std::path::Path;

use super::scoring::QualityAnalysis;

/// CSV报告生成器
pub struct ReportGenerator;

impl ReportGenerator {
    /// 创建新的报告生成器实例
    pub fn new() -> Self {
        Self
    }
    
    /// 生成CSV报告文件
    pub fn generate_csv_report<P: AsRef<Path>>(
        &self,
        analyses: &[QualityAnalysis],
        output_path: P,
    ) -> Result<()> {
        let file = File::create(&output_path)
            .with_context(|| format!("无法创建CSV文件: {}", output_path.as_ref().display()))?;
        
        let mut writer = WriterBuilder::new()
            .has_headers(true)
            .from_writer(file);
        
        // 按质量分数降序排序
        let mut sorted_analyses = analyses.to_vec();
        sorted_analyses.sort_by(|a, b| b.quality_score.cmp(&a.quality_score));
        
        // 写入CSV数据
        for analysis in &sorted_analyses {
            let csv_record = CsvRecord::from_analysis(analysis);
            writer.serialize(&csv_record)
                .context("写入CSV记录失败")?;
        }
        
        writer.flush()
            .context("刷新CSV文件失败")?;
        
        println!("✅ CSV报告已保存到: {}", output_path.as_ref().display());
        Ok(())
    }
    
    /// 显示质量分析摘要
    pub fn display_summary(&self, analyses: &[QualityAnalysis]) {
        if analyses.is_empty() {
            println!("没有可显示的分析结果。");
            return;
        }
        
        println!("\n--- 📊 质量分析摘要 ---");
        
        // 统计各状态的文件数量
        self.display_status_distribution(analyses);
        
        // 显示前10名
        self.display_top_rankings(analyses, 10);
        
        // 显示统计信息
        self.display_statistics(analyses);
    }
    
    /// 显示状态分布
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
    
    /// 显示前N名排名
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
            
            println!(
                " {}. [分数: {}] [状态: {}] {}",
                i + 1,
                analysis.quality_score,
                analysis.status,
                filename
            );
        }
    }
    
    /// 显示统计信息
    fn display_statistics(&self, analyses: &[QualityAnalysis]) {
        let scores: Vec<i32> = analyses.iter().map(|a| a.quality_score).collect();
        
        if !scores.is_empty() {
            let total_files = analyses.len();
            let avg_score = scores.iter().sum::<i32>() as f64 / total_files as f64;
            let max_score = *scores.iter().max().unwrap();
            let min_score = *scores.iter().min().unwrap();
            
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
        Self::new()
    }
}

/// CSV记录结构体，用于序列化到CSV文件
#[derive(Debug, Serialize)]
struct CsvRecord {
    #[serde(rename = "质量分")]
    quality_score: i32,
    
    #[serde(rename = "状态")]
    status: String,
    
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
    
    #[serde(rename = "文件大小(字节)")]
    file_size_bytes: u64,
    
    #[serde(rename = "处理时间(毫秒)")]
    processing_time_ms: u64,
}

impl CsvRecord {
    /// 从质量分析结果创建CSV记录
    fn from_analysis(analysis: &QualityAnalysis) -> Self {
        Self {
            quality_score: analysis.quality_score,
            status: analysis.status.to_string(),
            file_path: analysis.file_path.clone(),
            notes: analysis.notes.clone(),
            lra: analysis.metrics.lra,
            peak_amplitude_db: analysis.metrics.peak_amplitude_db,
            overall_rms_db: analysis.metrics.overall_rms_db,
            rms_db_above_16k: analysis.metrics.rms_db_above_16k,
            rms_db_above_18k: analysis.metrics.rms_db_above_18k,
            rms_db_above_20k: analysis.metrics.rms_db_above_20k,
            file_size_bytes: analysis.metrics.file_size_bytes,
            processing_time_ms: analysis.metrics.processing_time_ms,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzer::scoring::QualityStatus;
    use crate::analyzer::metrics::FileMetrics;
    use tempfile::NamedTempFile;

    fn create_test_analysis() -> QualityAnalysis {
        let metrics = FileMetrics {
            file_path: "test.flac".to_string(),
            file_size_bytes: 1000000,
            lra: Some(8.5),
            peak_amplitude_db: Some(-3.0),
            overall_rms_db: Some(-18.0),
            rms_db_above_16k: Some(-60.0),
            rms_db_above_18k: Some(-75.0),
            rms_db_above_20k: Some(-85.0),
            processing_time_ms: 1000,
        };

        QualityAnalysis {
            file_path: "test.flac".to_string(),
            quality_score: 85,
            status: QualityStatus::Good,
            notes: "未发现明显的硬性技术问题。".to_string(),
            metrics,
        }
    }

    #[test]
    fn test_report_generator_creation() {
        let generator = ReportGenerator::new();
        // Just test that it can be created
        assert_eq!(std::mem::size_of_val(&generator), 0); // Zero-sized struct
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
    }

    #[test]
    fn test_generate_csv_report() {
        let generator = ReportGenerator::new();
        let analyses = vec![create_test_analysis()];

        let temp_file = NamedTempFile::new().unwrap();
        let result = generator.generate_csv_report(&analyses, temp_file.path());

        assert!(result.is_ok());

        // Verify file was created and has content
        let content = std::fs::read_to_string(temp_file.path()).unwrap();
        assert!(content.contains("质量分"));
        assert!(content.contains("状态"));
        assert!(content.contains("test.flac"));
    }

    #[test]
    fn test_display_summary() {
        let generator = ReportGenerator::new();
        let analyses = vec![create_test_analysis()];

        // This test just ensures the function doesn't panic
        // In a real scenario, you might want to capture stdout to verify output
        generator.display_summary(&analyses);
    }

    #[test]
    fn test_display_summary_empty() {
        let generator = ReportGenerator::new();
        let analyses = vec![];

        // This should handle empty input gracefully
        generator.display_summary(&analyses);
    }
}
