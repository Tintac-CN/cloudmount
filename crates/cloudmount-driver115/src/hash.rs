//! 上传哈希计算（秒传判定）与任意区间读取。
//!
//! 115 秒传要求（实测）：
//! - `fileid` = **整文件 SHA1，大写十六进制**
//! - `preid`  = **前 128 KiB 的 SHA1，大写十六进制**（文件不足 128 KiB 时与 `fileid` 相同）
//!
//! 二次认证（code 700/701/702）时还要对 `sign_check` 指定的**闭区间字节**算大写 SHA1。

use cloudmount_core::{Error, Result, UploadSource};
use sha1::{Digest, Sha1};
use std::io::{Read, Seek, SeekFrom};

/// preid 的字节数（前 128 KiB）。
pub const PREID_CHUNK: usize = 128 * 1024;

/// 流式读取的块大小。
const READ_CHUNK: usize = 64 * 1024;

/// 一个上传源的三元信息。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadHashes {
    /// 整文件 SHA1（大写十六进制）。
    pub fileid: String,
    /// 前 128 KiB 的 SHA1（大写十六进制）。
    pub preid: String,
    /// 字节数。
    pub size: u64,
}

/// 计算 SHA1 并转大写十六进制。
pub fn sha1_hex_upper(bytes: &[u8]) -> String {
    let mut h = Sha1::new();
    h.update(bytes);
    hex::encode_upper(h.finalize())
}

/// 计算上传源的两个 SHA1 与实际大小（流式，不整读内存）。
pub fn hashes_for(source: &UploadSource<'_>) -> Result<UploadHashes> {
    match source {
        UploadSource::Bytes(b) => {
            let head = &b[..b.len().min(PREID_CHUNK)];
            Ok(UploadHashes {
                fileid: sha1_hex_upper(b),
                preid: sha1_hex_upper(head),
                size: b.len() as u64,
            })
        }
        UploadSource::Path(p) => {
            let mut f = std::fs::File::open(p).map_err(|e| {
                Error::Io(std::io::Error::new(
                    e.kind(),
                    format!("打开 {} 失败: {e}", p.display()),
                ))
            })?;
            let mut whole = Sha1::new();
            let mut head = Sha1::new();
            let mut head_remaining = PREID_CHUNK;
            let mut total: u64 = 0;
            let mut buf = vec![0u8; READ_CHUNK];
            loop {
                let n = f.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                whole.update(&buf[..n]);
                if head_remaining > 0 {
                    let take = n.min(head_remaining);
                    head.update(&buf[..take]);
                    head_remaining -= take;
                }
                total += n as u64;
            }
            Ok(UploadHashes {
                fileid: hex::encode_upper(whole.finalize()),
                preid: hex::encode_upper(head.finalize()),
                size: total,
            })
        }
    }
}

/// 读取上传源的 `[start, end]`（**闭区间**）内容，用于二次认证。
pub fn read_source_range(
    source: &UploadSource<'_>,
    start: u64,
    end_inclusive: u64,
) -> Result<Vec<u8>> {
    if end_inclusive < start {
        return Err(Error::Invalid("sign_check 区间非法".into()));
    }
    let len = (end_inclusive - start + 1) as usize;
    match source {
        UploadSource::Bytes(b) => {
            let s = start as usize;
            let e = (end_inclusive as usize + 1).min(b.len());
            if s >= b.len() {
                return Ok(Vec::new());
            }
            Ok(b[s..e].to_vec())
        }
        UploadSource::Path(p) => {
            let mut f = std::fs::File::open(p)?;
            f.seek(SeekFrom::Start(start))?;
            let mut buf = vec![0u8; len];
            let mut filled = 0;
            while filled < len {
                let n = f.read(&mut buf[filled..])?;
                if n == 0 {
                    break;
                }
                filled += n;
            }
            buf.truncate(filled);
            Ok(buf)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_file_hashes_are_the_known_sha1() {
        let src: &[u8] = b"";
        let h = hashes_for(&UploadSource::Bytes(src)).unwrap();
        assert_eq!(h.fileid, "DA39A3EE5E6B4B0D3255BFEF95601890AFD80709");
        assert_eq!(h.preid, h.fileid, "空文件 preid 等于 fileid");
        assert_eq!(h.size, 0);
    }

    #[test]
    fn abc_hash_matches_known_value() {
        let h = hashes_for(&UploadSource::Bytes(b"abc")).unwrap();
        assert_eq!(h.fileid, "A9993E364706816ABA3E25717850C26C9CD0D89D");
    }

    #[test]
    fn small_file_preid_equals_fileid() {
        let data = vec![7u8; 1000];
        let h = hashes_for(&UploadSource::Bytes(&data)).unwrap();
        assert_eq!(h.preid, h.fileid);
        assert_eq!(h.size, 1000);
    }

    #[test]
    fn large_file_preid_is_first_128k() {
        // 构造 200KiB：前 128KiB 全 1，后 72KiB 全 2
        let mut data = vec![1u8; PREID_CHUNK];
        data.extend(vec![2u8; 200 * 1024 - PREID_CHUNK]);
        let h = hashes_for(&UploadSource::Bytes(&data)).unwrap();
        assert_ne!(h.preid, h.fileid, "大文件 preid 应只覆盖前 128KiB");
        assert_eq!(h.preid, sha1_hex_upper(&vec![1u8; PREID_CHUNK]));
        assert_eq!(h.size, 200 * 1024);
    }

    #[test]
    fn file_path_hashing_matches_bytes() {
        let dir = std::env::temp_dir().join("cloudmount_hash_test");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("sample.bin");
        let data: Vec<u8> = (0..300_000u32).map(|i| (i % 251) as u8).collect();
        std::fs::write(&p, &data).unwrap();

        let from_path = hashes_for(&UploadSource::Path(&p)).unwrap();
        let from_bytes = hashes_for(&UploadSource::Bytes(&data)).unwrap();
        assert_eq!(from_path, from_bytes, "文件路径与内存缓冲结果必须一致");

        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn range_read_is_inclusive() {
        let data: Vec<u8> = (0..=255u8).collect();
        let got = read_source_range(&UploadSource::Bytes(&data), 10, 12).unwrap();
        assert_eq!(got, vec![10u8, 11, 12], "闭区间：含 end");
    }

    #[test]
    fn range_read_clamps_past_eof() {
        let data = vec![1u8, 2, 3];
        let got = read_source_range(&UploadSource::Bytes(&data), 1, 999).unwrap();
        assert_eq!(got, vec![2u8, 3]);
    }

    #[test]
    fn range_read_rejects_inverted() {
        let data = vec![1u8];
        assert!(read_source_range(&UploadSource::Bytes(&data), 5, 1).is_err());
    }
}
