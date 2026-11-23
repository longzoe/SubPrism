<div align="center">

# SubPrism

**Refracting live video audio into crystal-clear local AI subtitles — wrapped in Liquid Glass.**  
**棱镜字幕 · 基于 Rust 与本地 AI 的 Windows / macOS 液态玻璃实时视频字幕翻译软件**

[![License](https://img.shields.io/badge/License-Apache--2.0-38bdf8.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-Tauri%20v2-f97316.svg)](https://v2.tauri.app/)
[![Platform](https://img.shields.io/badge/Platform-Windows%20%7C%20macOS-818cf8.svg)](#)
[![Local AI](https://img.shields.io/badge/Local%20AI-Spark--X2.5--4B%20%2B%20llama.cpp-10b981.svg)](#)

</div>

---

## 为什么选择 SubPrism？

观看外语技术演讲、YouTube、B站、纪录片、直播或本地无字幕影片时，SubPrism 直接监听电脑系统内播声音，全程 **100% 本地离线推理**——无需付费 API、无需安装虚拟声卡驱动，以极简美观的 **液态玻璃（Liquid Glass）长条字幕** 实时悬浮呈现在视频上方。

- **零虚拟声卡双端内录**
  - **Windows 10/11**：原生 **WASAPI Loopback**，支持全局回环或指定输出设备。
  - **macOS 13+**：原生 **ScreenCaptureKit**，支持全局或指定应用进程内录，并启用 `FullScreenAuxiliary` 在全屏视频上层稳定悬浮。
- **超低延迟全本地 AI 流水线**
  - **本地语音识别（CPU ONNX，不占 GPU 显存）**：`Silero VAD v5` 精准断点防幻觉 + `SenseVoice-Small (INT8)` 自动识别中/英/日/韩/粤语音（实测单句 **25–133 ms**）。
  - **本地大模型同传（GPU CUDA / Metal）**：自动拉起并托管 **`llama.cpp` (`llama-server`)** 运行 **`Spark-X2.5-4B`**，强制开启 **Non-Thinking 极速模式**（`--reasoning off --reasoning-budget 0` + `enable_thinking: false`）与 **KV Cache 前缀复用**（`--cache-reuse 256`），在 RTX 5080 上实测 **TTFT 54–67 ms · 150–174 tok/s**，逐字流式上屏。
  - **防僵尸进程保护**：Windows 通过 Win32 Job Object (`KILL_ON_JOB_CLOSE`) 绑定子进程，软件关闭或异常退出时即刻释放全部显存。
- **Liquid Glass 液态玻璃长条悬浮 UI**
  - Windows 11 原生 DWM GPU Acrylic 圆角动态模糊；macOS 原生毛玻璃材质。
  - 镜面高光顶边、光标流光折射、动态声纹呼吸条、鼠标离开自动隐身工具栏。
  - 三档主题一键切换：**影院深玻璃** / **冰透浅玻璃** / **极简浮字**。
  - 支持 **鼠标穿透锁**（`Alt+Shift+L`），看视频拖拽进度条零遮挡；支持场景风格预设（通用 / 科技 / 影视 / 动漫）、自定义术语表与 `.srt` 双语字幕导出。

---

## 系统流水线

```text
系统内播音频 (Win: WASAPI / Mac: ScreenCaptureKit)
  └─▶ 纯 Rust 抗混叠重采样 (16 kHz Mono)
        └─▶ Silero VAD v5 (语音断点检测，过滤 BGM 与静音)
              └─▶ SenseVoice-Small INT8 (CPU ONNX，实时草稿 + 定稿识别)
                    └─▶ Spark-X2.5-4B via 自动托管 llama-server (GPU Non-Thinking + KV Cache + SSE 流式)
                          └─▶ SubPrism 液态玻璃长条字幕悬浮窗 (双语渐显 + 鼠标穿透)
```

---

## 模型与运行库准备（自带模型，轻量分发）

为保持安装包小巧与合规，SubPrism 不内置大模型权重与推理二进制。首次使用时，在右上角 **⚙ 设置 → 本地模型与引擎** 中选择以下 5 项本地路径即可：

| 组件 | 推荐版本 / 文件 | 获取地址 |
| :--- | :--- | :--- |
| **1. `llama-server`** | **`llama.cpp >= b10833`**（原生支持 `spark2_5` 架构）<br>Win CUDA 用户需将 `llama-*-cuda-*.zip` 与 `cudart-*.zip` 解压到同目录；Mac 下载 `macos-arm64` | [ggml-org/llama.cpp Releases](https://github.com/ggml-org/llama.cpp/releases) |
| **2. `Spark-X2.5-4B` GGUF** | `Spark-X2.5-4B-Q4_K_M.gguf`（~2.42 GB，速度与质量兼顾）或 `Q8_0` | [ModelScope (XHToken)](https://modelscope.cn/models/XHToken/Spark-X2.5-4B-GGUF) / [HuggingFace](https://huggingface.co/sizzlebop/Spark-X2.5-4B-GGUF) |
| **3. `sherpa-onnx` 共享库** | `v2.0.3` shared-no-tts-lib 中的 `sherpa-onnx-c-api.dll`（Win）或 `libsherpa-onnx-c-api.dylib`（Mac），保持同目录 `onnxruntime` 完整 | [k2-fsa/sherpa-onnx Releases](https://github.com/k2-fsa/sherpa-onnx/releases) |
| **4. SenseVoice-Small 模型目录** | 官方多语种版 `sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17`（含 `model.int8.onnx` 和 `tokens.txt`） | [sherpa-onnx ASR Models](https://github.com/k2-fsa/sherpa-onnx/releases/tag/asr-models) / [ModelScope 镜像](https://modelscope.cn/models/fengge2024/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17) |
| **5. Silero VAD v5** | `silero_vad_v5.onnx` | [sherpa-onnx ASR Models](https://github.com/k2-fsa/sherpa-onnx/releases/tag/asr-models) |

---

## 全局快捷键

| 快捷键 | 功能 |
| :--- | :--- |
| `Alt + Shift + S` | 开始 / 暂停实时音频监听 |
| `Alt + Shift + L` | 切换鼠标穿透锁（解锁/锁定字幕条点击） |
| `Alt + Shift + H` | 隐藏 / 显示液态玻璃字幕条 |

---

## 工程结构

```text
SubPrism/
├── crates/
│   ├── subprism-core/   # 配置持久化、硬件探测、Spark-X2.5-4B 提示词构造器
│   ├── subprism-audio/  # Win WASAPI Loopback / Mac ScreenCaptureKit + 16kHz 重采样
│   ├── subprism-asr/    # 动态 FFI 加载 sherpa-onnx v2.0.3 (Silero VAD v5 + SenseVoice)
│   └── subprism-llm/    # llama-server 自动生命周期托管 + Spark-X2.5-4B 流式翻译器
├── src-tauri/           # Tauri v2 主程序（液态玻璃原生窗口、托盘、全局快捷键、流水线编排）
├── ui/                  # React 19 + TypeScript + Tailwind CSS v4 液态玻璃双窗口前端
└── scripts/             # Windows / macOS 一键构建启动脚本
```

---

## 从源码构建与开发

### 前置依赖
- Rust (stable) + Cargo
- Node.js 20+ 或 Bun
- Windows：MSVC Build Tools + WebView2 Runtime（Win11 自带）
- macOS：Xcode Command Line Tools（macOS 13+，首次监听需在“隐私与安全性 → 录屏与系统录音”中授权）

### 构建并运行

```bash
# 1. 构建前端资源
cd ui && npm install && npm run build && cd ..

# 2. 运行单元测试
cargo test --workspace

# 3. 编译并启动独立 Release 桌面端（默认启用 custom-protocol 内嵌 ui/dist）
cargo run --release -p subprism
```

---

## 开源协议

本项目基于 [Apache-2.0](LICENSE) 协议开源。欢迎提 Issue 与 PR！
