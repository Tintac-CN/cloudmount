//! KM-1/KM-2 user-mode smoke tool.
//!
//! Two commands:
//!
//! ```text
//! cmflt-client handshake
//! cmflt-client create-placeholder <path> [--size N] [--file-id N]
//! ```
//!
//! `handshake` connects to `\CloudMountCmfltPort`, sends Hello, validates
//! HelloAck, then sends one Health heartbeat.
//!
//! `create-placeholder` creates a new sparse file, applies the CMFLT GUID
//! reparse point, and sets `RECALL_ON_DATA_ACCESS|OFFLINE`. It requires NTFS
//! and an elevated shell. The file is created with `CREATE_NEW`; an existing
//! path is never overwritten.

use core::ffi::c_void;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use cmflt_core::placeholder::{
    PlaceholderMeta, PlaceholderState, CMFLT_REPARSE_TAG, FILE_ATTRIBUTE_OFFLINE,
    FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS,
};
use cmflt_core::protocol::{Health, Hello, HelloAck, CAP_HEALTH, CAP_HELLO, PROTOCOL_VERSION};

const CLIENT_VERSION_WORDS: [u64; 2] = [
    u64::from_le_bytes(*b"cmflt-cl"),
    u64::from_le_bytes(*b"ient\0\0\0\0"),
];

/// `\CloudMountCmfltPort`, NUL-terminated for the user-mode FltLib API.
const PORT_NAME: &[u16] = &[
    0x5C, // '\'
    b'C' as u16,
    b'l' as u16,
    b'o' as u16,
    b'u' as u16,
    b'd' as u16,
    b'M' as u16,
    b'o' as u16,
    b'u' as u16,
    b'n' as u16,
    b't' as u16,
    b'C' as u16,
    b'm' as u16,
    b'f' as u16,
    b'l' as u16,
    b't' as u16,
    b'P' as u16,
    b'o' as u16,
    b'r' as u16,
    b't' as u16,
    0,
];

const FSCTL_SET_SPARSE: u32 = 0x0009_00C4;
const FSCTL_SET_REPARSE_POINT: u32 = 0x0009_00A4;
const FSCTL_GET_REPARSE_POINT: u32 = 0x0009_00A8;
const CREATE_NEW: u32 = 1;
const OPEN_EXISTING: u32 = 3;
const FILE_ATTRIBUTE_NORMAL: u32 = 0x0000_0080;
const GENERIC_READ_WRITE: u32 = 0x8000_0000 | 0x4000_0000;
const FILE_SHARE_ALL: u32 = 0x0000_0007;
const INVALID_FILE_ATTRIBUTES: u32 = 0xFFFF_FFFF;

extern "system" {
    fn FilterConnectCommunicationPort(
        port_name: *const u16,
        options: u32,
        context: *const c_void,
        context_size: u16,
        security_attributes: *const c_void,
        port: *mut *mut c_void,
    ) -> i32;

    fn FilterSendMessage(
        port: *mut c_void,
        input: *const c_void,
        input_size: u32,
        output: *mut c_void,
        output_size: u32,
        returned: *mut u32,
    ) -> i32;
}

#[link(name = "kernel32")]
extern "system" {
    fn CloseHandle(handle: *mut c_void) -> i32;
    fn CreateFileW(
        name: *const u16,
        access: u32,
        share: u32,
        security: *const c_void,
        disposition: u32,
        flags: u32,
        template: *mut c_void,
    ) -> *mut c_void;
    fn DeviceIoControl(
        handle: *mut c_void,
        code: u32,
        input: *const c_void,
        input_size: u32,
        output: *mut c_void,
        output_size: u32,
        returned: *mut u32,
        overlapped: *mut c_void,
    ) -> i32;
    fn GetFileInformationByHandleEx(
        handle: *mut c_void,
        class: i32,
        info: *mut c_void,
        size: u32,
    ) -> i32;
    fn SetFileInformationByHandle(
        handle: *mut c_void,
        class: i32,
        info: *const c_void,
        size: u32,
    ) -> i32;
    fn SetFilePointerEx(
        handle: *mut c_void,
        distance: i64,
        new_position: *mut i64,
        method: u32,
    ) -> i32;
    fn SetEndOfFile(handle: *mut c_void) -> i32;
    fn GetFileAttributesW(name: *const u16) -> u32;
}

#[repr(C)]
struct FileBasicInfo {
    creation_time: i64,
    last_access_time: i64,
    last_write_time: i64,
    change_time: i64,
    file_attributes: u32,
}

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        None | Some("handshake") => run_handshake(),
        Some("create-placeholder") => {
            let path = args.next().ok_or_else(|| usage().to_string())?;
            let mut size = 0u64;
            let mut file_id = 0u64;
            while let Some(arg) = args.next() {
                match arg.as_str() {
                    "--size" => {
                        size = args
                            .next()
                            .ok_or("--size requires a value")?
                            .parse()
                            .map_err(|_| "--size must be a u64".to_string())?
                    }
                    "--file-id" => {
                        file_id = args
                            .next()
                            .ok_or("--file-id requires a value")?
                            .parse()
                            .map_err(|_| "--file-id must be a u64".to_string())?
                    }
                    other => return Err(format!("unknown option: {other}")),
                }
            }
            create_placeholder(Path::new(&path), size, file_id)
        }
        Some("--help" | "-h") => {
            println!("{}", usage());
            Ok(())
        }
        Some(other) => Err(format!("unknown command: {other}\n{}", usage())),
    }
}

fn usage() -> &'static str {
    "usage:\n  cmflt-client handshake\n  cmflt-client create-placeholder <path> [--size N] [--file-id N]"
}

fn run_handshake() -> Result<(), String> {
    let mut port: *mut c_void = core::ptr::null_mut();
    let hr = unsafe {
        FilterConnectCommunicationPort(
            PORT_NAME.as_ptr(),
            0,
            core::ptr::null(),
            0,
            core::ptr::null(),
            &mut port,
        )
    };
    if hr < 0 {
        return Err(format!("FilterConnectCommunicationPort failed: 0x{hr:08X}"));
    }
    if port.is_null() {
        return Err("FilterConnectCommunicationPort returned a null port".to_string());
    }

    let result = handshake(port);
    unsafe { CloseHandle(port) };
    result
}

fn handshake(port: *mut c_void) -> Result<(), String> {
    let mut input = [0u8; 128];
    let mut output = [0u8; 128];

    let hello = Hello {
        proto_version: PROTOCOL_VERSION,
        capabilities: CAP_HELLO | CAP_HEALTH,
        engine_version: CLIENT_VERSION_WORDS,
    };
    let hello_len = hello
        .encode(&mut input)
        .map_err(|error| format!("encode Hello: {error:?}"))?;
    let mut returned = 0u32;
    let hr = unsafe {
        FilterSendMessage(
            port,
            input.as_ptr() as *const c_void,
            hello_len as u32,
            output.as_mut_ptr() as *mut c_void,
            output.len() as u32,
            &mut returned,
        )
    };
    if hr < 0 {
        return Err(format!("Hello send failed: 0x{hr:08X}"));
    }
    let ack = HelloAck::decode(&output[..returned as usize])
        .map_err(|error| format!("decode HelloAck: {error:?}"))?;
    if ack.status != 0 {
        return Err(format!("kernel rejected handshake: 0x{:08X}", ack.status));
    }
    println!(
        "handshake ok: proto={} capabilities=0x{:08X} kernel={}",
        ack.proto_version,
        ack.capabilities,
        printable(&ack.kernel_version)
    );

    let health = Health {
        uptime_ms: 0,
        inflight: 0,
        completed: 0,
        failed: 0,
        breaker_trips: 0,
    };
    let health_len = health
        .encode(&mut input)
        .map_err(|error| format!("encode Health: {error:?}"))?;
    let hr = unsafe {
        FilterSendMessage(
            port,
            input.as_ptr() as *const c_void,
            health_len as u32,
            output.as_mut_ptr() as *mut c_void,
            output.len() as u32,
            &mut returned,
        )
    };
    if hr < 0 {
        return Err(format!("Health send failed: 0x{hr:08X}"));
    }
    if returned != 0 {
        return Err(format!("Health unexpectedly returned {returned} bytes"));
    }
    println!("health ok");
    Ok(())
}

fn create_placeholder(path: &Path, size: u64, file_id: u64) -> Result<(), String> {
    let distance = i64::try_from(size).map_err(|_| "size does not fit in i64".to_string())?;
    let wide_path = wide(path);
    let handle = unsafe {
        CreateFileW(
            wide_path.as_ptr(),
            GENERIC_READ_WRITE,
            FILE_SHARE_ALL,
            core::ptr::null(),
            CREATE_NEW,
            FILE_ATTRIBUTE_NORMAL,
            core::ptr::null_mut(),
        )
    };
    if handle as isize == -1 {
        return Err(format!(
            "CreateFileW({}) failed: {}",
            path.display(),
            std::io::Error::last_os_error()
        ));
    }

    let result = create_placeholder_inner(handle, &wide_path, size, distance, file_id);
    unsafe { CloseHandle(handle) };
    if result.is_err() {
        let _ = std::fs::remove_file(path);
    }
    result
}

fn create_placeholder_inner(
    handle: *mut c_void,
    wide_path: &[u16],
    size: u64,
    distance: i64,
    file_id: u64,
) -> Result<(), String> {
    let mut returned = 0u32;

    if size > 0 {
        let sparse = 1u8;
        let ok = unsafe {
            DeviceIoControl(
                handle,
                FSCTL_SET_SPARSE,
                &sparse as *const u8 as *const c_void,
                1,
                core::ptr::null_mut(),
                0,
                &mut returned,
                core::ptr::null_mut(),
            )
        };
        if ok == 0 {
            return Err(format!(
                "FSCTL_SET_SPARSE failed: {}",
                std::io::Error::last_os_error()
            ));
        }
    }

    let ok = unsafe { SetFilePointerEx(handle, distance, core::ptr::null_mut(), 0) };
    if ok == 0 {
        return Err(format!(
            "SetFilePointerEx failed: {}",
            std::io::Error::last_os_error()
        ));
    }
    let ok = unsafe { SetEndOfFile(handle) };
    if ok == 0 {
        return Err(format!(
            "SetEndOfFile failed: {}",
            std::io::Error::last_os_error()
        ));
    }

    let meta = PlaceholderMeta {
        file_id,
        size,
        hydrated_bytes: 0,
        state: PlaceholderState::Placeholder,
        flags: 0,
    };
    let blob = meta
        .encode_blob()
        .map_err(|error| format!("encode placeholder: {error:?}"))?;
    let ok = unsafe {
        DeviceIoControl(
            handle,
            FSCTL_SET_REPARSE_POINT,
            blob.as_ptr() as *const c_void,
            blob.len() as u32,
            core::ptr::null_mut(),
            0,
            &mut returned,
            core::ptr::null_mut(),
        )
    };
    if ok == 0 {
        return Err(format!(
            "FSCTL_SET_REPARSE_POINT failed: {}",
            std::io::Error::last_os_error()
        ));
    }

    let mut info = FileBasicInfo {
        creation_time: 0,
        last_access_time: 0,
        last_write_time: 0,
        change_time: 0,
        file_attributes: 0,
    };
    let info_size = core::mem::size_of::<FileBasicInfo>() as u32;
    let ok = unsafe {
        GetFileInformationByHandleEx(
            handle,
            0,
            &mut info as *mut FileBasicInfo as *mut c_void,
            info_size,
        )
    };
    if ok == 0 {
        return Err(format!(
            "GetFileInformationByHandleEx failed: {}",
            std::io::Error::last_os_error()
        ));
    }
    info.file_attributes |= FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS | FILE_ATTRIBUTE_OFFLINE;
    let ok = unsafe {
        SetFileInformationByHandle(
            handle,
            0,
            &info as *const FileBasicInfo as *const c_void,
            info_size,
        )
    };
    if ok == 0 {
        return Err(format!(
            "SetFileInformationByHandle failed: {}",
            std::io::Error::last_os_error()
        ));
    }

    // Reopen once so a loaded cmflt driver's post-create callback sees the
    // new reparse tag and can add RECALL_ON_DATA_ACCESS (user mode cannot).
    let reopen = unsafe {
        CreateFileW(
            wide_path.as_ptr(),
            GENERIC_READ_WRITE,
            FILE_SHARE_ALL,
            core::ptr::null(),
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            core::ptr::null_mut(),
        )
    };
    if reopen as isize != -1 {
        unsafe { CloseHandle(reopen) };
    }

    let attrs = unsafe { GetFileAttributesW(wide_path.as_ptr()) };
    if attrs == INVALID_FILE_ATTRIBUTES {
        return Err(format!(
            "GetFileAttributesW failed: {}",
            std::io::Error::last_os_error()
        ));
    }

    let mut reparse = [0u8; 1024];
    let ok = unsafe {
        DeviceIoControl(
            handle,
            FSCTL_GET_REPARSE_POINT,
            core::ptr::null(),
            0,
            reparse.as_mut_ptr() as *mut c_void,
            reparse.len() as u32,
            &mut returned,
            core::ptr::null_mut(),
        )
    };
    if ok == 0 {
        return Err(format!(
            "FSCTL_GET_REPARSE_POINT failed: {}",
            std::io::Error::last_os_error()
        ));
    }
    let tag = reparse
        .get(..4)
        .and_then(|bytes| bytes.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or("short reparse response")?;
    if tag != CMFLT_REPARSE_TAG {
        return Err(format!("unexpected reparse tag: 0x{tag:08X}"));
    }
    let stored = PlaceholderMeta::decode(&reparse[..returned as usize])
        .map_err(|error| format!("decode reparse metadata: {error:?}"))?;
    if stored != meta {
        return Err("reparse metadata mismatch".to_string());
    }

    let recall = attrs & FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS != 0;
    let offline = attrs & FILE_ATTRIBUTE_OFFLINE != 0;
    println!(
        "placeholder metadata created: {} size={} file_id={} state={:?} attrs=0x{:08X} recall={} offline={}",
        String::from_utf16_lossy(&wide_path[..wide_path.len() - 1]),
        size,
        file_id,
        stored.state,
        attrs,
        recall,
        offline
    );
    if !recall {
        println!(
            "warning: RECALL_ON_DATA_ACCESS is missing; load the cmflt driver and reopen the file so its post-create callback can set it"
        );
    }
    Ok(())
}

fn wide(path: &Path) -> Vec<u16> {
    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

fn printable(words: &[u64; 2]) -> String {
    let low = words[0].to_le_bytes();
    let high = words[1].to_le_bytes();
    let mut bytes = [0u8; 16];
    bytes[..8].copy_from_slice(&low);
    bytes[8..].copy_from_slice(&high);
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}
