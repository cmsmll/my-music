use crate::lyrics_search::error::{GeneralError, LyrixResult};
use aes::cipher::{Block, BlockCipherEncrypt, KeyInit};
use aes::Aes128;

const AES_BLOCK_SIZE: usize = 16;
const EAPI_KEY: &[u8; 16] = b"e82ckenh8dichen8";

pub fn eapi_encrypt(url: &str, body: &str) -> LyrixResult<String> {
    let message = format!("nobody{url}use{body}md5forencrypt");
    let digest = format!("{:x}", md5::compute(message.as_bytes()));
    let data = format!("{url}-36cd479b6b5-{body}-36cd479b6b5-{digest}");
    aes_ecb_encode_hex(&data)
}

fn aes_ecb_encode_hex(data: &str) -> LyrixResult<String> {
    let cipher = Aes128::new_from_slice(EAPI_KEY).map_err(|err| GeneralError::Internal {
        detail: format!("网易云 EAPI AES key 初始化失败: {err}"),
    })?;
    let mut encrypted = pkcs7_padded(data.as_bytes());

    for chunk in encrypted.chunks_exact_mut(AES_BLOCK_SIZE) {
        let block = <&mut Block<Aes128>>::from(
            <&mut [u8; AES_BLOCK_SIZE]>::try_from(chunk).map_err(|err| GeneralError::Internal {
                detail: format!("网易云 EAPI AES 分块失败: {err}"),
            })?,
        );
        cipher.encrypt_block(block);
    }

    Ok(hex::encode_upper(encrypted))
}

fn pkcs7_padded(data: &[u8]) -> Vec<u8> {
    let padding_len = AES_BLOCK_SIZE - data.len() % AES_BLOCK_SIZE;
    let mut padded = Vec::with_capacity(data.len() + padding_len);
    padded.extend_from_slice(data);
    padded.extend(std::iter::repeat_n(padding_len as u8, padding_len));
    padded
}
