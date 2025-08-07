# 音频质量分析器 (AudioQuality-rs)

[![语言 (Language)](https://img.shields.io/badge/language-Rust-orange.svg)](https://www.rust-lang.org/)
[![许可证 (License)](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![版本 (Version)](https://img.shields.io/badge/version-4.0.0-blue.svg)]()

**中文 (Chinese) | [English](#english-version)**

这是一个使用纯 Rust 编写的高性能音频质量分析工具。它通过并行调用 FFmpeg 提取音频技术指标，使用先进的三维评分算法进行质量评估，并生成详细的分析报告。该工具能够准确识别假无损音频、削波问题、动态范围压缩等常见音频质量问题。

该项目专注于性能、准确性和易用性，通过 Rust 的并行处理能力 (Rayon) 和健壮的错误处理机制，为专业音频质量分析提供了完整的解决方案。

---

## ✨ 主要特性 (Features)

### 🎯 核心功能
- **智能质量评分**: 使用三维评分体系（完整性40分 + 动态范围30分 + 频谱30分）进行综合质量评估
- **假无损检测**: 通过高频段频谱分析准确识别转码音频（假无损）
- **削波检测**: 基于峰值电平分析检测数字削波问题
- **动态范围评估**: 使用 EBU R128 LRA 标准评估音频压缩程度
- **质量状态分类**: 自动分类音频质量状态（优秀/良好/削波/压缩/假无损等）

### ⚡ 性能优势
- **高性能并行处理**: 利用 [Rayon](https://github.com/rayon-rs/rayon) 库并发执行多个 FFmpeg 进程
- **多核CPU优化**: 充分利用现代多核处理器，显著缩短大型音乐库分析时间
- **内存高效**: 流式处理避免内存占用过高，支持处理大量文件

### 📊 全面的技术指标
- **响度范围 (LRA)**: 基于 EBU R128 标准的动态范围测量
- **峰值电平**: 数字削波风险评估
- **高频段能量**: 16kHz/18kHz/20kHz 以上频段 RMS 电平分析
- **整体 RMS**: 平均功率电平测量
- **处理时间**: 性能监控和优化参考

### 📈 丰富的输出格式
- **CSV 质量报告**: 按分数排序的详细分析报告，包含所有指标和评估结果
- **控制台摘要**: 实时显示前10名排名、状态分布和统计信息
- **JSON 原始数据**: 完整的技术指标数据，便于二次开发和集成
- **中文本地化**: 完整的中文界面和报告，符合国内用户习惯

---

## 🚀 快速开始 (Quick Start)

### 系统要求
- **操作系统**: macOS, Linux, Windows
- **Rust版本**: 1.70.0 或更高版本
- **FFmpeg**: 系统中需要安装FFmpeg或在项目resources目录中提供

### 安装与使用

1. **克隆项目**：
```bash
git clone <repository-url>
cd AudioQuality-rs
```

2. **编译项目**：
```bash
# 发布版本（推荐，性能更好）
cargo build --release
```

3. **运行分析**：
```bash
# 直接分析指定文件夹
cargo run --release -- /path/to/music/folder

# 交互模式
cargo run --release
```

### 输出文件

分析完成后，将在目标文件夹中生成：

- **`audio_quality_report.csv`**: 详细的质量分析报告（按分数排序）
- **`analysis_data.json`**: 原始技术指标数据
- **控制台输出**: 实时显示分析摘要和前10名排名

---

## 📊 评分体系说明

### 质量分数 (0-100分)

- **90-100分**: 优秀质量，无明显问题
- **80-89分**: 良好质量，可能有轻微问题  
- **70-79分**: 中等质量，存在一些问题
- **60-69分**: 较差质量，有明显问题
- **< 60分**: 质量很差，建议重新获取

### 质量状态分类

- **质量良好**: 无明显技术问题
- **已削波**: 存在数字削波风险
- **低动态**: 动态范围过低，可能过度压缩
- **严重压缩**: 动态范围极低，严重过度压缩
- **可疑(伪造)**: 疑似假无损音频
- **疑似处理**: 可能经过处理的音频
- **数据不完整**: 关键指标缺失

---

## 🛠️ 高级功能

### 支持的音频格式

- **无损格式**: FLAC, ALAC, AIFF, WAV
- **有损格式**: MP3, AAC, OGG, Opus, WMA, M4A

### 性能优化

```bash
# 设置并行线程数（可选）
export RAYON_NUM_THREADS=8
cargo run --release -- /path/to/music

# 大量文件处理建议
# - 使用发布版本 (--release)
# - 确保有足够的系统内存
# - 考虑分批处理超大音乐库
```

### 集成使用

作为库使用的示例：

```rust
use AudioQuality_rs::analyzer::{ffmpeg, scoring::QualityScorer, report::ReportGenerator};

// 提取技术指标
let metrics = ffmpeg::process_file(&audio_path, &ffmpeg_path)?;

// 质量评分
let scorer = QualityScorer::new();
let analysis = scorer.analyze_file(&metrics);

// 生成报告
let generator = ReportGenerator::new();
generator.generate_csv_report(&[analysis], "report.csv")?;
```

---

## 📚 文档

详细文档请参考 `docs/` 目录：

- **[使用指南](docs/使用指南.md)**: 详细的使用说明和最佳实践
- **[架构设计](docs/架构设计.md)**: 项目架构和设计理念
- **[API文档](docs/API文档.md)**: 完整的API参考文档
- **[评分算法](docs/SCORING_LOGIC.md)**: 质量评分算法详解
- **[FFmpeg逻辑](docs/FFMPEG_EXTRACTION_LOGIC.md)**: 音频指标提取逻辑

---

## 🧪 测试

```bash
# 运行所有测试
cargo test

# 运行性能测试
cargo test --release

# 查看测试覆盖率
cargo test -- --nocapture
```

---

## 📈 性能表现

在现代多核处理器上的典型性能：

- **15个FLAC文件**: ~1分35秒
- **并行处理**: 充分利用多核CPU
- **内存使用**: 高效的流式处理
- **准确性**: 与Python参考实现结果一致

---

## 🤝 贡献

欢迎提交 Issue 和 Pull Request！

1. Fork 项目
2. 创建功能分支 (`git checkout -b feature/AmazingFeature`)
3. 提交更改 (`git commit -m 'Add some AmazingFeature'`)
4. 推送到分支 (`git push origin feature/AmazingFeature`)
5. 打开 Pull Request

---

## 📄 许可证

本项目采用 MIT 许可证 - 详见 [LICENSE](LICENSE) 文件。

---

## English Version

AudioQuality-rs is a high-performance audio quality analysis tool written in pure Rust. It extracts audio technical metrics through parallel FFmpeg calls, uses advanced three-dimensional scoring algorithms for quality assessment, and generates detailed analysis reports.

### Key Features

- **Intelligent Quality Scoring**: Three-dimensional scoring system (Integrity 40pts + Dynamics 30pts + Spectrum 30pts)
- **Fake Lossless Detection**: Accurate identification of transcoded audio through high-frequency spectrum analysis
- **Clipping Detection**: Digital clipping detection based on peak level analysis
- **Dynamic Range Assessment**: Audio compression evaluation using EBU R128 LRA standard
- **Quality Status Classification**: Automatic classification of audio quality status

### Quick Start

```bash
git clone <repository-url>
cd AudioQuality-rs
cargo build --release
cargo run --release -- /path/to/music/folder
```

### Output Files

- **`audio_quality_report.csv`**: Detailed quality analysis report (sorted by score)
- **`analysis_data.json`**: Raw technical metrics data
- **Console output**: Real-time analysis summary and top 10 rankings

For detailed documentation, please refer to the `docs/` directory.
