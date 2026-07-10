# My Music

My Music 是一个基于 Tauri 2、Vue 3、Pinia 和 Rust 的本地音乐播放器。项目以本地曲库为核心：前端负责播放器界面、歌单交互、播放页、歌词展示、音频播放和配置表单，Rust 后端负责配置与缓存、曲库扫描、元数据提取、解码器、歌词搜索、日志记录和本地系统能力。

项目当前更偏向个人本地音乐管理工具：所有配置、曲库缓存、封面、歌词、播放统计和日志都保存在本机，不依赖在线音乐服务完成基础播放。

## 技术栈

- 桌面框架：Tauri 2
- 前端：Vue 3 + TypeScript + Vite
- 状态管理：Pinia
- 后端：Rust
- 音频播放：Web HTMLAudioElement + Tauri asset protocol
- 音频元数据：lofty
- 歌词搜索：Lyrix + moka 缓存
- 本地配置：TOML
- 本地缓存：JSON + 独立封面/歌词文件

## 最近更新

- 播放核心切换为前端 `AudioEngine.vue` 封装的 `<audio>` 标签，减少后端音频设备占用和输出设备切换问题。
- 播放进度缓存改为前端本地轻量存储，播放进度按音频事件更新，关闭应用前会刷新当前播放记录。
- 新增后端打开目录命令，歌曲详情中的文件路径、封面缓存和歌词缓存可打开所在文件夹；失败会写入日志并通过全局消息提示。
- 新增单例启动，重复打开应用时会唤起已有窗口，不再创建第二个实例。
- 语义化播放模式：`shuffle` 表示随机列表，`random` 表示随机播放，并同步播放队列展示逻辑。
- 歌曲详情弹窗补充文件路径、封面缓存、歌词缓存等信息，并提供打开文件夹入口。
- README、缓存配置和项目结构说明同步当前前后端实现，移除未使用的频谱缓存配置。

## 已实现功能

### 曲库

- 支持配置多个音乐目录。
- 启动时读取配置和已有缓存，不自动扫描目录。
- 手动点击“重载”后重新扫描配置中的音乐目录，并强制更新“全部”、歌手、专辑、统计等系统缓存。
- 支持歌曲名、歌手、专辑、时长、文件大小、码率、采样率、封面、歌词路径等信息。
- 元数据优先读取音频内嵌标签，缺失时按 `歌手-歌名` 文件名规则回退，解析不出来时显示未知歌手。
- 封面和歌词以独立文件缓存，不使用 base64 写入歌曲列表缓存。
- 歌曲详情弹窗可查看文件路径、大小、格式相关信息。

### 播放器

- 支持播放、暂停、上一首、下一首、停止、音量、进度拖拽、进度跳转。
- 播放模式支持随机列表、随机播放、列表循环、单曲循环。
- 支持系统媒体快捷键：播放/暂停、上一首、下一首。
- 支持单例启动，重复运行程序时会显示并聚焦已存在的主窗口。
- 播放队列以侧边栏显示当前队列，打开时定位到当前歌曲。
- 播放页从底部进入，保留公共播放控制条，展示唱片/唱臂动效、歌曲信息和歌词。
- 歌词按行解析和高亮，进度跳转时歌词会跟随定位。
- 音频播放由前端 `AudioEngine.vue` 统一封装，播放错误会记录到后端日志，便于排查本地资源加载和媒体解码问题。

### 播放模式

- 随机列表：内部模式名为 `shuffle`，切换到该模式时会打乱当前播放列表，播放队列侧栏也显示打乱后的 `shuffle_queue`，之后按这个随机列表顺序播放。列表走到末尾后会重新洗牌。
- 随机播放：内部模式名为 `random`，不会打乱播放列表，也不会改变播放队列显示；点击下一首或自动下一首时，从当前列表中临时随机挑选一首。
- 循环播放：内部模式名为 `repeat`，当前列表播放到末尾后回到第一首。
- 单曲循环：内部模式名为 `repeat_one`，当前歌曲播放结束后重复播放当前歌曲。

### 歌单

- 内置“最近播放”和“我的歌单”。
- 支持新建、重命名、删除用户歌单。
- 支持用户歌单拖拽排序，排序写入歌单自身 metadata 的 `index`。
- 歌曲右键可添加到歌单；已存在的歌曲按钮会置灰。
- 最近播放和用户歌单可通过右键菜单移除歌曲记录，不删除音频文件。
- 用户歌单缓存独立保存，不受曲库重载影响；失效 ID 会保留并在界面中以缺失歌曲显示。

### 歌手、专辑和统计

- 歌手页按歌手聚合歌曲，展示封面、歌曲数量和总时长。
- 专辑页按专辑聚合歌曲，展示封面、歌曲数量和总时长。
- 进入歌手/专辑详情后可播放对应列表。
- 统计页包含音乐统计、播放统计和最常播放列表。
- 音乐统计包含歌曲总数、歌手数量、专辑数量、总时长、总大小。
- 播放统计包含累计播放、聆听时长、最爱歌手、最爱专辑。

### 歌词

- 支持读取歌词缓存目录中的同名 `.lrc` 文件。
- 播放页可搜索歌词，搜索结果会显示来源、歌曲名、歌手、专辑和时长。
- 歌词搜索结果通过 moka 缓存，减少重复请求。
- 使用歌词时会写入歌词缓存文件，并同步歌曲缓存中的 `lyrics_cache_hash`。
- Auto 歌词开关支持持久化，开启后会自动尝试搜索并加载歌词。

### 解码器

解码器用于批量处理配置目录中的音频文件，当前支持：

- KGM/KGMA 解码
- NCM 解码
- MP3/FLAC 复制到输出目录
- 处理成功后将源文件重置为 0 字节
- 解码记录、跳过原因、失败原因写入日志

默认处理格式：

```text
mp3,flac,kgm,kgma,ncm
```

解码和曲库重载是两个独立操作：

- “解码”只扫描解码器配置中的目录，并输出到解码器输出目录。
- “重载”只扫描音乐库配置中的目录，不会自动执行解码。

### 设置和主题

- 设置页按“音乐库、解码器、缓存、样式、关于”分类。
- 音乐库可管理扫描目录。
- 解码器可管理输出目录、处理格式和扫描目录。
- 缓存页可管理曲库缓存、歌单缓存、歌词缓存、封面缓存和日志缓存。
- 样式支持背景颜色、背景图片、背景图片透明度、标题色、副标题色、高亮色、边框色和是否显示边框。
- 状态类配置不在设置页显示，包括窗口宽高、音量、侧边栏宽度和 Auto 歌词开关。
- 前端配置变更使用 1 秒防抖同步到后端 `config.toml`。

## 配置文件

程序启动时会在可执行文件所在目录读取或生成 `config.toml`。配置字段使用下划线命名。

当前主要结构：

```toml
music_directory = []

[decoder]
output_dir = ""
process_formats = "mp3,flac,kgm,kgma,ncm"
scan_directory = []

[cache]
library_cache_dir = "library-cache"
cover_cache_dir = "cover-cache"
lyrics_cache_dir = "lyrics-cache"
playlist_cache_dir = "playlist-cache"
log_cache_dir = "log-cache"

[style]
background_color = "#ffffff"
background_image = ""
background_image_opacity = 1.0
title_color = "#1e2026"
subtitle_color = "#8b919c"
highlight_color = "#3bce82"
border_color = "#e8e8e8"
show_border = true

[state]
width = 1280
height = 820
volume = 1.0
sidebar_width = 250
auto_lyrics_enabled = false
```

说明：

- `music_directory` 是音乐库扫描目录数组。
- `decoder.scan_directory` 是解码器扫描目录数组。
- `decoder.output_dir` 为空时不会执行解码。
- `cache` 下统一管理曲库、歌单、封面、歌词和日志缓存目录。
- `state` 是运行状态，不在设置页面直接展示。

## 缓存设计

曲库缓存以“全部”为核心数据源，其他列表通过歌曲 ID 引用它。

- `all_playlist.json`：全部曲库核心缓存，`tracks` 是以歌曲 ID 为 key 的对象。
- `artists_playlist.json`：歌手聚合缓存。
- `albums_playlist.json`：专辑聚合缓存。
- `playlist-cache/recent_playlist.json`：最近播放缓存。
- `playlist-cache/*.json`：用户歌单缓存。
- `cover-cache/*`：封面文件缓存。
- `lyrics-cache/*`：歌词文件缓存。
- `library-cache/play-statistics.json`：播放统计缓存。
- `audio.log`：音频播放和跳转错误日志。
- `app.log`：应用运行、打开目录和配置相关日志。

曲库重载会更新系统生成的数据：全部、歌手、专辑、统计等。用户歌单只保存歌曲 ID 引用，不会因为重载曲库被清空。

## 项目结构

```text
src/
  components/                 Vue 组件
    ContentArea.vue           主内容区：全部、歌手、专辑、统计、歌单
    LibrarySidebar.vue        左侧曲库和歌单导航
    PlayerBar.vue             公共底部播放器
    NowPlayingPage.vue        播放详情页和歌词搜索
    PlaybackQueuePanel.vue    播放队列侧栏
    SettingsPanel.vue         设置页
  stores/                     Pinia 状态
    app_config.ts             配置和防抖保存
    library.ts                曲库、歌单、统计
    playback.ts               当前歌曲、播放状态、进度
    player_queue.ts           当前播放队列和播放模式
    ui.ts                     弹窗和页面显示状态
  types/                      前端类型定义
  utils/                      前端工具函数

src-tauri/src/
  lib.rs                      Tauri 应用入口和插件注册
  logger.rs                   日志目录和日志写入
  utils.rs                    通用缓存、哈希和时间工具
  interaction/                前后端交互层
    commands.rs               Tauri 命令入口
    config.rs                 配置读写、默认值和兼容旧配置
    library.rs                曲库扫描和元数据解析
    lyrics.rs                 歌词缓存、搜索和使用
    playlist.rs               歌单缓存和最近播放
    statistics.rs             播放统计
    media_shortcuts.rs        系统媒体快捷键
    models.rs                 前后端数据模型
    decoder.rs                解码器命令调度
  decoder/                    本地音频解码实现
    kgm.rs                    KGM/KGMA 解码
    ncm.rs                    NCM 解码
    scanner.rs                解码扫描器
  lyrics_search/              本地集成的 Lyrix 歌词搜索实现
```

## 启动流程

1. Rust 创建 `ConfigManager`，读取或生成 `config.toml`。
2. Tauri 注册歌词搜索服务、配置管理和系统媒体快捷键。
3. 单例插件会注册应用实例锁，重复启动时把已有主窗口显示、取消最小化并聚焦。
4. 启动后前端调用 `get_startup_state`，加载配置、已有曲库缓存、歌单缓存和播放统计。
5. 窗口根据配置中的宽高居中显示。
6. 文件选择和文件打开插件在 setup 中延迟初始化；音频播放由前端 `AudioEngine.vue` 在页面内管理。
7. 曲库不会自动重载，需要用户手动点击“重载”。

## 开发

安装依赖：

```bash
pnpm install
```

启动 Tauri 开发模式：

```bash
pnpm tauri dev
```

只启动前端开发服务：

```bash
pnpm dev
```

前端类型检查和生产构建：

```bash
pnpm build
```

Rust 检查：

```bash
cd src-tauri
cargo check
cargo clippy
```

## 构建和发布

只更新版本号：

```bash
pnpm release
```

可指定版本更新类型或明确版本号：

```bash
pnpm release patch
pnpm release minor
pnpm release major
pnpm release 1.2.3
```

更新版本号并打包：

```bash
pnpm release:build
```

不更新版本号，直接打包：

```bash
pnpm tauri:build
```

发布当前版本到 GitHub Releases：

```bash
pnpm release:github
```

发布脚本会读取 `package.json`、`src-tauri/Cargo.toml` 和 `src-tauri/tauri.conf.json` 中的当前版本号，并要求三者一致。脚本只上传当前版本的 MSI 和 NSIS 构建产物：

```text
src-tauri/target/release/bundle/msi/*当前版本*.msi
src-tauri/target/release/bundle/nsis/*当前版本*.exe
```

如果当前版本的 MSI 或 NSIS 任意一种不存在，发布会立即停止。发布前需要安装并登录 GitHub CLI：

```bash
gh auth login
```

Release 构建已开启 Rust 优化配置，包括 LTO、单 codegen unit、strip、`panic = "abort"` 和关闭 debug/incremental。

## 说明

这个项目目前主要面向 Windows 桌面环境开发和测试。Tauri 配置已关闭系统标题栏，窗口最小尺寸在 `tauri.conf.json` 中额外补偿了无标题栏窗口的系统占用尺寸。基础播放、曲库缓存、歌单、歌词和解码器都以本地文件为中心，适合继续扩展更多本地音乐管理能力。
