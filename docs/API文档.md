# AudioQuality-rs API 文档

## 概述

本文档详细描述了 AudioQuality-rs 项目中所有公共 API 的使用方法和参数说明。

## 核心数据结构

### FileMetrics

音频文件技术指标的核心数据结构。

```rust
pub struct FileMetrics {
    pub file_path: String,              // 文件完整路径
    pub file_size_bytes: u64,           // 文件大小（字节）
    pub lra: Option<f64>,               // 响度范围 (LU)
    pub peak_amplitude_db: Option<f64>, // 峰值电平 (dB)
    pub overall_rms_db: Option<f64>,    // 整体RMS电平 (dB)
    pub rms_db_above_16k: Option<f64>,  // 16kHz以上RMS (dB)
    pub rms_db_above_18k: Option<f64>,  // 18kHz以上RMS (dB)
    pub rms_db_above_20k: Option<f64>,  // 20kHz以上RMS (dB)
    pub processing_time_ms: u64,        // 处理时间 (毫秒)
}
```

**字段说明**：
- `lra`: 基于EBU R128标准的响度范围，用于评估动态范围
- `peak_amplitude_db`: 音频样本的最大绝对值，用于检测削波
- `rms_db_above_*`: 高频段能量，用于检测假无损音频

### QualityStatus

音频质量状态枚举。

```rust
pub enum QualityStatus {
    Good,                    // 质量良好
    Incomplete,              // 数据不完整
    Suspicious,              // 可疑(伪造)
    Processed,               // 疑似处理
    Clipped,                 // 已削波
    SeverelyCompressed,      // 严重压缩
    LowDynamic,              // 低动态
}
```

### QualityAnalysis

质量分析结果的完整数据结构。

```rust
pub struct QualityAnalysis {
    pub file_path: String,           // 文件路径
    pub quality_score: i32,          // 质量分数 (0-100)
    pub status: QualityStatus,       // 质量状态
    pub notes: String,               // 分析备注
    pub metrics: FileMetrics,        // 原始技术指标
}
```

## FFmpeg 交互模块

### process_file

处理单个音频文件并提取所有技术指标。

```rust
pub fn process_file(path: &Path, ffmpeg_path: &Path) -> Result<FileMetrics>
```

**参数**：
- `path`: 音频文件路径
- `ffmpeg_path`: FFmpeg可执行文件路径

**返回值**：
- `Result<FileMetrics>`: 成功时返回文件指标，失败时返回错误

**使用示例**：
```rust
use std::path::Path;
use crate::analyzer::ffmpeg;

let audio_path = Path::new("audio.flac");
let ffmpeg_path = Path::new("/usr/bin/ffmpeg");

match ffmpeg::process_file(audio_path, ffmpeg_path) {
    Ok(metrics) => println!("LRA: {:?}", metrics.lra),
    Err(e) => eprintln!("分析失败: {}", e),
}
```

## 质量评分模块

### QualityScorer

质量评分器的主要接口。

#### new()

创建使用默认阈值的评分器实例。

```rust
pub fn new() -> Self
```

#### with_thresholds()

使用自定义阈值创建评分器实例。

```rust
pub fn with_thresholds(thresholds: QualityThresholds) -> Self
```

#### analyze_file()

分析单个文件的质量。

```rust
pub fn analyze_file(&self, metrics: &FileMetrics) -> QualityAnalysis
```

**参数**：
- `metrics`: 文件的技术指标

**返回值**：
- `QualityAnalysis`: 完整的质量分析结果

#### analyze_files()

批量分析多个文件的质量。

```rust
pub fn analyze_files(&self, metrics_list: &[FileMetrics]) -> Vec<QualityAnalysis>
```

**参数**：
- `metrics_list`: 文件指标列表

**返回值**：
- `Vec<QualityAnalysis>`: 质量分析结果列表

**使用示例**：
```rust
use crate::analyzer::scoring::QualityScorer;

let scorer = QualityScorer::new();
let analysis = scorer.analyze_file(&metrics);

println!("质量分数: {}", analysis.quality_score);
println!("状态: {}", analysis.status);
```

## 报告生成模块

### ReportGenerator

报告生成器的主要接口。

#### new()

创建报告生成器实例。

```rust
pub fn new() -> Self
```

#### generate_csv_report()

生成CSV格式的质量报告。

```rust
pub fn generate_csv_report<P: AsRef<Path>>(
    &self,
    analyses: &[QualityAnalysis],
    output_path: P,
) -> Result<()>
```

**参数**：
- `analyses`: 质量分析结果列表
- `output_path`: 输出CSV文件路径

**返回值**：
- `Result<()>`: 成功时返回Ok，失败时返回错误

#### display_summary()

在控制台显示质量分析摘要。

```rust
pub fn display_summary(&self, analyses: &[QualityAnalysis])
```

**参数**：
- `analyses`: 质量分析结果列表

**使用示例**：
```rust
use crate::analyzer::report::ReportGenerator;

let generator = ReportGenerator::new();

// 生成CSV报告
generator.generate_csv_report(&analyses, "report.csv")?;

// 显示摘要
generator.display_summary(&analyses);
```

## 质量阈值配置

### QualityThresholds

质量评分的阈值配置结构。

```rust
pub struct QualityThresholds {
    // 频谱相关阈值
    pub spectrum_fake_threshold: f64,      // -85.0 dB
    pub spectrum_processed_threshold: f64, // -80.0 dB
    pub spectrum_good_threshold: f64,      // -70.0 dB
    
    // 动态范围阈值
    pub lra_poor_max: f64,        // 3.0 LU
    pub lra_low_max: f64,         // 6.0 LU
    pub lra_excellent_min: f64,   // 8.0 LU
    pub lra_excellent_max: f64,   // 12.0 LU
    pub lra_acceptable_max: f64,  // 15.0 LU
    pub lra_too_high: f64,        // 20.0 LU
    
    // 峰值相关阈值
    pub peak_clipping_db: f64,     // -0.1 dB
    pub peak_clipping_linear: f64, // 0.999
    pub peak_good_db: f64,         // -6.0 dB
    pub peak_medium_db: f64,       // -3.0 dB
}
```

**自定义阈值示例**：
```rust
let custom_thresholds = QualityThresholds {
    spectrum_fake_threshold: -90.0,  // 更严格的假无损检测
    lra_poor_max: 2.5,               // 更严格的压缩检测
    ..Default::default()
};

let scorer = QualityScorer::with_thresholds(custom_thresholds);
```

## 错误处理

所有可能失败的操作都返回 `Result<T, E>` 类型，其中错误类型通常是 `anyhow::Error`。

**常见错误类型**：
- FFmpeg执行失败
- 文件访问权限问题
- 音频格式不支持
- 输出解析失败
- 文件写入失败

**错误处理最佳实践**：
```rust
match ffmpeg::process_file(path, ffmpeg_path) {
    Ok(metrics) => {
        // 处理成功的情况
    },
    Err(e) => {
        eprintln!("处理文件 {} 时发生错误: {}", path.display(), e);
        // 记录错误但继续处理其他文件
    }
}
```

## 性能考虑

### 并行处理

- `process_file()` 内部使用并行处理多个FFmpeg任务
- `analyze_files()` 对大量文件使用并行评分计算
- 建议根据CPU核心数调整并行度

### 内存使用

- 使用 `&[FileMetrics]` 而非 `Vec<FileMetrics>` 避免不必要的克隆
- 及时释放大型数据结构
- 对于大量文件，考虑分批处理

### I/O优化

- CSV写入使用缓冲区提高性能
- 避免频繁的小文件写入操作
- 使用适当的文件缓冲区大小
