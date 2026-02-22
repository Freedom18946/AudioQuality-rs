# AudioQuality-rs API 文档（当前实现）

## 核心数据结构

### FileMetrics

`src/analyzer/metrics.rs` 中的核心结果结构，包含：

- 基础字段：`filePath`、`fileSizeBytes`、`processingTimeMs`
- ffmpeg 指标：`lra`、`peakAmplitudeDb`、`overallRmsDb`、`rmsDbAbove16k/18k/20k`
- ffprobe 指标：`sampleRateHz`、`bitrateKbps`、`channels`、`codecName`、`containerFormat`、`durationSeconds`
- 缓存/审计字段：`cacheHit`、`contentSha256`、`errorCodes`

### QualityStatus

当前状态枚举：

- `Good`（质量良好）
- `Incomplete`（数据不完整）
- `Suspicious`（可疑/伪造）
- `Processed`（疑似处理）
- `Clipped`（已削波）
- `SeverelyCompressed`（严重压缩）
- `LowDynamic`（低动态）
- `LowBitrate`（低码率）
- `LowSampleRate`（低采样率）
- `Mono`（单声道）

### QualityAnalysis

评分结果结构：

- `filePath`
- `质量分`
- `状态`
- `备注`
- `FileMetrics` 展平字段

## FFmpeg/FFprobe 处理 API

文件：`src/analyzer/ffmpeg.rs`

### ProcessingConfig

```rust
pub struct ProcessingConfig {
    pub ffmpeg_path: PathBuf,
    pub ffprobe_path: Option<PathBuf>,
    pub command_timeout: Duration,
    pub process_limiter: ProcessLimiter,
}
```

### ProcessLimiter

全局并发控制，限制外部进程总数，避免并发放大导致资源耗尽。

### process_file

```rust
pub fn process_file(path: &Path, config: &ProcessingConfig) -> Result<FileMetrics>
```

行为：

- 并发执行多个 ffmpeg 分析任务
- 执行 ffprobe 元数据采集（若可用）
- 汇总并输出 `errorCodes`（如 `E_TIMEOUT`, `E_PARSE_LRA`）

## 缓存 API

文件：`src/analyzer/cache.rs`

### AnalysisCache::load / save

- 读取/写入 `.audio_quality_cache.json`
- 缓存版本不匹配时自动忽略旧缓存

### fingerprint_file

```rust
pub fn fingerprint_file(path: &Path) -> Result<FileFingerprint>
```

生成 `mtime + size + SHA-256` 指纹。

### AnalysisCache::lookup / upsert

- `lookup` 命中后返回 `cacheHit=true` 的 `FileMetrics`
- `upsert` 更新缓存内容

## 报告 API

文件：`src/analyzer/report.rs`

### ReportGenerator::new

```rust
pub fn new(safe_mode: bool) -> Self
```

### generate_csv_report

```rust
pub fn generate_csv_report<P: AsRef<Path>>(&self, analyses: &[QualityAnalysis], output_path: P) -> Result<()>
```

### generate_jsonl_report

```rust
pub fn generate_jsonl_report<P: AsRef<Path>>(&self, analyses: &[QualityAnalysis], output_path: P) -> Result<()>
```

### generate_sarif_report

```rust
pub fn generate_sarif_report<P: AsRef<Path>>(&self, analyses: &[QualityAnalysis], output_path: P) -> Result<()>
```

### display_summary

控制台打印状态分布、Top N、统计摘要（文件名经过终端控制字符清洗）。

## 安全写入 API

文件：`src/analyzer/safe_io.rs`

- `atomic_write_bytes(path, data, safe_mode)`
- `atomic_write_string(path, content, safe_mode)`

安全模式下会拒绝符号链接路径并使用原子替换写入，防止链接覆盖风险。
