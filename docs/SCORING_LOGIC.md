# 音频质量评分逻辑（v2）

实现文件：`src/analyzer/scoring.rs`

## 评分档案（Profile）

- `pop`（默认）：面向 A-pop / J-pop / K-pop 流媒体交付
- `broadcast`：面向广播响度目标
- `archive`：面向存档/审计场景（响度约束更宽）

`pop` 档案默认目标：

- Target Loudness: `-14 LUFS`
- True Peak warning: `-1.0 dBTP`
- True Peak critical: `-0.2 dBTP`

## 输入指标

- `integratedLoudnessLufs`（I）
- `truePeakDbtp`（TP）
- `lra`
- 高频段 `rmsDbAbove16k/18k/20k`
- ffprobe 元数据（采样率/码率/声道/codec/container）

## 状态判定顺序（短路）

1. `数据不完整`
2. `可疑 (伪造)`（lossless 且高频极低）
3. `疑似处理`
4. `已削波`（TP 超过 critical）
5. `真峰值风险`（TP 超过 warning）
6. `响度偏离目标`
7. `低码率`
8. `低采样率`
9. `单声道`
10. `严重压缩` / `低动态`
11. `质量良好`

## 分数构成（0-100）

- Compliance：35 分（LUFS + True Peak）
- Dynamics：20 分（LRA）
- Spectrum：25 分（16k/18k 高频能量）
- Authenticity：10 分（无损真实性/高频一致性）
- Integrity：10 分（完整性 + errorCodes）

并附加 profile 相关扣分（低码率/低采样率/单声道等），最后按状态施加上限：

- Suspicious 上限 25
- Incomplete 上限 45
- Clipped 上限 75
- TruePeakRisk 上限 85

## 置信度

输出 `confidence`，根据关键字段缺失与 `errorCodes` 下降，范围 `[0.1, 1.0]`。
