//! ec115 协议加密（上传 `initupload` 请求的 ECDH-AES 通道）。
//!
//! 移植自 SheltonZhu/115driver（Go, MIT）的 `pkg/crypto/ec115`：
//!
//! - **ECDH**：P-224，115 固定公钥（56 字节 = X‖Y，各 28 字节）。
//!   共享密钥 28 字节：`key = secret[0..16]`、`iv = secret[12..28]`
//!   （**两段重叠 4 字节**——Go 原实现如此，勿"修正"）。
//! - **加密**：AES-128-CBC（Go 手写了 CBC：xorKey 初始为 IV，每块输出
//!   作为下一块的 xor key——即标准 CBC）+ PKCS7 填充。
//! - **解密**：AES-128-CBC 解密 → 明文格式 `[长度(2 字节 LE)][LZ4 块]`，
//!   解压 LZ4。
//! - **EncodeToken**：48 字节 token（公钥分两段与随机字节异或 + 时间戳
//!   + CRC32 盐值校验）→ base64，作为 `k_ec` 查询参数。
//!
//! 本实现只做协议要求的固定变换，不含任何风控规避。

use aes::cipher::{BlockDecrypt, BlockEncrypt, KeyInit};
use aes::Aes128;
use cloudmount_core::{Error, Result};
use p224::ecdh::EphemeralSecret;
use p224::elliptic_curve::sec1::ToSec1Point;
use p224::PublicKey;

/// 115 固定公钥（P-224 未压缩坐标 X‖Y，各 28 字节）。
const REMOTE_PUB_KEY: [u8; 56] = [
    0x57, 0xA2, 0x92, 0x57, 0xCD, 0x23, 0x20, 0xE5, 0xD6, 0xD1, 0x43, 0x32, 0x2F, 0xA4, 0xBB, 0x8A,
    0x3C, 0xF9, 0xD3, 0xCC, 0x62, 0x3E, 0xF5, 0xED, 0xAC, 0x62, 0xB7, 0x67, 0x8A, 0x89, 0xC9, 0x1A,
    0x83, 0xBA, 0x80, 0x0D, 0x61, 0x29, 0xF5, 0x22, 0xD0, 0x34, 0xC8, 0x95, 0xDD, 0x24, 0x65, 0x24,
    0x3A, 0xDD, 0xC2, 0x50, 0x95, 0x3B, 0xEE, 0xBA,
];

/// EncodeToken 的 CRC32 盐值。
const CRC_SALT: &[u8] = b"^j>WD3Kr?J2gLFjD4W2y@";

/// P-224 坐标字节长。
const P224_LEN: usize = 28;

/// ec115 加密器（一次 ECDH 会话）。
pub struct EcdhCipher {
    /// AES-128 密钥（共享密钥前 16 字节）。
    key: [u8; 16],
    /// CBC IV（共享密钥后 16 字节，与 key 重叠 4 字节）。
    iv: [u8; 16],
    /// 本端公钥的 token 编码形态（[29, 0x02/0x03, X(28)]，30 字节）。
    pub_key: [u8; 30],
}

impl EcdhCipher {
    /// 生成新的 ECDH 会话。
    pub fn new() -> Result<Self> {
        // 远端公钥：未压缩 SEC1（0x04 || X || Y）
        let mut uncompressed = Vec::with_capacity(1 + REMOTE_PUB_KEY.len());
        uncompressed.push(0x04);
        uncompressed.extend_from_slice(&REMOTE_PUB_KEY);
        let remote = PublicKey::from_sec1_bytes(&uncompressed)
            .map_err(|e| Error::Other(format!("远端公钥解析失败: {e:?}")))?;

        // 本端密钥对 + ECDH 共享密钥
        use p224::elliptic_curve::Generate as _;
        let secret = EphemeralSecret::generate_from_rng(&mut OsRng);
        let public = secret.public_key();
        let shared = secret.diffie_hellman(&remote);
        let secret_bytes = shared.raw_secret_bytes();

        let mut key = [0u8; 16];
        let mut iv = [0u8; 16];
        key.copy_from_slice(&secret_bytes[..16]);
        iv.copy_from_slice(&secret_bytes[secret_bytes.len() - 16..]);

        // 本端公钥 token 形态：[29, 0x02/0x03, X(28)]（Go 前缀长度字节）
        let compressed = public.to_sec1_point(true);
        let comp = compressed.as_bytes();
        if comp.len() != P224_LEN + 1 {
            return Err(Error::Other("本端公钥压缩编码长度异常".into()));
        }
        let mut pub_key = [0u8; 30];
        pub_key[0] = (P224_LEN + 1) as u8;
        pub_key[1..].copy_from_slice(comp);

        Ok(Self { key, iv, pub_key })
    }

    /// 加密（AES-128-CBC + PKCS7 填充）。
    pub fn encrypt(&self, plaintext: &[u8]) -> Vec<u8> {
        // PKCS7
        let pad_len = 16 - (plaintext.len() % 16);
        let mut data = plaintext.to_vec();
        data.extend(std::iter::repeat_n(pad_len as u8, pad_len));

        // 手写 CBC（与 Go 逐块语义一致）
        let cipher = Aes128::new((&self.key).into());
        let mut prev = self.iv;
        for chunk in data.chunks_mut(16) {
            for i in 0..16 {
                chunk[i] ^= prev[i];
            }
            let block = aes::cipher::generic_array::GenericArray::from_mut_slice(chunk);
            cipher.encrypt_block(block);
            prev.copy_from_slice(chunk);
        }
        data
    }

    /// 解密（AES-128-CBC）→ `[长度(2 LE)][LZ4 块]` → LZ4 解压。
    pub fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        tracing::debug!(
            target: "cm_ec115",
            ct_len = ciphertext.len(),
            ct_head = %ciphertext.iter().take(24).map(|b| format!("{b:02x}")).collect::<String>(),
            key = %self.key.iter().map(|b| format!("{b:02x}")).collect::<String>(),
            iv = %self.iv.iter().map(|b| format!("{b:02x}")).collect::<String>(),
            "decrypt 输入"
        );
        // 截掉尾部非整块（Go 原样处理）
        let usable = ciphertext.len() - ciphertext.len() % 16;
        if usable == 0 {
            return Err(Error::Other("ec115 密文不足一个块".into()));
        }
        // 手写 CBC 解密：p_i = D(c_i) XOR c_{i-1}——**prev 存密文块**
        // （曾把明文拷进 prev：首块碰巧正确、第二块起全错；单测加解密
        // 镜像同 bug 假绿，真机数据才炸出）
        let mut buf = ciphertext[..usable].to_vec();
        let cipher = Aes128::new((&self.key).into());
        let mut prev: aes::cipher::generic_array::GenericArray<u8, aes::cipher::consts::U16> =
            self.iv.into();
        for chunk in buf.chunks_mut(16) {
            let c_i = *aes::cipher::generic_array::GenericArray::from_slice(chunk);
            let mut block = c_i;
            cipher.decrypt_block(&mut block);
            for i in 0..16 {
                block[i] ^= prev[i];
            }
            chunk.copy_from_slice(&block);
            prev = c_i;
        }
        // 明文：[长度(2 字节 LE)][LZ4 块]
        if buf.len() < 2 {
            return Err(Error::Other("ec115 明文不足长度头".into()));
        }
        tracing::debug!(
            target: "cm_ec115",
            plain_hex = %buf.iter().map(|b| format!("{b:02x}")).collect::<String>(),
            "CBC 解密后（全量）"
        );
        let lz4_len = buf[0] as usize | ((buf[1] as usize) << 8);
        if lz4_len == 0 || 2 + lz4_len > buf.len() {
            return Err(Error::Other("ec115 LZ4 长度头非法".into()));
        }
        // 与 Go 原实现一致：预分配 64KB（initupload 响应为小 JSON），
        // decompress_into 需要预知输出容量——超出时报错带期望值，可按需放大
        let mut out = vec![0u8; 0x10000];
        let n = lz4_flex::block::decompress_into(&buf[2..2 + lz4_len], &mut out)
            .map_err(|e| Error::Other(format!("ec115 LZ4 解压失败: {e}")))?;
        out.truncate(n);
        Ok(out)
    }

    /// 生成 `k_ec` token（48 字节结构 → base64）。
    pub fn encode_token(&self, timestamp_millis: i64) -> String {
        let mut r1 = rand_byte();
        let mut r2 = rand_byte();
        // 随机字节不能为 0（会导致段前缀暴露公钥明文）
        while r1 == 0 {
            r1 = rand_byte();
        }
        while r2 == 0 {
            r2 = rand_byte();
        }

        let mut tmp = Vec::with_capacity(48);
        // 公钥前 15 字节 ^ r1
        for b in &self.pub_key[..15] {
            tmp.push(b ^ r1);
        }
        tmp.push(r1);
        tmp.push(0x73 ^ r1);
        tmp.push(r1);
        tmp.push(r1);
        tmp.push(r1);
        // 时间戳（大端 4 字节，倒序异或）
        let t = (timestamp_millis as u32).to_be_bytes();
        for i in 0..4 {
            tmp.push(r1 ^ t[3 - i]);
        }
        // 公钥后 15 字节 ^ r2
        for b in &self.pub_key[15..] {
            tmp.push(b ^ r2);
        }
        tmp.push(r2);
        tmp.push(0x01 ^ r2);
        tmp.push(r2);
        tmp.push(r2);
        tmp.push(r2);

        // CRC32（盐值前缀）大端倒序附加
        let mut crc_input = CRC_SALT.to_vec();
        crc_input.extend_from_slice(&tmp);
        let crc = crc32_ieee(&crc_input).to_be_bytes();
        for i in 0..4 {
            tmp.push(crc[3 - i]);
        }

        use base64::Engine;
        base64::engine::general_purpose::STANDARD.encode(&tmp)
    }
}

impl Default for EcdhCipher {
    fn default() -> Self {
        Self::new().expect("ECDH 会话生成不应失败")
    }
}

/// OS 随机源（ECDH 私钥需要密码学强度）。
struct OsRng;

impl rand_core::TryRng for OsRng {
    type Error = core::convert::Infallible;
    fn try_next_u32(&mut self) -> std::result::Result<u32, Self::Error> {
        let mut b = [0u8; 4];
        self.try_fill_bytes(&mut b)?;
        Ok(u32::from_be_bytes(b))
    }
    fn try_next_u64(&mut self) -> std::result::Result<u64, Self::Error> {
        let mut b = [0u8; 8];
        self.try_fill_bytes(&mut b)?;
        Ok(u64::from_be_bytes(b))
    }
    fn try_fill_bytes(&mut self, dst: &mut [u8]) -> std::result::Result<(), Self::Error> {
        // OS 熵源失败不可恢复（与 std OsRng 语义一致）
        getrandom::fill(dst).expect("OS 随机源失败");
        Ok(())
    }
}
impl rand_core::TryCryptoRng for OsRng {}

/// 简单随机字节（非密码学强度：token 的 r1/r2 本身随密文明文传输）。
fn rand_byte() -> u8 {
    let mut state = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x9e37_79b9_7f4a_7c15)
        .max(1);
    state ^= state << 13;
    state ^= state >> 7;
    state ^= state << 17;
    (state >> 56) as u8 | 1 // 恒非零
}

/// IEEE CRC32（查表法，标准多项式 0xEDB88320 反射）。
fn crc32_ieee(data: &[u8]) -> u32 {
    static TABLE: std::sync::OnceLock<[u32; 256]> = std::sync::OnceLock::new();
    let table = TABLE.get_or_init(|| {
        let mut t = [0u32; 256];
        for (i, e) in t.iter_mut().enumerate() {
            let mut c = i as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 {
                    0xEDB8_8320 ^ (c >> 1)
                } else {
                    c >> 1
                };
            }
            *e = c;
        }
        t
    });
    let mut crc = 0xFFFF_FFFFu32;
    for b in data {
        crc = table[((crc ^ *b as u32) & 0xFF) as usize] ^ (crc >> 8);
    }
    crc ^ 0xFFFF_FFFF
}

/// 上传签名（GenerateSignature）：`SHA1(user_id + fileID + target + "0")`
/// → `userkey + sha1 + "000000"` → SHA1 → 大写十六进制。
pub fn generate_signature(user_id: &str, userkey: &str, file_id: &str, target: &str) -> String {
    use sha1::{Digest, Sha1};
    let mut h = Sha1::new();
    h.update(format!("{user_id}{file_id}{target}0").as_bytes());
    let inner = hex::encode(h.finalize());
    let mut h2 = Sha1::new();
    h2.update(format!("{userkey}{inner}000000").as_bytes());
    hex::encode(h2.finalize()).to_uppercase()
}

/// 上传 token（GenerateToken）：
/// `MD5(md5Salt + fileID + fileSize + signKey + signVal + userID + t +
///      MD5(userID) + appVer)` → 小写十六进制。
pub fn generate_token(
    user_id: &str,
    file_id: &str,
    timestamp: &str,
    file_size: &str,
    sign_key: &str,
    sign_val: &str,
    app_ver: &str,
) -> String {
    use md5::{Digest, Md5};
    let user_id_md5 = {
        let mut h = Md5::new();
        h.update(user_id.as_bytes());
        hex::encode(h.finalize())
    };
    const MD5_SALT: &str = "Qclm8MGWUv59TnrR0XPg";
    // app_ver 必须与表单 appversion 字段**完全一致**（服务端校验 token）
    let input = format!(
        "{MD5_SALT}{file_id}{file_size}{sign_key}{sign_val}{user_id}{timestamp}{user_id_md5}{app_ver}"
    );
    let mut h = Md5::new();
    h.update(input.as_bytes());
    hex::encode(h.finalize())
}

/// ECDH 测试对端（**仅测试辅助**；`#[doc(hidden)]`，生产路径绝不依赖）。
///
/// 回环硬前提（F-14 / T-24）：无人持有 115 固定公钥（`REMOTE_PUB_KEY`）对应的
/// 私钥，测试必须以**测试密钥对替换远端公钥**：
/// - [`TestEcdhPeer::generate`] 生成服务端密钥对，公钥（56 字节 = X‖Y）经
///   `Cookie115Provider::with_ecdh_remote_pub` 注入驱动；
/// - [`TestEcdhPeer::session`] 从请求 URL 的 `k_ec` token 逆解码客户端公钥
///   （[`EcdhCipher::encode_token`] 的逆变换）→ ECDH 共享密钥 → 与客户端同构的
///   key/iv（重叠 4 字节派生）；
/// - [`PeerSession::decrypt_request`] / [`PeerSession::encrypt_response`] 镜像
///   请求/响应方向的 CBC + PKCS7 / `[长度(2 LE)][LZ4]` 帧（复用本文件既有代码）。
#[doc(hidden)]
pub mod test_peer {
    /// 测试服务端（持有测试密钥对的私钥）。
    pub struct TestEcdhPeer {}

    /// 与客户端的一次 ECDH 会话（key/iv 派生与 [`super::EcdhCipher`] 一致）。
    pub struct PeerSession {}

    impl TestEcdhPeer {
        /// 生成服务端测试密钥对；返回 (peer, 远端公钥 56 字节 = X‖Y)。
        ///
        /// 公钥注入 `Cookie115Provider::with_ecdh_remote_pub`。
        pub fn generate() -> (Self, [u8; 56]) {
            todo!("U1-GREEN: 生成 P-224 测试密钥对（未压缩公钥 56 字节 = X‖Y）")
        }

        /// 从 `k_ec` token 解出客户端公钥（`encode_token` 的逆变换）并派生共享会话。
        pub fn session(&self, k_ec: &str) -> cloudmount_core::Result<PeerSession> {
            let _ = k_ec;
            todo!("U1-GREEN: k_ec 逆解码 → ECDH 共享密钥 → key/iv（重叠 4 字节）")
        }
    }

    impl PeerSession {
        /// 解密 initupload 请求体（CBC + PKCS7；请求方向无 LZ4 帧）→ 表单明文。
        pub fn decrypt_request(&self, body: &[u8]) -> Vec<u8> {
            let _ = body;
            todo!("U1-GREEN: CBC 解密 + PKCS7 去填充")
        }

        /// 加密响应：LZ4 压缩 + `[长度(2 LE)]` 帧 + CBC → 客户端 `decrypt` 可解。
        pub fn encrypt_response(&self, plain: &[u8]) -> cloudmount_core::Result<Vec<u8>> {
            let _ = plain;
            todo!("U1-GREEN: LZ4 + 长度帧 + CBC 加密")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32_ieee_known_vector() {
        // IEEE CRC32("123456789") = 0xCBF43926
        assert_eq!(crc32_ieee(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32_ieee(b""), 0);
    }

    #[test]
    fn ecdh_cipher_cbc_roundtrip() {
        // CBC 加解密互逆（绕过 LZ4 层直接验 CBC：解密后应是
        // [PKCS7 明文]，但解密路径会尝试 LZ4——改为验密文结构 +
        // 手工逆 CBC 验证互逆性）
        let cipher = EcdhCipher::new().expect("ECDH 会话");
        let pt = b"hello ec115 protocol!!"; // 21 字节 → 填充到 32
        let ct = cipher.encrypt(pt);
        assert_eq!(ct.len() % 16, 0, "CBC 密文应为整块");
        assert_eq!(ct.len(), 32);
        // 逆 CBC（手工）应还原出 PKCS7 填充的明文
        let aes = Aes128::new((&cipher.key).into());
        let mut prev = cipher.iv;
        let mut recovered = Vec::new();
        for chunk in ct.chunks(16) {
            let c_i = *aes::cipher::generic_array::GenericArray::from_slice(chunk);
            let mut block = c_i;
            aes.decrypt_block(&mut block);
            let mut plain = block.to_vec();
            for i in 0..16 {
                plain[i] ^= prev[i];
            }
            prev.copy_from_slice(&c_i);
            recovered.extend_from_slice(&plain);
        }
        // PKCS7 去填充
        let pad = recovered.last().copied().unwrap_or(0) as usize;
        assert!(pad > 0 && pad <= 16);
        assert_eq!(&recovered[..recovered.len() - pad], pt);
    }

    #[test]
    fn encode_token_shape() {
        let cipher = EcdhCipher::new().expect("ECDH 会话");
        let token = cipher.encode_token(1_791_875_040);
        use base64::Engine;
        let raw = base64::engine::general_purpose::STANDARD
            .decode(&token)
            .expect("合法 base64");
        assert_eq!(raw.len(), 48, "token 固定 48 字节");
    }

    #[test]
    fn generate_signature_shape() {
        let sig = generate_signature("123", "key", "FILEID", "U_1_0");
        assert_eq!(sig.len(), 40, "SHA1 十六进制 40 字符");
        assert!(sig
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()));
    }

    #[test]
    fn generate_token_shape() {
        let token = generate_token("123", "FILEID", "1791875040000", "1024", "", "", "36.0.1");
        assert_eq!(token.len(), 32, "MD5 十六进制 32 字符");
        assert!(token
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }

    #[test]
    fn ecdh_shared_secret_deterministic_per_key() {
        // 同一会话内 key/iv 长度合法；不同会话密钥不同
        let a = EcdhCipher::new().unwrap();
        let b = EcdhCipher::new().unwrap();
        assert_ne!(a.key, b.key, "两次 ECDH 会话应产生不同密钥");
    }
}
