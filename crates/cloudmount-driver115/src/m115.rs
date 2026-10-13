//! m115 协议加密（下载直链接口的 `data` 参数编码与响应解码）。
//!
//! 移植自 SheltonZhu/115driver（Go, MIT）的 `pkg/crypto/m115`，协议常量
//! 与变换顺序逐行对齐：
//!
//! - **RSA**：固定 1024-bit 公钥模数 N、e=65537、PKCS#1 v1.5 type-2 填充、
//!   117 字节分块。请求方向客户端用 E 加密；响应方向服务端用私钥"签名"、
//!   客户端用同一 E 做 modpow 恢复（客户端无私钥——**同钥往返不成立**，
//!   modpow(modpow(m,E),E) ≠ m，测试只能验证填充结构与块长）。
//! - **XOR**：预置种子派生密钥 + 特殊对齐的循环异或（前 `len%4` 字节用
//!   `key[i%len]`，之后密钥流从 0 重新对齐）。
//! - **编码**（请求）：`[16 字节随机 key || 明文]` → `xor(derive(key,4))`
//!   → `reverse` → `xor(xorClientKey)` → RSA → base64。
//! - **解码**（响应）：base64 → RSA 恢复 → 跳过前 16 字节（服务端内嵌
//!   key）→ `xor(derive(内嵌key,12))` → `reverse` → `xor(derive(请求key,4))`。
//!   注意两个方向的 XOR 常量**不对称**（客户端固定 key vs 服务端派生 key），
//!   这是 115 协议本身的设计，勿"对称化"。
//!
//! 本实现只做协议要求的固定变换，不含任何设备指纹伪造或风控规避。

use cloudmount_core::{Error, Result};
use num_bigint::BigUint;

/// 固定公钥模数（1024-bit，115 协议常量）。
const N_HEX: &str = concat!(
    "8686980c0f5a24c4b9d43020cd2c22703ff3f450756529058b1cf88f09b86021",
    "36477198a6e2683149659bd122c33592fdb5ad47944ad1ea4d36c6b172aad633",
    "8c3bb6ac6227502d010993ac967d1aef00f0c8e038de2e4d3bc2ec368af2e9f1",
    "0a6f1eda4f7262f136420c07c331b871bf139f74f3010e3c4fe57df3afb71683",
);

/// 公钥指数。
const E: u64 = 0x1_0001;

/// RSA 块长（字节）。
const KEY_LEN: usize = 128;

/// 明文分块长（KEY_LEN - 11，PKCS#1 v1.5 填充开销）。
const CHUNK_LEN: usize = KEY_LEN - 11;

/// XOR 种子表（115 协议常量，144 字节）。
const XOR_KEY_SEED: [u8; 144] = [
    0xf0, 0xe5, 0x69, 0xae, 0xbf, 0xdc, 0xbf, 0x8a, 0x1a, 0x45, 0xe8, 0xbe, 0x7d, 0xa6, 0x73, 0xb8,
    0xde, 0x8f, 0xe7, 0xc4, 0x45, 0xda, 0x86, 0xc4, 0x9b, 0x64, 0x8b, 0x14, 0x6a, 0xb4, 0xf1, 0xaa,
    0x38, 0x01, 0x35, 0x9e, 0x26, 0x69, 0x2c, 0x86, 0x00, 0x6b, 0x4f, 0xa5, 0x36, 0x34, 0x62, 0xa6,
    0x2a, 0x96, 0x68, 0x18, 0xf2, 0x4a, 0xfd, 0xbd, 0x6b, 0x97, 0x8f, 0x4d, 0x8f, 0x89, 0x13, 0xb7,
    0x6c, 0x8e, 0x93, 0xed, 0x0e, 0x0d, 0x48, 0x3e, 0xd7, 0x2f, 0x88, 0xd8, 0xfe, 0xfe, 0x7e, 0x86,
    0x50, 0x95, 0x4f, 0xd1, 0xeb, 0x83, 0x26, 0x34, 0xdb, 0x66, 0x7b, 0x9c, 0x7e, 0x9d, 0x7a, 0x81,
    0x32, 0xea, 0xb6, 0x33, 0xde, 0x3a, 0xa9, 0x59, 0x34, 0x66, 0x3b, 0xaa, 0xba, 0x81, 0x60, 0x48,
    0xb9, 0xd5, 0x81, 0x9c, 0xf8, 0x6c, 0x84, 0x77, 0xff, 0x54, 0x78, 0x26, 0x5f, 0xbe, 0xe8, 0x1e,
    0x36, 0x9f, 0x34, 0x80, 0x5c, 0x45, 0x2c, 0x9b, 0x76, 0xd5, 0x1b, 0x8f, 0xcc, 0xc3, 0xb8, 0xf5,
];

/// 客户端固定 XOR key（编码方向最后一层，12 字节）。
const XOR_CLIENT_KEY: [u8; 12] = [
    0x78, 0x06, 0xad, 0x4c, 0x33, 0x86, 0x5d, 0x18, 0x4c, 0x01, 0x3f, 0x46,
];

/// 固定模数（懒解析一次）。
fn modulus() -> BigUint {
    BigUint::parse_bytes(N_HEX.as_bytes(), 16).expect("固定模数解析")
}

/// 随机 16 字节密钥。
fn generate_key() -> [u8; 16] {
    // 无需密码学强随机：key 会随密文一起传输（RSA 首块的前 16 字节），
    // 作用是混淆而非保密。
    let mut key = [0u8; 16];
    let mut state = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x9e37_79b9_7f4a_7c15)
        .max(1);
    for b in key.iter_mut() {
        // xorshift64
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        *b = (state >> 32) as u8;
    }
    key
}

/// 从种子派生 `size` 字节 XOR key（协议固定算法）。
fn xor_derive_key(seed: &[u8], size: usize) -> Vec<u8> {
    let mut key = vec![0u8; size];
    for i in 0..size {
        // Go 的 (a+b)&0xff 语义：先宽位加法再截断
        let sum = (seed[i] as u16 + XOR_KEY_SEED[size * i] as u16) & 0xff;
        key[i] = (sum as u8) ^ XOR_KEY_SEED[size * (size - i - 1)];
    }
    key
}

/// 循环异或（协议特殊对齐：前 `data%4` 字节后密钥流从 0 重新对齐）。
fn xor_transform(data: &mut [u8], key: &[u8]) {
    let data_size = data.len();
    let key_size = key.len();
    let rem = data_size % 4;
    for (i, b) in data.iter_mut().enumerate() {
        let idx = if i < rem {
            i % key_size
        } else {
            (i - rem) % key_size
        };
        *b ^= key[idx];
    }
}

/// PKCS#1 v1.5 type-2 填充：`0x00 0x02 [非零 pad] 0x00 [明文]`。
fn pkcs1_pad(input: &[u8]) -> Vec<u8> {
    let pad_size = KEY_LEN - input.len() - 3;
    let mut buf = vec![0u8; KEY_LEN];
    buf[1] = 2;
    for i in 0..pad_size {
        // pad 字节须非零：取 1..=255
        buf[2 + i] = (i % 254 + 1) as u8;
    }
    buf[pad_size + 2] = 0;
    buf[pad_size + 3..].copy_from_slice(input);
    buf
}

/// 去填充：跳过（可能被 to_bytes_be 剥掉的）前导零后找 `0x02 ... 0x00` 分界。
fn pkcs1_unpad(buf: &[u8]) -> Option<Vec<u8>> {
    let mut i = 0;
    while i < buf.len() && buf[i] == 0 {
        i += 1;
    }
    // buf[i] 应为 0x02（填充类型）
    let mut j = i + 1;
    while j < buf.len() && buf[j] != 0 {
        j += 1;
    }
    if j >= buf.len() {
        return None;
    }
    Some(buf[j + 1..].to_vec())
}

/// RSA 单块加密（填充 + modpow），输出定长 KEY_LEN。
fn rsa_encrypt_slice(input: &[u8]) -> Vec<u8> {
    let msg = BigUint::from_bytes_be(&pkcs1_pad(input));
    let ret = msg.modpow(&BigUint::from(E), &modulus()).to_bytes_be();
    // 左侧补零到块长
    let mut out = vec![0u8; KEY_LEN - ret.len()];
    out.extend_from_slice(&ret);
    out
}

/// RSA 分块加密（117 字节/块）。
fn rsa_encrypt(input: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    for chunk in input.chunks(CHUNK_LEN) {
        out.extend_from_slice(&rsa_encrypt_slice(chunk));
    }
    out
}

/// RSA 单块恢复（modpow + 去填充）。
fn rsa_decrypt_slice(input: &[u8]) -> Option<Vec<u8>> {
    let msg = BigUint::from_bytes_be(input);
    let ret = msg.modpow(&BigUint::from(E), &modulus()).to_bytes_be();
    pkcs1_unpad(&ret)
}

/// RSA 分块恢复（128 字节/块）。
fn rsa_decrypt(input: &[u8]) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    for chunk in input.chunks(KEY_LEN) {
        let part = rsa_decrypt_slice(chunk)
            .ok_or_else(|| Error::Other("m115 RSA 恢复失败（填充结构非法）".into()))?;
        out.extend_from_slice(&part);
    }
    Ok(out)
}

/// 编码请求 `data` 参数：明文 → base64 密文。
///
/// 返回 `(密文, key)`——**key 必须留存**：响应解码（[`decode`]）需要它。
pub fn encode(plaintext: &[u8]) -> (String, [u8; 16]) {
    let key = generate_key();
    let mut buf = Vec::with_capacity(16 + plaintext.len());
    buf.extend_from_slice(&key);
    buf.extend_from_slice(plaintext);

    let data = &mut buf[16..];
    xor_transform(data, &xor_derive_key(&key, 4));
    data.reverse();
    xor_transform(data, &XOR_CLIENT_KEY);

    let cipher = rsa_encrypt(&buf);
    use base64::Engine;
    (
        base64::engine::general_purpose::STANDARD.encode(&cipher),
        key,
    )
}

/// 解码响应 `data`：base64 密文 → 明文（`key` 为**请求**时生成的 key）。
pub fn decode(ciphertext: &str, key: &[u8; 16]) -> Result<Vec<u8>> {
    use base64::Engine;
    let cipher = base64::engine::general_purpose::STANDARD
        .decode(ciphertext.trim())
        .map_err(|e| Error::Other(format!("m115 base64 解码失败: {e}")))?;
    let mut data = rsa_decrypt(&cipher)?;
    if data.len() < 16 {
        return Err(Error::Other("m115 解码后数据不足 16 字节".into()));
    }
    // 前 16 字节是服务端内嵌 key
    let embedded: [u8; 16] = data[..16].try_into().expect("长度已校验");
    let output = &mut data[16..];
    xor_transform(output, &xor_derive_key(&embedded, 12));
    output.reverse();
    xor_transform(output, &xor_derive_key(key, 4));
    Ok(output.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xor_derive_key_matches_reference() {
        // 手工对照 Go 算法（seed=[1..8], size=4）：
        // key[i] = (seed[i] + SEED[4i]) & 0xff ^ SEED[4*(4-i-1)]
        let seed = [1u8, 2, 3, 4, 5, 6, 7, 8];
        let k = xor_derive_key(&seed, 4);
        assert_eq!(k.len(), 4);
        assert_eq!(k[0], ((1u16 + 0xf0) & 0xff) as u8 ^ 0x7d); // SEED[0], SEED[12]
        assert_eq!(k[1], ((2u16 + 0xbf) & 0xff) as u8 ^ 0x1a); // SEED[4], SEED[8]
        assert_eq!(k[2], ((3u16 + 0x1a) & 0xff) as u8 ^ 0xbf); // SEED[8], SEED[4]
        assert_eq!(k[3], ((4u16 + 0x7d) & 0xff) as u8 ^ 0xf0); // SEED[12], SEED[0]
    }

    #[test]
    fn xor_derive_key_no_overflow_on_max_bytes() {
        // Go 的 &0xff 截断语义：255+255 不 panic
        let seed = [255u8; 16];
        let k = xor_derive_key(&seed, 4);
        assert_eq!(k.len(), 4);
    }

    #[test]
    fn xor_transform_alignment_quirk() {
        // 前 data%4 字节后密钥流重新对齐：len=6, key=[0xff, 0x00]
        let mut data = [1u8, 2, 3, 4, 5, 6];
        let key = [0xffu8, 0x00];
        xor_transform(&mut data, &key);
        // rem=2: i=0→key[0], i=1→key[1]; i=2→key[(2-2)%2]=key[0], ...
        assert_eq!(data, [1 ^ 0xff, 2, 3 ^ 0xff, 4, 5 ^ 0xff, 6]);
    }

    #[test]
    fn pkcs1_pad_unpad_roundtrip() {
        let msg = b"hello m115 protocol";
        let padded = pkcs1_pad(msg);
        assert_eq!(padded.len(), KEY_LEN);
        assert_eq!(padded[0], 0);
        assert_eq!(padded[1], 2);
        let back = pkcs1_unpad(&padded).expect("应能去填充");
        assert_eq!(back, msg);
    }

    #[test]
    fn pkcs1_unpad_rejects_malformed() {
        // 无 0x00 分界
        assert_eq!(pkcs1_unpad(&[2u8; 64]), None);
        // 全零
        assert_eq!(pkcs1_unpad(&[0u8; 64]), None);
    }

    #[test]
    fn rsa_encrypt_block_shape() {
        // modpow 正确性只能由真实 API 验证（同钥往返不成立）；
        // 这里验证块长与密文 < N 的结构约束。
        let cipher = rsa_encrypt_slice(b"payload");
        assert_eq!(cipher.len(), KEY_LEN);
        let c = BigUint::from_bytes_be(&cipher);
        assert!(c < modulus(), "密文必须小于模数");
    }

    #[test]
    fn rsa_chunking_multi_block() {
        let msg = vec![42u8; 300]; // 300 > 117，需 3 块
        let cipher = rsa_encrypt(&msg);
        assert_eq!(cipher.len(), 3 * KEY_LEN);
    }

    #[test]
    fn xor_layers_roundtrip_symmetric() {
        // XOR 三层在"对称密钥"假设下可逆（真实协议两方向密钥不同，
        // 这里验证变换本身的可逆性）
        let key: [u8; 16] = [7u8; 16];
        let mut data = b"payload bytes here!!".to_vec();
        let layer = xor_derive_key(&key, 4);
        xor_transform(&mut data, &layer);
        data.reverse();
        xor_transform(&mut data, &layer);
        // 逆变换
        xor_transform(&mut data, &layer);
        data.reverse();
        xor_transform(&mut data, &layer);
        assert_eq!(data, b"payload bytes here!!");
    }

    #[test]
    fn encode_produces_valid_base64_of_rsa_blocks() {
        let (enc, key) = encode(br#"{"pickcode":"abc123"}"#);
        assert_eq!(key.len(), 16);
        use base64::Engine;
        let raw = base64::engine::general_purpose::STANDARD
            .decode(&enc)
            .expect("应为合法 base64");
        // 明文 24 字节 + 16 key = 40 字节 → 1 个 RSA 块 = 128 字节
        assert_eq!(raw.len(), KEY_LEN);
    }
}
