# FFmpeg/FFprobe 指标提取逻辑

实现文件：`src/analyzer/ffmpeg.rs`

## 1. 执行模型

- 每个文件内部并发执行 5 个 `ffmpeg` 任务：
  - `ebur128=peak=true` 提取 `LRA + Integrated LUFS + True Peak`
  - `astats` 提取 peak/rms
  - `highpass+astats` 提取 `>16k`, `>18k`, `>20k` RMS
- 额外执行 1 个 `ffprobe` 任务提取元数据
- 全部外部命令经过统一执行器：
  - 超时控制（`command_timeout`）
  - 全局并发控制（`ProcessLimiter`）
  - stdout/stderr 并发读取，避免管道阻塞

## 2. 安全与稳定性策略

- 外部命令超时后会主动 `kill`，并记录 `E_TIMEOUT`
- 命令失败返回 `E_EXEC_FAILED`
- 解析失败返回 `E_PARSE_*`
- 所有错误码最终写入 `FileMetrics.errorCodes`，便于审计

## 3. 解析规则

### 响度与真峰值（ebur128）

- 从 summary 解析：
  - `I: ... LUFS`（integrated loudness）
  - `LRA: ... LU`
  - `Peak: ... dBFS`（true peak）
- `LRA` 和 `true peak` 支持流式日志兜底解析

### Peak / RMS（astats）

- 使用 `Overall` 统计块解析
- 正则不再绑定 `Parsed_astats_0/1`，提升跨版本兼容性

### 高频 RMS（highpass + astats）

- 解析 `Overall` 中的 `RMS level dB`

### ffprobe 元数据

读取以下字段：

- `codec_name`
- `sample_rate`
- `channels`
- `bit_rate`（stream 优先，format 兜底）
- `format_name`
- `duration`

## 4. 输出字段

`process_file()` 最终输出：

- 音频技术指标（Integrated LUFS / True Peak / LRA / Peak / RMS / 高频 RMS）
- ffprobe 元数据（采样率/码率/声道/编码器/容器/时长）
- `processingTimeMs`
- `errorCodes`
