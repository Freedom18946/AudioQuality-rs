# AudioQuality-rs

Rust 音频质量分析器，基于 FFmpeg/FFprobe 执行指标提取、质量评分和报告输出。

## 功能

- 递归扫描常见音频格式（wav/mp3/m4a/flac/aac/ogg/opus/wma/aiff/alac）
- 并行提取指标：LRA、Peak、RMS、16k/18k/20k 高频能量
- `ffprobe` 元数据：采样率、码率、声道、编码器、容器、时长
- 质量状态分类：`质量良好`、`数据不完整`、`可疑(伪造)`、`疑似处理`、`已削波`、`真峰值风险`、`响度偏离目标`、`严重压缩`、`低动态`、`低码率`、`低采样率`、`单声道`
- 安全模式（默认开启）：
  - 原子写入输出文件
  - 拒绝写入到符号链接路径（防止链接覆盖）
  - 外部命令超时保护
  - 外部命令并发限流
- 增量缓存（默认开启）：基于 `mtime + size + SHA-256` 跳过未变化文件
- 输出格式：CSV、JSON（默认），可选 JSONL、SARIF

## 快速开始

```bash
cargo build --release
cargo run --release -- /path/to/music
```

交互模式：

```bash
cargo run --release
```

## CLI 参数

```bash
AudioQuality-rs [PATH] [OPTIONS]
```

常用选项：

- `--ffmpeg-timeout-seconds <N>` 每个外部命令超时秒数（默认 `90`）
- `--max-ffmpeg-processes <N>` 最大并发外部进程数（默认 CPU 核心数）
- `--unsafe-mode` 关闭安全模式（不推荐）
- `--no-cache` 关闭增量缓存
- `--jsonl` 额外生成 `audio_quality_report.jsonl`
- `--sarif` 额外生成 `audio_quality_report.sarif.json`
- `--profile <pop|broadcast|archive>` 评分档案（默认 `pop`，面向 A-pop/J-pop/K-pop）
  - `pop` 默认是宽松流行乐档案：约 `-9 LUFS` 目标、`+0.1 / +1.0 dBTP` 风险阈值

## 输出文件

默认输出（写入目标目录）：

- `audio_quality_report.csv`
- `analysis_data.json`
- `.audio_quality_cache.json`（缓存开启时）

可选输出：

- `audio_quality_report.jsonl`（使用 `--jsonl`）
- `audio_quality_report.sarif.json`（使用 `--sarif`）

## 评分说明（实现版）

综合分数范围 `0-99`（硬上限，永不满分），由多维得分叠加并结合额外扣分：

- `Compliance`：基于 `Integrated LUFS` + `True Peak dBTP`
- `Dynamics`：基于 `LRA`
- `Spectrum/Authenticity`：基于高频段 RMS 与容器/编码推断
- `Integrity`：基于关键字段完整性与错误码
- 默认 `pop` 档案以流媒体音乐为目标（A-pop/J-pop/K-pop），可切换 `broadcast/archive`
- `90+` 仅授予通过 elite gate 的曲目（关键指标同时优秀）
- 未通过 elite gate 但原始总分大于 `90` 的曲目，会按 `elite_readiness` 连续压缩到 `85-89`，避免大量堆积在单一分数

## 开发与测试

```bash
cargo fmt
cargo clippy --all-targets --all-features
cargo test
```

当前测试：`24` 个测试，覆盖评分/报告/主配置逻辑。
