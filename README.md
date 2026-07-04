# My Music

My Music 是一个基于 Tauri 2、Vue 3 和 Rust 实现的本地音乐播放器。项目目标是提供一个轻量、响应快、数据可控的桌面音乐管理工具：前端负责现代化播放器界面和交互状态，Rust 负责曲库扫描、元数据解析、音频播放、缓存生成、解码处理和日志记录。

## 技术栈

- 桌面框架：Tauri 2
- 前端框架：Vue 3 + TypeScript + Vite
- 状态管理：Pinia
- 后端核心：Rust
- 音频播放：rodio 0.22.2
- 音频元数据：lofty
- 本地配置：TOML
- 本地缓存：JSON + 独立封面/歌词缓存文件

## 主要功能

- 本地曲库：支持配置多个音乐目录，手动重载曲库。
- 歌曲列表：展示歌曲名、歌手、专辑、时长、封面等信息。
- 元数据解析：优先读取音频内嵌标签，缺失时回退到 `歌手-歌名` 文件名规则。
- 封面与歌词缓存：封面和歌词会解析为独立文件保存，不使用 base64 存储。
- 播放控制：播放、暂停、上一首、下一首、音量、进度拖拽和进度跳转。
- 播放模式：随机播放、循环播放、单曲循环。
- 播放队列：侧边栏展示当前播放列表，可跳转当前来源页面。
- 曲库视图：全部、歌手、专辑、统计、最近播放、用户歌单。
- 歌手/专辑页：按歌手或专辑聚合歌曲，展示封面、数量和总时长。
- 歌单管理：新建、重命名、删除、拖拽排序，歌曲右键添加或移除。
- 最近播放：自动记录播放过的歌曲。
- 播放统计：累计播放、聆听时长、最爱歌手、最爱专辑、最常播放歌曲。
- 设置页面：管理音乐库、解码器、缓存、样式和关于信息。
- 窗口状态：保存窗口宽高、音量、侧边栏宽度等运行状态。
- 系统热键：支持播放/暂停、上一首、下一首等媒体快捷键。

## 解码器

解码器用于扫描并处理加密或普通音频文件。当前支持配置扫描目录、输出目录和处理格式，默认处理格式为：

```text
mp3,flac,kgm,kgma,ncm
```

当前实现包括：

- KGM/KGMA 解码
- NCM 解码
- MP3/FLAC 复制到输出目录
- 成功处理后将源文件重置为 0 字节
- 解码过程、跳过原因和失败信息写入日志

解码和曲库重载是两个独立操作：点击“解码”只处理解码目录，点击“重载”只扫描配置的音乐目录。

## 配置与缓存

程序启动时会在可执行文件所在目录读取或生成 `config.toml`。配置采用下划线命名风格，主要结构包括：

```toml
music_directory = []

[decoder]
output_dir = ""
process_formats = "mp3,flac,kgm,kgma,ncm"
scan_directory = []

[cache]
library_cache_dir = ""
cover_cache_dir = ""
lyrics_cache_dir = ""
my_playlist_cache_dir = ""
log_dir = ""
play_statistics_cache_path = ""

[style]
background_color = ""
background_image = ""

[state]
width = 1200
height = 600
volume = 1.0
sidebar_width = 250
```

曲库缓存会按音乐目录生成对应 JSON 文件；封面、歌词、歌单、播放统计和日志使用独立目录或文件保存。用户歌单缓存不受曲库重载影响，内部保存歌曲 ID 引用，失效歌曲会在界面中以浅红色背景显示。

## 项目结构

```text
src/
  components/            Vue 组件
  stores/                Pinia 状态
  types/                 前端类型定义
  utils/                 前端工具函数

src-tauri/src/
  audio.rs               播放引擎
  commands.rs            Tauri 命令入口
  config.rs              配置读写和默认值
  decoder.rs             解码器调度与日志
  kgm.rs                 KGM/KGMA 解码
  ncm.rs                 NCM 解码
  library.rs             曲库扫描和元数据解析
  playlist.rs            歌单缓存
  scanner.rs             解码扫描器
  statistics.rs          播放统计
  media_shortcuts.rs     系统媒体快捷键
```

## 开发

安装依赖：

```bash
pnpm install
```

启动前端开发服务：

```bash
pnpm dev
```

启动 Tauri 开发模式：

```bash
pnpm tauri dev
```

## 构建

前端生产构建：

```bash
pnpm build
```

打包桌面应用：

```bash
pnpm release
```

`pnpm release` 默认递增 patch 版本，并同步更新 `package.json`、`src-tauri/Cargo.toml` 和 `src-tauri/tauri.conf.json` 后执行 Tauri 打包。也可以指定版本类型或明确版本号：

```bash
pnpm release minor
pnpm release major
pnpm release 1.2.3
```

如果只想打包、不更新版本号：

```bash
pnpm tauri:build
```

Rust 检查：

```bash
cd src-tauri
cargo check
cargo clippy
```

Release 构建已开启 Rust 优化配置，包括 LTO、单 codegen unit、strip 和 `panic = "abort"`。

## 说明

本项目专注本地音乐管理和播放，不依赖在线音乐服务。音频文件、配置、缓存和日志都保存在本地，适合用于个人本地曲库整理、播放和后续扩展。
