//! File lists on the Windows clipboard (CF_HDROP + "Preferred DropEffect"), so copy, cut
//! and paste interoperate with Windows Explorer and other applications.

use std::ffi::{c_void, OsStr, OsString};
use std::io;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::PathBuf;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL, HWND};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
};
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE};

const CF_HDROP: u32 = 15;
const DROPEFFECT_COPY: u32 = 1;
const DROPEFFECT_MOVE: u32 = 2;
/// Size of the DROPFILES header: pFiles (u32), pt (2 × i32), fNC (BOOL), fWide (BOOL).
const DROPFILES_SIZE: usize = 20;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipboardFiles {
    pub paths: Vec<PathBuf>,
    /// True when the files were cut (move on paste) rather than copied.
    pub cut: bool,
}

/// Serialises paths into a CF_HDROP buffer (DROPFILES header + double-NUL-terminated
/// UTF-16 list).
pub fn encode_drop_files(paths: &[PathBuf]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(DROPFILES_SIZE + paths.len() * 64);
    buf.extend_from_slice(&(DROPFILES_SIZE as u32).to_le_bytes()); // pFiles
    buf.extend_from_slice(&[0u8; 8]); // pt
    buf.extend_from_slice(&0u32.to_le_bytes()); // fNC
    buf.extend_from_slice(&1u32.to_le_bytes()); // fWide
    for path in paths {
        for unit in path.as_os_str().encode_wide() {
            buf.extend_from_slice(&unit.to_le_bytes());
        }
        buf.extend_from_slice(&[0, 0]);
    }
    buf.extend_from_slice(&[0, 0]);
    buf
}

/// Parses a CF_HDROP buffer. Tolerates malformed data from other applications by
/// returning what can be read safely within the buffer bounds.
pub fn decode_drop_files(buf: &[u8]) -> Vec<PathBuf> {
    if buf.len() < DROPFILES_SIZE {
        return Vec::new();
    }
    let offset = u32::from_le_bytes(buf[0..4].try_into().unwrap()) as usize;
    let wide = u32::from_le_bytes(buf[16..20].try_into().unwrap()) != 0;
    if offset >= buf.len() {
        return Vec::new();
    }
    let data = &buf[offset..];
    let mut paths = Vec::new();
    if wide {
        let units: Vec<u16> = data.as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes(*c)).collect();
        for part in units.split(|&u| u == 0) {
            if part.is_empty() {
                break;
            }
            paths.push(PathBuf::from(OsString::from_wide(part)));
        }
    } else {
        for part in data.split(|&b| b == 0) {
            if part.is_empty() {
                break;
            }
            paths.push(PathBuf::from(String::from_utf8_lossy(part).into_owned()));
        }
    }
    paths
}

fn win_err(e: windows::core::Error) -> io::Error {
    io::Error::other(e.message())
}

fn drop_effect_format() -> u32 {
    let name: Vec<u16> = OsStr::new("Preferred DropEffect").encode_wide().chain(Some(0)).collect();
    unsafe { RegisterClipboardFormatW(PCWSTR(name.as_ptr())) }
}

/// Keeps the clipboard open for the lifetime of the guard.
struct OpenGuard;

impl OpenGuard {
    fn open(owner: Option<isize>) -> io::Result<Self> {
        // The clipboard may be briefly held by another application; retry a few times.
        let hwnd = owner.map(|h| HWND(h as *mut c_void));
        let mut last = None;
        for _ in 0..10 {
            match unsafe { OpenClipboard(hwnd) } {
                Ok(()) => return Ok(OpenGuard),
                Err(e) => last = Some(e),
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        Err(win_err(last.expect("at least one attempt")))
    }
}

impl Drop for OpenGuard {
    fn drop(&mut self) {
        let _ = unsafe { CloseClipboard() };
    }
}

fn global_from_bytes(bytes: &[u8]) -> io::Result<HGLOBAL> {
    unsafe {
        let mem = GlobalAlloc(GMEM_MOVEABLE, bytes.len()).map_err(win_err)?;
        let ptr = GlobalLock(mem) as *mut u8;
        if ptr.is_null() {
            let _ = GlobalFree(Some(mem));
            return Err(io::Error::other("GlobalLock failed"));
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, bytes.len());
        let _ = GlobalUnlock(mem);
        Ok(mem)
    }
}

/// Puts files on the clipboard. `owner` must be a window of this process: Windows
/// rejects clipboard data set without an owner window.
pub fn set_files(paths: &[PathBuf], cut: bool, owner: isize) -> io::Result<()> {
    let drop = encode_drop_files(paths);
    let effect = (if cut { DROPEFFECT_MOVE } else { DROPEFFECT_COPY }).to_le_bytes();
    let _guard = OpenGuard::open(Some(owner))?;
    unsafe {
        EmptyClipboard().map_err(win_err)?;
        let files = global_from_bytes(&drop)?;
        if let Err(e) = SetClipboardData(CF_HDROP, Some(HANDLE(files.0))) {
            let _ = GlobalFree(Some(files));
            return Err(win_err(e));
        }
        // After a successful SetClipboardData the system owns the memory.
        let effect_mem = global_from_bytes(&effect)?;
        if SetClipboardData(drop_effect_format(), Some(HANDLE(effect_mem.0))).is_err() {
            let _ = GlobalFree(Some(effect_mem));
        }
    }
    Ok(())
}

fn read_global(handle: HANDLE) -> Option<Vec<u8>> {
    let mem = HGLOBAL(handle.0);
    unsafe {
        let size = GlobalSize(mem);
        let ptr = GlobalLock(mem) as *const u8;
        if ptr.is_null() || size == 0 {
            return None;
        }
        let bytes = std::slice::from_raw_parts(ptr, size).to_vec();
        let _ = GlobalUnlock(mem);
        Some(bytes)
    }
}

/// Reads files from the clipboard, or `None` when it holds no file list.
pub fn get_files() -> io::Result<Option<ClipboardFiles>> {
    if unsafe { IsClipboardFormatAvailable(CF_HDROP) }.is_err() {
        return Ok(None);
    }
    let _guard = OpenGuard::open(None)?;
    let Some(drop) = unsafe { GetClipboardData(CF_HDROP) }.ok().and_then(read_global) else {
        return Ok(None);
    };
    let paths = decode_drop_files(&drop);
    if paths.is_empty() {
        return Ok(None);
    }
    let cut = unsafe { GetClipboardData(drop_effect_format()) }
        .ok()
        .and_then(read_global)
        .and_then(|b| b.get(0..4).map(|s| u32::from_le_bytes(s.try_into().unwrap())))
        .is_some_and(|effect| effect & DROPEFFECT_MOVE != 0 && effect & DROPEFFECT_COPY == 0);
    Ok(Some(ClipboardFiles { paths, cut }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drop_files_round_trip() {
        let paths = vec![
            PathBuf::from(r"C:\a b\c.txt"),
            PathBuf::from(r"D:\日本語\😀.png"),
            PathBuf::from(r"\\srv\share\x"),
        ];
        let buf = encode_drop_files(&paths);
        assert_eq!(&buf[0..4], &20u32.to_le_bytes());
        assert_eq!(decode_drop_files(&buf), paths);
    }

    #[test]
    fn decoding_tolerates_bad_buffers() {
        assert!(decode_drop_files(&[]).is_empty());
        assert!(decode_drop_files(&[0u8; 19]).is_empty());
        let mut bad = encode_drop_files(&[PathBuf::from(r"C:\x")]);
        bad[0..4].copy_from_slice(&9999u32.to_le_bytes());
        assert!(decode_drop_files(&bad).is_empty());
        // Missing terminators: still bounded by the buffer.
        let mut truncated = encode_drop_files(&[PathBuf::from(r"C:\abc")]);
        truncated.truncate(truncated.len() - 4);
        assert_eq!(decode_drop_files(&truncated), vec![PathBuf::from(r"C:\abc")]);
    }

    #[test]
    fn decodes_ansi_lists() {
        let mut buf = vec![0u8; 20];
        buf[0..4].copy_from_slice(&20u32.to_le_bytes());
        buf.extend_from_slice(b"C:\\one\0C:\\two\0\0");
        assert_eq!(decode_drop_files(&buf), vec![PathBuf::from(r"C:\one"), PathBuf::from(r"C:\two")]);
    }
}
