//! PKCE（RFC 7636）实现。
//!
//! 115 官方要求：`code_challenge = BASE64URL-NOPAD(SHA256(code_verifier))`，
//! `code_challenge_method = "sha256"`。verifier 需为 43–128 位随机串，
//! 字符集为 RFC 3986 unreserved（`A-Z a-z 0-9 - . _ ~`）。
//!
//! 注意：p115client 里 `"0"*64` 的固定 verifier 是偷懒默认值，**非官方要求**，
//! 且会削弱 PKCE 的绑定作用，本项目一律随机生成。

use crate::{Error, Result};
use base64::Engine as _;
use sha2::{Digest, Sha256};

/// PKCE verifier 的字符集（RFC 3986 unreserved）。
const UNRESERVED: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~";

/// verifier 长度（官方要求 43–128，取 64 平衡熵与长度）。
pub const VERIFIER_LEN: usize = 64;

/// 一次 PKCE 会话。
#[derive(Debug, Clone)]
pub struct Pkce {
    /// 随机 verifier（换取 token 时回传）。
    pub verifier: String,
    /// 挑战方式，固定 "sha256"。
    pub method: &'static str,
}

impl Pkce {
    /// 生成随机 PKCE（使用 OS CSPRNG）。
    pub fn generate() -> Result<Self> {
        // 拒绝采样边界：66 * 3 = 198，丢弃 >= 198 的字节以消除模偏置
        const REJECT_FROM: u8 = 198;
        let mut verifier = String::with_capacity(VERIFIER_LEN);
        let mut buf = [0u8; VERIFIER_LEN];
        while verifier.len() < VERIFIER_LEN {
            getrandom::fill(&mut buf).map_err(|e| Error::Other(format!("CSPRNG 失败: {e}")))?;
            for &b in buf.iter() {
                if b < REJECT_FROM {
                    verifier.push(UNRESERVED[(b as usize) % UNRESERVED.len()] as char);
                    if verifier.len() == VERIFIER_LEN {
                        break;
                    }
                }
            }
        }
        Ok(Self {
            verifier,
            method: "sha256",
        })
    }

    /// 从固定 verifier 构造（用于测试与确定性场景）。
    pub fn from_verifier(verifier: impl Into<String>) -> Self {
        Self {
            verifier: verifier.into(),
            method: "sha256",
        }
    }

    /// 计算 `code_challenge` = BASE64URL-NOPAD(SHA256(verifier))。
    pub fn challenge(&self) -> String {
        let digest = Sha256::digest(self.verifier.as_bytes());
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn challenge_matches_rfc7636_appendix_b() {
        // RFC 7636 附录 B 的官方测试向量
        let pkce = Pkce::from_verifier("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk");
        assert_eq!(
            pkce.challenge(),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn generated_verifier_is_valid() {
        let a = Pkce::generate().unwrap();
        let b = Pkce::generate().unwrap();
        assert_eq!(a.verifier.len(), VERIFIER_LEN);
        assert_ne!(a.verifier, b.verifier, "两次生成不应相同");
        assert!(a.verifier.bytes().all(|c| UNRESERVED.contains(&c)));
        assert_eq!(
            a.challenge().len(),
            43,
            "SHA256 的 base64url 挑战固定 43 字符"
        );
        assert!(!a.challenge().contains('='), "必须 NOPAD");
    }
}
