# 音频质量评分算法详解

**中文 (Chinese) | [English](#english-version)**

音频质量的最终得分是一个满分为 100 分的综合评估值。分数是根据一系列技术指标，通过扣分制计算得出的。每项检查都针对一种常见的音频质量问题，如“假无损”、码率过低或动态范围压缩过度。

---

## 评分流程 (Scoring Process)

对于每个音频文件，程序首先初始化一个 **满分 (100分)**，然后根据以下规则逐项进行检查。如果文件触发了某个问题的条件，就会从总分中扣除相应的分数，并记录下具体问题。

### 规则 1: “假无损”检查 (Transcode Check)

这是最严重的一类问题，因此扣分最多。如果一个文件声称是 **无损格式 (Lossless)**（如 FLAC, ALAC, WAV），但其频谱在 **21kHz 以下** 就出现了明显的截止 (cutoff)，那么它极有可能是从一个有损源（如 MP3）转码而来的。

- **触发条件**: `文件为无损` 并且 `频谱截止频率 < 21000 Hz`
- **扣分**: **-60分**
- **记录问题**: "疑似假无损: 频谱在 [截止频率] Hz 处有明显截止，这通常是源文件为有损格式的迹象。"

### 规则 2: 有损压缩质量评估 (Lossy Compression Quality)

对于 **有损格式 (Lossy)** 的文件，我们主要关注其码率和频谱表现是否匹配。

- **2a. 码率过低 (Low Bitrate)**:
    - **触发条件**: `码率 < 192 kbps`
    - **扣分**: **-30分**
    - **记录问题**: "码率较低 ([码率] kbps)，可能导致音质细节损失。"

- **2b. 高码率但频谱不佳 (High Bitrate, Poor Spectrum)**:
    - **触发条件**: `码率 > 256 kbps` 并且 `频谱截止频率 < 19500 Hz`
    - **扣分**: **-25分**
    - **记录问题**: "高码率但频谱截止过早 ([截止频率] Hz)，可能编码效率不高或源文件有问题。"

### 规则 3: 采样率检查 (Sample Rate Check)

行业标准的采样率是 44.1kHz 或 48kHz。过低的采样率会直接限制音频能够记录的最高频率。

- **触发条件**: `采样率 < 44100 Hz`
- **扣分**: **-20分**
- **记录问题**: "采样率较低 ([采样率] Hz)，限制了音频的最高频率。"

### 规则 4: 响度与峰值检查 (Loudness and Peak Check)

此项检查基于 EBU R128 响度标准，用于评估音频的动态是否被过度压缩，以及是否存在数字削波的风险。

- **4a. 响度过高 (High Loudness)**:
    - **触发条件**: `综合响度 (Integrated LUFS) > -9.0 LUFS`
    - **扣分**: **-10分**
    - **记录问题**: "响度较高 ([响度值] LUFS)，可能在某些设备上听起来过载。"

- **4b. 峰值过高 (High True Peak)**:
    - **触发条件**: `真实峰值 (True Peak) > -1.0 dBTP`
    - **扣分**: **-15分**
    - **记录问题**: "真实峰值过高 ([峰值] dBTP)，在转换为有损格式时有削波风险。"

### 规则 5: 通道数检查 (Channel Check)

大多数商业音乐都应该是立体声 (Stereo) 的。

- **触发条件**: `通道数 < 2` (即单声道 Mono)
- **扣分**: **-5分**
- **记录问题**: "文件为单声道。"

---

## 最终总结 (Final Summary)

所有检查完成后，程序会根据最终分数生成一个简短的 **质量摘要 (Summary)**：

- **95-100**: 优秀 (Excellent)
- **80-94**:  良好 (Good)
- **60-79**:  中等 (Fair)
- **40-59**:  较差 (Poor)
- **0-39**:   严重问题 (Critical)

如果文件被判定为“疑似假无损”，摘要中会额外注明。所有记录的问题点会合并成一条 **具体建议 (Suggestion)**，最终和分数、摘要一起写入报告。

---

## English Version

The final audio quality score is a comprehensive assessment out of a maximum of 100 points. The score is calculated using a penalty system based on a series of technical checks. Each check targets a common audio quality issue, such as "transcoding," low bitrate, or excessive dynamic range compression.

### Scoring Rules

1.  **Transcode Check**: If a file is lossless but its spectral cutoff is below 21kHz, it's likely a transcode from a lossy source. (**-60 points**)
2.  **Lossy Compression Quality**:
    -   **Low Bitrate**: If bitrate is below 192 kbps. (**-30 points**)
    -   **High Bitrate, Poor Spectrum**: If bitrate is > 256 kbps but spectral cutoff is below 19.5kHz. (**-25 points**)
3.  **Sample Rate Check**: If the sample rate is below 44.1kHz. (**-20 points**)
4.  **Loudness and Peak Check (EBU R128)**:
    -   **High Loudness**: If Integrated LUFS is above -9.0. (**-10 points**)
    -   **High True Peak**: If True Peak is above -1.0 dBTP. (**-15 points**)
5.  **Channel Check**: If the file is mono. (**-5 points**)

The final score determines a summary (e.g., "Excellent", "Good", "Critical"), and all detected issues are compiled into a suggestion string in the final report.
