//! KM-1 user-mode smoke client.
//!
//! Connects to `\CloudMountCmfltPort`, sends a `Hello`, validates the
//! `HelloAck`, then sends one `Health` heartbeat. The protocol code is the
//! same `cmflt-core` code used by the kernel driver.
//!
//! Run it after installing/loading the testsigned driver:
//!
//! ```text
//! cmflt-client.exe
//! ```
//!
//! Exit code 0 means the connection and handshake succeeded.

use core::ffi::c_void;

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
}

fn main() -> Result<(), String> {
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
