use std::fmt;
use std::io::{self, Read, Seek, SeekFrom, Write};

use aes::cipher::{Block, BlockCipherDecrypt, KeyInit};
use aes::Aes128;
use base64::engine::general_purpose;
use base64::Engine;
use serde_json::Value;

const MAGIC_HEADER: &[u8; 8] = b"CTENFDAM";
const METADATA_PREFIX: &[u8] = b"163 key(Don't modify):";

/// NCM 文件头部用于解密音频 key 的固定 AES-128 key。
const CORE_KEY: [u8; 16] = [
    0x68, 0x7A, 0x48, 0x52, 0x41, 0x6D, 0x73, 0x6F, 0x35, 0x6B, 0x49, 0x6E, 0x62, 0x61, 0x78, 0x57,
];

/// NCM metadata JSON 的固定 AES-128 key。
const META_KEY: [u8; 16] = [
    0x23, 0x31, 0x34, 0x6C, 0x6A, 0x6B, 0x5F, 0x21, 0x5C, 0x5D, 0x26, 0x30, 0x55, 0x3C, 0x27, 0x28,
];

#[derive(Debug)]
pub enum NcmError {
    Io(io::Error),
    InvalidHeader,
    InvalidKey,
    InvalidMetadata,
    InvalidPadding,
}

impl fmt::Display for NcmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NcmError::Io(err) => write!(f, "读取 NCM 文件失败: {err}"),
            NcmError::InvalidHeader => write!(f, "文件头不是已知的 NCM 格式"),
            NcmError::InvalidKey => write!(f, "NCM 音频 key 无效"),
            NcmError::InvalidMetadata => write!(f, "NCM metadata 无效"),
            NcmError::InvalidPadding => write!(f, "NCM AES 填充数据无效"),
        }
    }
}

impl std::error::Error for NcmError {}

impl From<io::Error> for NcmError {
    fn from(value: io::Error) -> Self {
        NcmError::Io(value)
    }
}

/// 从 NCM metadata JSON 中抽取出的常用音频标签。
#[derive(Debug, Default, Clone)]
pub struct NcmMetadata {
    pub title: Option<String>,
    pub album: Option<String>,
    pub artists: Vec<String>,
    pub format: Option<String>,
}

/// NCM 音频流解码器。
///
/// 构造时会消费 NCM 容器头部，同时保留 metadata 与封面；之后每次 `read`
/// 都会对读出的音频密文字节做 key box 异或还原。
pub struct NcmDecoder<R> {
    inner: R,
    key_box: [u8; 256],
    metadata: NcmMetadata,
    cover: Vec<u8>,
    pos: u64,
}

impl<R: Read + Seek> NcmDecoder<R> {
    pub fn new(mut inner: R) -> Result<Self, NcmError> {
        let mut header = [0_u8; 10];
        inner.read_exact(&mut header)?;
        if !header.starts_with(MAGIC_HEADER) {
            return Err(NcmError::InvalidHeader);
        }

        let key_len = read_u32_le(&mut inner)? as usize;
        if key_len == 0 {
            return Err(NcmError::InvalidKey);
        }
        let mut key_data = vec![0_u8; key_len];
        inner.read_exact(&mut key_data)?;
        for byte in &mut key_data {
            *byte ^= 0x64;
        }

        let key_plain = aes_128_ecb_decrypt(&key_data, &CORE_KEY)?;
        if key_plain.len() <= 17 {
            return Err(NcmError::InvalidKey);
        }

        let key_box = build_key_box(&key_plain[17..]);

        let metadata_len = read_u32_le(&mut inner)? as usize;
        let mut metadata_data = vec![0_u8; metadata_len];
        inner.read_exact(&mut metadata_data)?;
        let metadata = decode_metadata(&metadata_data)?;
        let media_info_pos = inner.stream_position()?;

        let cover = seek_to_audio_start(&mut inner, media_info_pos, &key_box)?;

        Ok(Self {
            inner,
            key_box,
            metadata,
            cover,
            pos: 0,
        })
    }

    pub fn metadata(&self) -> &NcmMetadata {
        &self.metadata
    }

    pub fn cover(&self) -> &[u8] {
        &self.cover
    }
}

impl<R: Read + Seek> Read for NcmDecoder<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let len = self.inner.read(buf)?;
        let audio = &mut buf[..len];

        decrypt_audio_chunk(&self.key_box, self.pos, audio);
        self.pos += len as u64;

        Ok(len)
    }
}

pub fn write_tagged_audio(
    writer: &mut impl Write,
    audio: &[u8],
    ext: &str,
    metadata: &NcmMetadata,
    cover: &[u8],
) -> io::Result<()> {
    match ext {
        "mp3" => write_mp3_with_id3(writer, audio, metadata, cover),
        "flac" => write_flac_with_metadata(writer, audio, metadata, cover),
        _ => writer.write_all(audio),
    }
}

fn read_u32_le(reader: &mut impl Read) -> Result<u32, NcmError> {
    let mut bytes = [0_u8; 4];
    reader.read_exact(&mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}

fn seek_to_audio_start<R: Read + Seek>(
    inner: &mut R,
    media_info_pos: u64,
    key_box: &[u8; 256],
) -> Result<Vec<u8>, NcmError> {
    let file_len = inner.seek(SeekFrom::End(0))?;

    if let Ok(modern) = read_modern_cover_layout(inner, media_info_pos, file_len) {
        if looks_like_audio_at(inner, modern.audio_pos, key_box)? {
            inner.seek(SeekFrom::Start(modern.audio_pos))?;
            return Ok(modern.cover);
        }
    }

    let legacy = read_legacy_cover_layout(inner, media_info_pos, file_len)?;
    if looks_like_audio_at(inner, legacy.audio_pos, key_box)? {
        inner.seek(SeekFrom::Start(legacy.audio_pos))?;
        return Ok(legacy.cover);
    }

    let modern = read_modern_cover_layout(inner, media_info_pos, file_len)?;
    inner.seek(SeekFrom::Start(modern.audio_pos))?;
    Ok(modern.cover)
}

struct CoverLayout {
    cover: Vec<u8>,
    audio_pos: u64,
}

fn read_modern_cover_layout<R: Read + Seek>(
    inner: &mut R,
    media_info_pos: u64,
    file_len: u64,
) -> Result<CoverLayout, NcmError> {
    inner.seek(SeekFrom::Start(media_info_pos + 5))?;

    let cover_frame_len = read_u32_le(inner)? as u64;
    let image_len = read_u32_le(inner)? as u64;
    let audio_pos = media_info_pos + 5 + 8 + cover_frame_len;
    if image_len > cover_frame_len || audio_pos > file_len {
        return Err(NcmError::InvalidHeader);
    }

    let mut cover = vec![0_u8; image_len as usize];
    inner.read_exact(&mut cover)?;

    Ok(CoverLayout { cover, audio_pos })
}

fn read_legacy_cover_layout<R: Read + Seek>(
    inner: &mut R,
    media_info_pos: u64,
    file_len: u64,
) -> Result<CoverLayout, NcmError> {
    inner.seek(SeekFrom::Start(media_info_pos + 9))?;

    let image_len = read_u32_le(inner)? as u64;
    let audio_pos = media_info_pos + 9 + 4 + image_len;
    if audio_pos > file_len {
        return Err(NcmError::InvalidHeader);
    }

    let mut cover = vec![0_u8; image_len as usize];
    inner.read_exact(&mut cover)?;

    Ok(CoverLayout { cover, audio_pos })
}

fn looks_like_audio_at<R: Read + Seek>(
    inner: &mut R,
    pos: u64,
    key_box: &[u8; 256],
) -> Result<bool, NcmError> {
    inner.seek(SeekFrom::Start(pos))?;

    let mut probe = [0_u8; 16];
    let len = inner.read(&mut probe)?;
    let probe = &mut probe[..len];
    decrypt_audio_chunk(key_box, 0, probe);

    Ok(probe.starts_with(b"ID3")
        || probe.starts_with(b"fLaC")
        || probe.starts_with(&[0xff, 0xfb])
        || probe.starts_with(&[0xff, 0xf3])
        || probe.starts_with(&[0xff, 0xf2]))
}

fn decrypt_audio_chunk(key_box: &[u8; 256], pos: u64, audio: &mut [u8]) {
    for (offset, byte) in audio.iter_mut().enumerate() {
        let j = ((pos + offset as u64 + 1) & 0xff) as usize;
        let k = key_box[j].wrapping_add(j as u8) as usize;
        let key_index = key_box[k].wrapping_add(key_box[j]) as usize;
        *byte ^= key_box[key_index];
    }
}

fn aes_128_ecb_decrypt(data: &[u8], key: &[u8; 16]) -> Result<Vec<u8>, NcmError> {
    if data.is_empty() || data.len() % 16 != 0 {
        return Err(NcmError::InvalidKey);
    }

    let cipher = Aes128::new_from_slice(key).map_err(|_| NcmError::InvalidKey)?;
    let mut out = data.to_vec();

    for chunk in out.chunks_exact_mut(16) {
        let block = <&mut Block<Aes128>>::from(
            <&mut [u8; 16]>::try_from(chunk).map_err(|_| NcmError::InvalidKey)?,
        );
        cipher.decrypt_block(block);
    }

    let pad = *out.last().ok_or(NcmError::InvalidPadding)? as usize;
    if pad == 0 || pad > 16 || pad > out.len() {
        return Err(NcmError::InvalidPadding);
    }
    if !out[out.len() - pad..]
        .iter()
        .all(|byte| *byte as usize == pad)
    {
        return Err(NcmError::InvalidPadding);
    }

    out.truncate(out.len() - pad);
    Ok(out)
}

fn decode_metadata(data: &[u8]) -> Result<NcmMetadata, NcmError> {
    if data.is_empty() {
        return Ok(NcmMetadata::default());
    }

    let mut xored = data.to_vec();
    for byte in &mut xored {
        *byte ^= 0x63;
    }

    if !xored.starts_with(METADATA_PREFIX) {
        return Err(NcmError::InvalidMetadata);
    }

    let encrypted = general_purpose::STANDARD
        .decode(&xored[METADATA_PREFIX.len()..])
        .map_err(|_| NcmError::InvalidMetadata)?;
    let plain = aes_128_ecb_decrypt(&encrypted, &META_KEY)?;
    if !plain.starts_with(b"music:") {
        return Err(NcmError::InvalidMetadata);
    }

    let value: Value =
        serde_json::from_slice(&plain[6..]).map_err(|_| NcmError::InvalidMetadata)?;
    let title = value
        .get("musicName")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let album = value
        .get("album")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let format = value
        .get("format")
        .and_then(Value::as_str)
        .map(|value| value.to_ascii_lowercase());
    let artists = value
        .get("artist")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            item.as_array()
                .and_then(|values| values.first())
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .collect();

    Ok(NcmMetadata {
        title,
        album,
        artists,
        format,
    })
}

fn build_key_box(key: &[u8]) -> [u8; 256] {
    let mut key_box = [0_u8; 256];
    for (index, byte) in key_box.iter_mut().enumerate() {
        *byte = index as u8;
    }

    let mut last_byte = 0_u8;
    let mut key_offset = 0_usize;

    for i in 0..256 {
        let swap = key_box[i];
        let c = swap.wrapping_add(last_byte).wrapping_add(key[key_offset]) as usize;
        key_offset = (key_offset + 1) % key.len();

        key_box[i] = key_box[c];
        key_box[c] = swap;
        last_byte = c as u8;
    }

    key_box
}

fn write_mp3_with_id3(
    writer: &mut impl Write,
    audio: &[u8],
    metadata: &NcmMetadata,
    cover: &[u8],
) -> io::Result<()> {
    let mut frames = Vec::new();

    if let Some(title) = metadata.title.as_deref() {
        push_text_frame(&mut frames, b"TIT2", title);
    }
    if !metadata.artists.is_empty() {
        push_text_frame(&mut frames, b"TPE1", &metadata.artists.join("/"));
    }
    if let Some(album) = metadata.album.as_deref() {
        push_text_frame(&mut frames, b"TALB", album);
    }
    if !cover.is_empty() {
        push_apic_frame(&mut frames, cover);
    }

    if !frames.is_empty() {
        writer.write_all(b"ID3")?;
        writer.write_all(&[3, 0, 0])?;
        writer.write_all(&syncsafe_u32(frames.len() as u32))?;
        writer.write_all(&frames)?;
    }

    writer.write_all(audio)
}

fn push_text_frame(frames: &mut Vec<u8>, id: &[u8; 4], text: &str) {
    let mut payload = Vec::new();
    payload.push(1);
    payload.extend_from_slice(&[0xff, 0xfe]);
    for code in text.encode_utf16() {
        payload.extend_from_slice(&code.to_le_bytes());
    }

    push_id3_frame(frames, id, &payload);
}

fn push_apic_frame(frames: &mut Vec<u8>, cover: &[u8]) {
    let mut payload = Vec::new();
    payload.push(0);
    payload.extend_from_slice(cover_mime(cover).as_bytes());
    payload.push(0);
    payload.push(3);
    payload.push(0);
    payload.extend_from_slice(cover);

    push_id3_frame(frames, b"APIC", &payload);
}

fn push_id3_frame(frames: &mut Vec<u8>, id: &[u8; 4], payload: &[u8]) {
    frames.extend_from_slice(id);
    frames.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    frames.extend_from_slice(&[0, 0]);
    frames.extend_from_slice(payload);
}

fn syncsafe_u32(value: u32) -> [u8; 4] {
    [
        ((value >> 21) & 0x7f) as u8,
        ((value >> 14) & 0x7f) as u8,
        ((value >> 7) & 0x7f) as u8,
        (value & 0x7f) as u8,
    ]
}

fn write_flac_with_metadata(
    writer: &mut impl Write,
    audio: &[u8],
    metadata: &NcmMetadata,
    cover: &[u8],
) -> io::Result<()> {
    if !audio.starts_with(b"fLaC") {
        return writer.write_all(audio);
    }

    let vorbis = build_vorbis_comment(metadata);
    let picture = if cover.is_empty() {
        None
    } else {
        Some(build_flac_picture(cover))
    };

    if vorbis.is_empty() && picture.is_none() {
        return writer.write_all(audio);
    }

    writer.write_all(b"fLaC")?;
    let mut cursor = 4_usize;

    while cursor + 4 <= audio.len() {
        let header = audio[cursor];
        let is_last = header & 0x80 != 0;
        let len = ((audio[cursor + 1] as usize) << 16)
            | ((audio[cursor + 2] as usize) << 8)
            | audio[cursor + 3] as usize;
        let data_start = cursor + 4;
        let data_end = data_start + len;
        if data_end > audio.len() {
            return writer.write_all(&audio[4..]);
        }

        let out_header = [
            if is_last { header & 0x7f } else { header },
            audio[cursor + 1],
            audio[cursor + 2],
            audio[cursor + 3],
        ];
        writer.write_all(&out_header)?;
        writer.write_all(&audio[data_start..data_end])?;

        cursor = data_end;
        if is_last {
            if !vorbis.is_empty() {
                write_flac_block(writer, 4, &vorbis, picture.is_none())?;
            }
            if let Some(picture) = picture.as_deref() {
                write_flac_block(writer, 6, picture, true)?;
            }
            writer.write_all(&audio[cursor..])?;
            return Ok(());
        }
    }

    writer.write_all(&audio[4..])
}

fn build_vorbis_comment(metadata: &NcmMetadata) -> Vec<u8> {
    let mut comments = Vec::new();
    if let Some(title) = metadata.title.as_deref() {
        comments.push(format!("TITLE={title}"));
    }
    if !metadata.artists.is_empty() {
        comments.push(format!("ARTIST={}", metadata.artists.join("/")));
    }
    if let Some(album) = metadata.album.as_deref() {
        comments.push(format!("ALBUM={album}"));
    }
    if comments.is_empty() {
        return Vec::new();
    }

    let vendor = b"unlock-music";
    let mut out = Vec::new();
    out.extend_from_slice(&(vendor.len() as u32).to_le_bytes());
    out.extend_from_slice(vendor);
    out.extend_from_slice(&(comments.len() as u32).to_le_bytes());
    for comment in comments {
        out.extend_from_slice(&(comment.len() as u32).to_le_bytes());
        out.extend_from_slice(comment.as_bytes());
    }
    out
}

fn build_flac_picture(cover: &[u8]) -> Vec<u8> {
    let mime = cover_mime(cover);
    let mut out = Vec::new();
    out.extend_from_slice(&3_u32.to_be_bytes());
    out.extend_from_slice(&(mime.len() as u32).to_be_bytes());
    out.extend_from_slice(mime.as_bytes());
    out.extend_from_slice(&0_u32.to_be_bytes());
    out.extend_from_slice(&0_u32.to_be_bytes());
    out.extend_from_slice(&0_u32.to_be_bytes());
    out.extend_from_slice(&0_u32.to_be_bytes());
    out.extend_from_slice(&0_u32.to_be_bytes());
    out.extend_from_slice(&(cover.len() as u32).to_be_bytes());
    out.extend_from_slice(cover);
    out
}

fn write_flac_block(
    writer: &mut impl Write,
    block_type: u8,
    payload: &[u8],
    is_last: bool,
) -> io::Result<()> {
    let len = payload.len();
    let header = [
        (if is_last { 0x80 } else { 0 }) | block_type,
        ((len >> 16) & 0xff) as u8,
        ((len >> 8) & 0xff) as u8,
        (len & 0xff) as u8,
    ];
    writer.write_all(&header)?;
    writer.write_all(payload)
}

fn cover_mime(cover: &[u8]) -> &'static str {
    if cover.starts_with(&[0xff, 0xd8, 0xff]) {
        "image/jpeg"
    } else if cover.starts_with(b"\x89PNG\r\n\x1a\n") {
        "image/png"
    } else if cover.starts_with(b"GIF87a") || cover.starts_with(b"GIF89a") {
        "image/gif"
    } else {
        "application/octet-stream"
    }
}

#[cfg(test)]
mod tests {
    use super::{build_key_box, decrypt_audio_chunk, write_tagged_audio, NcmMetadata};

    #[test]
    fn writes_id3_text_and_cover_for_mp3() {
        let metadata = NcmMetadata {
            title: Some("歌名".to_string()),
            album: Some("专辑".to_string()),
            artists: vec!["歌手".to_string()],
            format: Some("mp3".to_string()),
        };
        let cover = [0xff, 0xd8, 0xff, 0x00];
        let mut out = Vec::new();

        write_tagged_audio(&mut out, b"\xff\xfbmp3-data", "mp3", &metadata, &cover).unwrap();

        assert!(out.starts_with(b"ID3"));
        assert!(out.windows(4).any(|value| value == b"TIT2"));
        assert!(out.windows(4).any(|value| value == b"TPE1"));
        assert!(out.windows(4).any(|value| value == b"TALB"));
        assert!(out.windows(4).any(|value| value == b"APIC"));
        assert!(out.ends_with(b"\xff\xfbmp3-data"));
    }

    #[test]
    fn writes_vorbis_comment_and_picture_for_flac() {
        let metadata = NcmMetadata {
            title: Some("Title".to_string()),
            album: Some("Album".to_string()),
            artists: vec!["Artist".to_string()],
            format: Some("flac".to_string()),
        };
        let cover = *b"\x89PNG\r\n\x1a\n";
        let mut out = Vec::new();

        let flac = b"fLaC\x80\x00\x00\x04metaFRAME";
        write_tagged_audio(&mut out, flac, "flac", &metadata, &cover).unwrap();

        assert!(out.starts_with(b"fLaC"));
        assert!(out.windows(11).any(|value| value == b"TITLE=Title"));
        assert!(out.windows(13).any(|value| value == b"ARTIST=Artist"));
        assert!(out.windows(11).any(|value| value == b"ALBUM=Album"));
        assert!(out.windows(9).any(|value| value == b"image/png"));
        assert!(out.ends_with(b"FRAME"));
    }

    #[test]
    fn decrypts_same_bytes_across_arbitrary_read_chunks() {
        let key_box = build_key_box(b"0123456789abcdef");
        let mut one_shot = (0_u8..=255).cycle().take(1000).collect::<Vec<_>>();
        let mut chunked = one_shot.clone();

        decrypt_audio_chunk(&key_box, 0, &mut one_shot);

        let mut pos = 0_u64;
        for chunk in chunked.chunks_mut(137) {
            decrypt_audio_chunk(&key_box, pos, chunk);
            pos += chunk.len() as u64;
        }

        assert_eq!(chunked, one_shot);
    }
}
