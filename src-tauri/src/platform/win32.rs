//! Thin, safe wrappers around the Win32 APIs Nexdir relies on. Each function here is
//! chosen because the std equivalent is either unsafe for user data (e.g. `fs::rename`
//! and `fs::copy` silently replace existing files) or unavailable.

use std::ffi::{c_void, OsStr, OsString};
use std::fs::File;
use std::io;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::os::windows::io::AsRawHandle;
use std::path::{Path, PathBuf};

use windows::core::PCWSTR;
use windows::Win32::Foundation::{HANDLE, HWND};
use windows::Win32::Storage::FileSystem::{
    FileBasicInfo, FileDispositionInfo, FileRenameInfo, FindClose, FindFirstFileW, FindFirstStreamW, FindNextStreamW,
    FindStreamInfoStandard, GetDiskFreeSpaceExW, GetDriveTypeW, GetFileInformationByHandle, GetLogicalDrives, GetVolumeInformationW,
    GetVolumePathNameW, MoveFileExW, SetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION, FILE_BASIC_INFO, FILE_DISPOSITION_INFO,
    FILE_RENAME_INFO, MOVE_FILE_FLAGS, WIN32_FIND_DATAW, WIN32_FIND_STREAM_DATA,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE,
};
use windows::Win32::System::Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_DWORD, RRF_RT_REG_SZ};
use windows::Win32::UI::Shell::{
    FileOperation, IFileOperation, IShellItem, SHCreateItemFromParsingName, ShellExecuteExW, FOFX_RECYCLEONDELETE, FOF_ALLOWUNDO,
    FOF_NOCONFIRMATION, FOF_NOCONFIRMMKDIR, FOF_SILENT, FOF_WANTNUKEWARNING, SEE_MASK_FLAG_NO_UI, SEE_MASK_INVOKEIDLIST, SEE_MASK_NOASYNC,
    SHELLEXECUTEINFOW,
};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

pub const IO_REPARSE_TAG_MOUNT_POINT: u32 = 0xA000_0003;
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;

fn wide(s: &OsStr) -> Vec<u16> {
    s.encode_wide().chain(std::iter::once(0)).collect()
}

/// Wide string for Win32 file APIs, using the `\\?\` prefix so paths longer than
/// MAX_PATH work. Expects an absolute, normalised path (see `security::validate_path`).
fn wide_long(path: &Path) -> Vec<u16> {
    let s = path.as_os_str().to_string_lossy();
    // `\\?\` paths are passed through unparsed, so "/" must become "\" when adding the prefix.
    let prefixed: OsString = if s.starts_with(r"\\?\") {
        path.as_os_str().to_owned()
    } else if let Some(unc) = s.strip_prefix(r"\\") {
        OsString::from(format!(r"\\?\UNC\{}", unc.replace('/', "\\")))
    } else {
        OsString::from(format!(r"\\?\{}", s.replace('/', "\\")))
    };
    wide(&prefixed)
}

fn from_wide_buf(buf: &[u16]) -> String {
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    OsString::from_wide(&buf[..len]).to_string_lossy().into_owned()
}

fn win_err(e: windows::core::Error) -> io::Error {
    // HRESULT_FROM_WIN32 values carry the Win32 code in the low word.
    let code = e.code().0;
    if (code as u32) & 0xFFFF_0000 == 0x8007_0000 {
        io::Error::from_raw_os_error(code & 0xFFFF)
    } else {
        io::Error::other(e.message())
    }
}

/// Renames or moves within one volume. Fails with `AlreadyExists` instead of replacing
/// an existing target, and atomically so (no check-then-act race).
pub fn move_no_replace(from: &Path, to: &Path) -> io::Result<()> {
    let (f, t) = (wide_long(from), wide_long(to));
    unsafe { MoveFileExW(PCWSTR(f.as_ptr()), PCWSTR(t.as_ptr()), MOVE_FILE_FLAGS(0)) }.map_err(win_err)
}

// --- Handle-bound file operations ---------------------------------------------------------
//
// These act on an open handle, i.e. on the file object Nexdir actually opened, not on whatever
// a path names at the moment of the call. A path can be renamed, deleted or replaced by another
// process between two calls; a handle keeps referring to the same file.

pub const ACCESS_DELETE: u32 = 0x0001_0000;
pub const ACCESS_GENERIC_READ: u32 = 0x8000_0000;
pub const ACCESS_GENERIC_WRITE: u32 = 0x4000_0000;
pub const ACCESS_READ_ATTRIBUTES: u32 = 0x0080;
pub const SHARE_READ: u32 = 0x1;
pub const SHARE_WRITE: u32 = 0x2;
pub const SHARE_DELETE: u32 = 0x4;
/// Open a link/junction itself instead of what it points to.
pub const FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;

fn raw(file: &File) -> HANDLE {
    HANDLE(file.as_raw_handle())
}

/// Facts about an open file, read from its handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HandleInfo {
    /// Volume serial number and file index: equal for two handles to the same file (also
    /// through 8.3 short names, different case or hard links).
    pub identity: (u32, u64),
    pub size: u64,
    pub last_write: u64,
    pub attributes: u32,
    pub links: u32,
}

impl HandleInfo {
    pub fn is_reparse_point(&self) -> bool {
        self.attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    pub fn is_dir(&self) -> bool {
        self.attributes & 0x10 != 0
    }
}

pub fn handle_info(file: &File) -> io::Result<HandleInfo> {
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    // SAFETY: valid open handle and a correctly sized output struct.
    unsafe { GetFileInformationByHandle(raw(file), &mut info) }.map_err(win_err)?;
    Ok(HandleInfo {
        identity: (
            info.dwVolumeSerialNumber,
            ((info.nFileIndexHigh as u64) << 32) | info.nFileIndexLow as u64,
        ),
        size: ((info.nFileSizeHigh as u64) << 32) | info.nFileSizeLow as u64,
        last_write: ((info.ftLastWriteTime.dwHighDateTime as u64) << 32) | info.ftLastWriteTime.dwLowDateTime as u64,
        attributes: info.dwFileAttributes,
        links: info.nNumberOfLinks,
    })
}

/// NT-namespace form of an absolute path (`\??\C:\…`, `\??\UNC\server\…`), as expected by
/// `FILE_RENAME_INFO`.
fn nt_path(path: &Path) -> Vec<u16> {
    let mut w = wide_long(path);
    w.pop(); // no terminating NUL: the length is passed explicitly
             // `\\?\` → `\??\`
    if w.len() > 4 && w[..4] == [b'\\' as u16, b'\\' as u16, b'?' as u16, b'\\' as u16] {
        w[1] = b'?' as u16;
    }
    w
}

/// Renames the file behind `file` to `to`, which must be on the same volume. With
/// `replace == false` it fails if `to` exists (atomically, no check-then-act). The handle must
/// have been opened with `ACCESS_DELETE`.
pub fn rename_by_handle(file: &File, to: &Path, replace: bool) -> io::Result<()> {
    let name = nt_path(to);
    let header = std::mem::size_of::<FILE_RENAME_INFO>();
    let bytes = name.len() * 2;
    // u64 storage keeps the struct correctly aligned.
    let mut buf = vec![0u64; (header + bytes).div_ceil(8) + 1];
    let info = buf.as_mut_ptr() as *mut FILE_RENAME_INFO;
    // SAFETY: `buf` is large enough for the header plus the name; all writes stay inside it.
    unsafe {
        (*info).Anonymous.ReplaceIfExists = replace;
        (*info).RootDirectory = HANDLE::default();
        (*info).FileNameLength = bytes as u32;
        std::ptr::copy_nonoverlapping(name.as_ptr(), (*info).FileName.as_mut_ptr(), name.len());
        SetFileInformationByHandle(raw(file), FileRenameInfo, info as *const c_void, (header + bytes) as u32)
    }
    .map_err(win_err)
}

/// Marks the file behind `file` for deletion; it disappears when the last handle closes. Only
/// the file this handle refers to can be affected — never another file that now has the same
/// name. Fails for read-only files. The handle must have `ACCESS_DELETE`.
pub fn delete_by_handle(file: &File) -> io::Result<()> {
    let info = FILE_DISPOSITION_INFO { DeleteFile: true };
    // SAFETY: valid handle and a correctly sized input struct.
    unsafe {
        SetFileInformationByHandle(
            raw(file),
            FileDispositionInfo,
            &info as *const _ as *const c_void,
            std::mem::size_of::<FILE_DISPOSITION_INFO>() as u32,
        )
    }
    .map_err(win_err)
}

/// Sets timestamps (FILETIME units; 0 = leave unchanged) and attributes through the handle.
pub fn set_basic_info_by_handle(file: &File, created: i64, written: i64, attributes: u32) -> io::Result<()> {
    let info = FILE_BASIC_INFO {
        CreationTime: created,
        LastAccessTime: 0,
        LastWriteTime: written,
        ChangeTime: 0,
        FileAttributes: attributes,
    };
    // SAFETY: valid handle and a correctly sized input struct.
    unsafe {
        SetFileInformationByHandle(
            raw(file),
            FileBasicInfo,
            &info as *const _ as *const c_void,
            std::mem::size_of::<FILE_BASIC_INFO>() as u32,
        )
    }
    .map_err(win_err)
}

/// Names of the alternate data streams of a file (e.g. `Zone.Identifier`, the
/// "downloaded from the internet" mark), without the main stream.
pub fn alternate_streams(path: &Path) -> io::Result<Vec<String>> {
    let w = wide_long(path);
    let mut data = WIN32_FIND_STREAM_DATA::default();
    // SAFETY: valid NUL-terminated path and output struct.
    let handle = match unsafe { FindFirstStreamW(PCWSTR(w.as_ptr()), FindStreamInfoStandard, &mut data as *mut _ as *mut c_void, None) } {
        Ok(h) => h,
        Err(e) if e.code().0 as u32 == 0x8007_0026 => return Ok(Vec::new()), // ERROR_HANDLE_EOF: no streams
        Err(e) => return Err(win_err(e)),
    };
    let mut names = Vec::new();
    loop {
        let name = from_wide_buf(&data.cStreamName);
        // Entries look like ":name:$DATA"; "::$DATA" is the main stream.
        if let Some(inner) = name.strip_prefix(':').and_then(|n| n.strip_suffix(":$DATA")) {
            if !inner.is_empty() {
                names.push(inner.to_string());
            }
        }
        // SAFETY: handle from FindFirstStreamW, output struct as above.
        if unsafe { FindNextStreamW(handle, &mut data as *mut _ as *mut c_void) }.is_err() {
            break;
        }
    }
    // SAFETY: handle from FindFirstStreamW.
    let _ = unsafe { FindClose(handle) };
    Ok(names)
}

/// Returns the reparse tag of `path` if it is a reparse point.
pub fn reparse_tag(path: &Path) -> Option<u32> {
    let w = wide_long(path);
    let mut data = WIN32_FIND_DATAW::default();
    let handle = unsafe { FindFirstFileW(PCWSTR(w.as_ptr()), &mut data) }.ok()?;
    let _ = unsafe { FindClose(handle) };
    (data.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0).then_some(data.dwReserved0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriveType {
    Unknown,
    Removable,
    Fixed,
    Remote,
    CdRom,
    RamDisk,
}

fn drive_type_of_root(root: &[u16]) -> DriveType {
    match unsafe { GetDriveTypeW(PCWSTR(root.as_ptr())) } {
        2 => DriveType::Removable,
        3 => DriveType::Fixed,
        4 => DriveType::Remote,
        5 => DriveType::CdRom,
        6 => DriveType::RamDisk,
        _ => DriveType::Unknown,
    }
}

/// Drive type of the volume containing `path` (follows mounted-folder volumes).
pub fn drive_type_of(path: &Path) -> DriveType {
    let w = wide(path.as_os_str());
    let mut buf = [0u16; 1024];
    if unsafe { GetVolumePathNameW(PCWSTR(w.as_ptr()), &mut buf) }.is_err() {
        return DriveType::Unknown;
    }
    drive_type_of_root(&buf)
}

pub struct RawDrive {
    pub root: String,
    pub kind: DriveType,
}

pub fn logical_drives() -> Vec<RawDrive> {
    let mask = unsafe { GetLogicalDrives() };
    (0..26u8)
        .filter(|i| mask & (1 << i) != 0)
        .map(|i| {
            let root = format!("{}:\\", (b'A' + i) as char);
            let kind = drive_type_of_root(&wide(OsStr::new(&root)));
            RawDrive { root, kind }
        })
        .collect()
}

pub struct VolumeInfo {
    pub label: String,
    pub file_system: String,
    pub total_bytes: u64,
    pub free_bytes: u64,
}

/// Queries label, file system and space of a drive root. May block for a long time on
/// disconnected network drives, so callers run it with a timeout.
pub fn volume_info(root: &str) -> io::Result<VolumeInfo> {
    let w = wide(OsStr::new(root));
    let mut label = [0u16; 261];
    let mut fs = [0u16; 261];
    unsafe { GetVolumeInformationW(PCWSTR(w.as_ptr()), Some(&mut label), None, None, None, Some(&mut fs)) }.map_err(win_err)?;
    let (mut avail, mut total, mut free) = (0u64, 0u64, 0u64);
    unsafe { GetDiskFreeSpaceExW(PCWSTR(w.as_ptr()), Some(&mut avail), Some(&mut total), Some(&mut free)) }.map_err(win_err)?;
    Ok(VolumeInfo {
        label: from_wide_buf(&label),
        file_system: from_wide_buf(&fs),
        total_bytes: total,
        free_bytes: avail,
    })
}

/// Free space available to this user on the volume that holds `dir`.
pub fn free_space(dir: &Path) -> io::Result<u64> {
    let w = wide(dir.as_os_str());
    let mut avail = 0u64;
    unsafe { GetDiskFreeSpaceExW(PCWSTR(w.as_ptr()), Some(&mut avail), None, None) }.map_err(win_err)?;
    Ok(avail)
}

/// RAII guard for a single-threaded COM apartment on the current thread.
struct ComApartment;

impl ComApartment {
    fn enter() -> io::Result<Self> {
        let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) };
        hr.ok().map_err(win_err)?;
        Ok(ComApartment)
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

/// Sends items to the Recycle Bin through the Windows shell. `FOF_WANTNUKEWARNING`
/// makes Windows ask the user before anything would be deleted permanently instead
/// (for example when an item is too large for the Recycle Bin). Must be called on a
/// thread without an existing COM apartment.
pub fn recycle(paths: &[PathBuf], owner: Option<isize>) -> io::Result<()> {
    let _com = ComApartment::enter()?;
    unsafe {
        let op: IFileOperation = CoCreateInstance(&FileOperation, None, CLSCTX_ALL).map_err(win_err)?;
        op.SetOperationFlags(
            FOF_ALLOWUNDO | FOFX_RECYCLEONDELETE | FOF_NOCONFIRMATION | FOF_WANTNUKEWARNING | FOF_NOCONFIRMMKDIR | FOF_SILENT,
        )
        .map_err(win_err)?;
        if let Some(hwnd) = owner {
            op.SetOwnerWindow(HWND(hwnd as *mut c_void)).map_err(win_err)?;
        }
        for path in paths {
            let w = wide(path.as_os_str());
            let item: IShellItem = SHCreateItemFromParsingName(PCWSTR(w.as_ptr()), None).map_err(win_err)?;
            op.DeleteItem(&item, None).map_err(win_err)?;
        }
        // Per-item outcomes are determined by the caller by checking which paths still exist.
        let _ = op.PerformOperations();
    }
    Ok(())
}

/// Opens a file with its associated application (the shell's default verb).
pub fn shell_open(path: &Path) -> io::Result<()> {
    let _com = ComApartment::enter()?;
    let file = wide(path.as_os_str());
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI,
        lpFile: PCWSTR(file.as_ptr()),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };
    unsafe { ShellExecuteExW(&mut info) }.map_err(win_err)
}

/// Opens a web address in the default browser. Callers must have validated `url`
/// (see `security::validate_external_url`).
pub fn open_url(url: &str) -> io::Result<()> {
    shell_open(Path::new(url))
}

const CURRENT_VERSION_KEY: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";

fn registry_string(key: &str, value: &str) -> Option<String> {
    let (k, v) = (wide(OsStr::new(key)), wide(OsStr::new(value)));
    let mut buf = [0u16; 256];
    let mut len = (buf.len() * 2) as u32;
    // SAFETY: NUL-terminated key/value names; `len` is the buffer size in bytes.
    let err = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            PCWSTR(k.as_ptr()),
            PCWSTR(v.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr() as *mut c_void),
            Some(&mut len),
        )
    };
    err.is_ok().then(|| from_wide_buf(&buf))
}

fn registry_dword(key: &str, value: &str) -> Option<u32> {
    let (k, v) = (wide(OsStr::new(key)), wide(OsStr::new(value)));
    let mut data = 0u32;
    let mut len = 4u32;
    // SAFETY: as above, with a 4-byte output.
    let err = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            PCWSTR(k.as_ptr()),
            PCWSTR(v.as_ptr()),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut data as *mut u32 as *mut c_void),
            Some(&mut len),
        )
    };
    err.is_ok().then_some(data)
}

/// Windows version as shown in Settings → System → About, e.g. ("Windows 11", "25H2", "26300.1234").
pub struct OsVersion {
    pub name: String,
    pub display_version: Option<String>,
    pub build: Option<String>,
}

pub fn os_version() -> OsVersion {
    let build = registry_string(CURRENT_VERSION_KEY, "CurrentBuildNumber");
    let number = build.as_deref().and_then(|b| b.parse::<u32>().ok());
    // ProductName still says "Windows 10" on Windows 11; the build number tells them apart.
    let name = match number {
        Some(n) if n >= 22000 => "Windows 11",
        Some(_) => "Windows 10",
        None => "Windows",
    };
    let ubr = registry_dword(CURRENT_VERSION_KEY, "UBR");
    OsVersion {
        name: name.to_string(),
        display_version: registry_string(CURRENT_VERSION_KEY, "DisplayVersion"),
        build: build.map(|b| match ubr {
            Some(u) => format!("{b}.{u}"),
            None => b,
        }),
    }
}

/// Shows the Windows properties sheet. Must run on the UI thread: the sheet is modeless
/// and lives on the calling thread's message loop.
pub fn shell_properties(path: &Path, owner: Option<isize>) -> io::Result<()> {
    let file = wide(path.as_os_str());
    let verb = wide(OsStr::new("properties"));
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_INVOKEIDLIST | SEE_MASK_FLAG_NO_UI,
        hwnd: HWND(owner.unwrap_or(0) as *mut c_void),
        lpVerb: PCWSTR(verb.as_ptr()),
        lpFile: PCWSTR(file.as_ptr()),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };
    unsafe { ShellExecuteExW(&mut info) }.map_err(win_err)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_path_prefixing() {
        let s = |p: &str| {
            String::from_utf16_lossy(&wide_long(Path::new(p)))
                .trim_end_matches('\0')
                .to_string()
        };
        assert_eq!(s(r"C:\a\b"), r"\\?\C:\a\b");
        assert_eq!(s(r"C:\a/b/c"), r"\\?\C:\a\b\c");
        assert_eq!(s(r"\\srv\share\x"), r"\\?\UNC\srv\share\x");
        assert_eq!(s(r"\\?\C:\a"), r"\\?\C:\a");
    }

    #[test]
    fn move_no_replace_refuses_to_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.txt");
        let b = dir.path().join("b.txt");
        std::fs::write(&a, "A").unwrap();
        std::fs::write(&b, "B").unwrap();
        let err = move_no_replace(&a, &b).unwrap_err();
        assert_eq!(err.raw_os_error(), Some(183)); // ERROR_ALREADY_EXISTS
        assert_eq!(std::fs::read_to_string(&b).unwrap(), "B");
        assert_eq!(std::fs::read_to_string(&a).unwrap(), "A");
    }

    use std::io::{Read, Write};
    use std::os::windows::fs::OpenOptionsExt;

    /// Created exclusively (CREATE_NEW), no sharing at all, with DELETE access.
    fn exclusive(path: &Path) -> io::Result<File> {
        std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .access_mode(ACCESS_GENERIC_READ | ACCESS_GENERIC_WRITE | ACCESS_DELETE)
            .share_mode(0)
            .open(path)
    }

    #[test]
    fn rename_by_handle_follows_the_file_and_never_replaces() {
        let dir = tempfile::tempdir().unwrap();
        let (a, b, c) = (dir.path().join("a.tmp"), dir.path().join("b.txt"), dir.path().join("c.txt"));
        let mut f = exclusive(&a).unwrap();
        f.write_all(b"mine").unwrap();
        std::fs::write(&b, "other").unwrap();
        let err = rename_by_handle(&f, &b, false).unwrap_err();
        assert_eq!(err.raw_os_error(), Some(183), "{err}"); // ERROR_ALREADY_EXISTS
        assert_eq!(std::fs::read_to_string(&b).unwrap(), "other");
        rename_by_handle(&f, &c, false).unwrap();
        drop(f);
        assert!(!a.exists());
        assert_eq!(std::fs::read_to_string(&c).unwrap(), "mine");
    }

    #[test]
    fn exclusive_handle_blocks_other_processes_and_delete_affects_only_that_file() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("x.tmp");
        let f = exclusive(&p).unwrap();
        assert!(exclusive(&p).is_err(), "CREATE_NEW never opens an existing file");
        assert!(std::fs::OpenOptions::new().read(true).open(&p).is_err(), "others cannot open it");
        assert!(std::fs::rename(&p, dir.path().join("y")).is_err(), "others cannot rename it");
        assert!(std::fs::remove_file(&p).is_err(), "others cannot delete it");
        // Another process (cmd) cannot replace it either.
        let out = std::process::Command::new("cmd")
            .args(["/C", "echo foreign>"])
            .arg(&p)
            .output()
            .unwrap();
        assert!(!out.status.success() || std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0) == 0);
        delete_by_handle(&f).unwrap();
        drop(f);
        assert!(!p.exists());
    }

    #[test]
    fn delete_by_handle_never_hits_a_file_that_took_the_name() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("x.tmp");
        let mine = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .access_mode(ACCESS_GENERIC_READ | ACCESS_GENERIC_WRITE | ACCESS_DELETE)
            .share_mode(SHARE_READ | SHARE_WRITE | SHARE_DELETE) // deliberately weak, to allow the swap
            .open(&p)
            .unwrap();
        // Another program renames "our" file away and puts its own file under the name.
        std::fs::rename(&p, dir.path().join("moved-away")).unwrap();
        std::fs::write(&p, "foreign").unwrap();
        delete_by_handle(&mine).unwrap();
        drop(mine);
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "foreign", "the foreign file must survive");
        assert!(
            !dir.path().join("moved-away").exists(),
            "only the file behind the handle is deleted"
        );
    }

    #[test]
    fn alternate_streams_can_be_written_while_the_main_stream_is_exclusive() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("download.exe");
        std::fs::write(&src, "MZ").unwrap();
        let mut zone = src.clone().into_os_string();
        zone.push(":Zone.Identifier");
        std::fs::write(&zone, "[ZoneTransfer]\r\nZoneId=3\r\n").unwrap();
        assert_eq!(alternate_streams(&src).unwrap(), vec!["Zone.Identifier".to_string()]);
        assert!(alternate_streams(&dir.path().join("none")).is_err());
        let plain = dir.path().join("plain.txt");
        std::fs::write(&plain, "x").unwrap();
        assert!(alternate_streams(&plain).unwrap().is_empty());

        let temp = dir.path().join("t.tmp");
        let f = exclusive(&temp).unwrap();
        let mut stream = temp.clone().into_os_string();
        stream.push(":Zone.Identifier");
        let mut s = std::fs::OpenOptions::new().write(true).create_new(true).open(&stream).unwrap();
        s.write_all(b"[ZoneTransfer]\r\nZoneId=3\r\n").unwrap();
        drop(s);
        let mut back = String::new();
        std::fs::File::open(&stream).map(|mut r| r.read_to_string(&mut back)).ok();
        drop(f);
        assert!(std::fs::read_to_string(&stream).unwrap().contains("ZoneId=3"));
    }

    #[test]
    fn handle_info_detects_hard_links_and_reparse_points() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.txt");
        std::fs::write(&a, "x").unwrap();
        let b = dir.path().join("b.txt");
        std::fs::hard_link(&a, &b).unwrap();
        let ia = handle_info(&File::open(&a).unwrap()).unwrap();
        let ib = handle_info(&File::open(&b).unwrap()).unwrap();
        assert_eq!(ia.identity, ib.identity);
        assert_eq!(ia.links, 2);
        let j = dir.path().join("junction");
        std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&j)
            .arg(dir.path())
            .output()
            .unwrap();
        let jf = std::fs::OpenOptions::new()
            .access_mode(ACCESS_READ_ATTRIBUTES)
            .custom_flags(FLAG_OPEN_REPARSE_POINT | 0x0200_0000) // FILE_FLAG_BACKUP_SEMANTICS for directories
            .open(&j)
            .unwrap();
        assert!(handle_info(&jf).unwrap().is_reparse_point());
    }

    #[test]
    fn long_paths_work() {
        let dir = tempfile::tempdir().unwrap();
        let mut deep = dir.path().to_path_buf();
        while deep.as_os_str().len() < 300 {
            deep.push("a_fairly_long_directory_name_segment");
        }
        std::fs::create_dir_all(&deep).unwrap();
        let mut f = exclusive(&deep.join("file.tmp")).unwrap();
        f.write_all(b"deep").unwrap();
        rename_by_handle(&f, &deep.join("file.txt"), false).unwrap();
        drop(f);
        move_no_replace(&deep.join("file.txt"), &deep.join("moved.txt")).unwrap();
        assert_eq!(std::fs::read_to_string(deep.join("moved.txt")).unwrap(), "deep");
    }

    #[test]
    fn detects_fixed_drive_for_temp() {
        let dir = tempfile::tempdir().unwrap();
        assert_ne!(drive_type_of(dir.path()), DriveType::Unknown);
        assert!(!logical_drives().is_empty());
    }
}
