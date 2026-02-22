# 音频质量评分逻辑（当前实现）

本文档描述 `src/analyzer/scoring.rs` 的实际行为。

## 输入指标

评分输入来自两部分：

- `ffmpeg`：`lra`、`peak`、`overall rms`、`rms >16k/18k/20k`
- `ffprobe`：`sampleRateHz`、`bitrateKbps`、`channels`、`codecName`、`containerFormat`、`durationSeconds`

## 状态判定顺序

状态是单值，按以下顺序短路判定：

1. `数据不完整`：关键字段缺失数量 `>=2`（18k、lra、peak）
2. `可疑 (伪造)`：判定为无损且 `rmsDbAbove18k < -85`
3. `疑似处理`：`rmsDbAbove18k < -80`
4. `低码率`：判定为有损且 `bitrateKbps < 192`
5. `低采样率`：`sampleRateHz < 44100`
6. `单声道`：`channels < 2`
7. `已削波`：`peakAmplitudeDb >= -0.1`
8. `严重压缩`：`lra < 3`
9. `低动态`：`3 <= lra < 6`
10. `质量良好`

## 分数计算

基础分由三部分组成：

- 完整性（40分）：基于 `rmsDbAbove18k` + `peak`
- 动态（30分）：基于 `lra`
- 频谱（30分）：基于 `rmsDbAbove16k`

然后叠加惩罚：

- 关键字段缺失：每项 `-10`
- 有损低码率（<192 kbps）：`-30`
- 有损高码率但高频异常（>256 kbps 且 18k 低）：`-25`
- 低采样率（<44100）：`-20`
- 单声道：`-5`

最后按状态施加上限：

- `可疑 (伪造)`：总分上限 `20`
- `数据不完整`：总分上限 `40`

并裁剪到 `0..=100`。

## lossless/lossy 判定

`is_lossless` 由扩展名/codec/container 综合判断：

- 扩展名：`flac/alac/wav/aiff/aif`
- codec：`pcm_*`, `flac`, `alac`, `wavpack`, `ape`
- container 包含：`flac/wav/aiff`

`is_lossy` 在非 lossless 条件下，基于：

- 扩展名：`mp3/aac/m4a/ogg/opus/wma`
- codec：`mp3/aac/vorbis/opus/wmav2/mp2/ac3`

## 备注生成

输出备注会针对状态给出文本说明（如低码率、低采样率、单声道、削波、压缩等），用于 CSV/JSON/SARIF 报告。
