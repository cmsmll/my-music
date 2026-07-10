//! 本地音频解码功能域。
//!
//! 这里只保留扫描、格式识别、加密音频解码和写出逻辑；前端交互入口放在
//! `interaction` 模块中。

mod kgm;
mod ncm;
mod scanner;

pub(crate) use scanner::{ScanEvent, Scanner};
