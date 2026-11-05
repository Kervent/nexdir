//! Thin adapter over the Windows network functions (`mpr.dll`, the "WNet" API) and the SMB
//! client. Nexdir never speaks SMB itself: every connection, sign-in and enumeration goes
//! through Windows, with Windows' own security settings (SMB signing, guest policy, SMB1 off).
//!
//! The `NetApi` trait lets the logic above be tested with test doubles; `WindowsNet` is the
//! real implementation. All functions here may block for a long time and are only called
//! through `pool::NETWORK`.

use serde::Serialize;
use std::path::Path;

/// What kind of object a network resource is (`dwDisplayType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ResourceKind {
    Network,
    Domain,
    Server,
    Share,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetResource {
    /// `\\server`, `\\server\share`, a workgroup name or a provider name.
    pub remote: String,
    /// Drive letter of a mapped drive (`Z:`).
    pub local: Option<String>,
    pub kind: ResourceKind,
    /// True when it can be enumerated further (providers, workgroups, servers).
    pub container: bool,
    pub comment: Option<String>,
}

pub trait NetApi: Send + Sync {
    /// Connections of the current Windows user: mapped drives and connections without a letter.
    fn connected(&self) -> Result<Vec<NetResource>, u32>;
    /// Drive mappings Windows remembers (also those that are currently disconnected).
    fn remembered(&self) -> Result<Vec<NetResource>, u32>;
    /// Children of a container in the network neighbourhood (`None` = top level). For a
    /// server (`\\server`) these are its shares.
    fn enumerate(&self, container: Option<&NetResource>) -> Result<Vec<NetResource>, u32>;
    /// Connects to `\\server\share` as the signed-in Windows user, without a drive letter and
    /// without remembering it. With `prompt_owner`, Windows shows its own sign-in dialog if
    /// credentials are needed (Nexdir never sees them).
    fn add_connection(&self, share_root: &str, prompt_owner: Option<isize>) -> Result<(), u32>;
    /// Removes a connection without forcing it (fails while files are open).
    fn cancel_connection(&self, share_root: &str) -> Result<(), u32>;
    /// Opens the path's metadata through the SMB client; the error code tells why it failed.
    fn probe(&self, path: &str) -> Result<(), u32>;
}

pub struct WindowsNet;

#[cfg(windows)]
mod imp {
    use super::*;
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows::core::{PCWSTR, PWSTR};
    use windows::Win32::Foundation::{ERROR_MORE_DATA, ERROR_NO_MORE_ITEMS, HANDLE, HWND, NO_ERROR, WIN32_ERROR};
    use windows::Win32::NetworkManagement::WNet::*;

    const RESOURCEUSAGE_CONTAINER_BIT: u32 = 0x2;
    const DISPLAY_DOMAIN: u32 = 1;
    const DISPLAY_SERVER: u32 = 2;
    const DISPLAY_SHARE: u32 = 3;
    const DISPLAY_NETWORK: u32 = 6;
    /// Upper bound on resources read from one enumeration (keeps memory bounded).
    const MAX_ITEMS: usize = 4096;

    fn wide(s: &str) -> Vec<u16> {
        OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
    }

    fn read(p: PWSTR) -> Option<String> {
        if p.is_null() {
            return None;
        }
        let s = unsafe { p.to_string() }.ok()?;
        (!s.is_empty()).then_some(s)
    }

    fn check(e: WIN32_ERROR) -> Result<(), u32> {
        if e == NO_ERROR {
            Ok(())
        } else {
            Err(e.0)
        }
    }

    fn kind(display: u32) -> ResourceKind {
        match display {
            DISPLAY_NETWORK => ResourceKind::Network,
            DISPLAY_DOMAIN => ResourceKind::Domain,
            DISPLAY_SERVER => ResourceKind::Server,
            DISPLAY_SHARE => ResourceKind::Share,
            _ => ResourceKind::Other,
        }
    }

    /// Owned copy of a NETRESOURCEW for passing a container back into WNetOpenEnumW.
    struct OwnedResource {
        _strings: Vec<Vec<u16>>,
        raw: NETRESOURCEW,
    }

    fn owned(r: &NetResource) -> OwnedResource {
        let mut remote = wide(&r.remote);
        let raw = NETRESOURCEW {
            dwScope: RESOURCE_GLOBALNET,
            dwType: RESOURCETYPE_DISK,
            dwDisplayType: match r.kind {
                ResourceKind::Network => DISPLAY_NETWORK,
                ResourceKind::Domain => DISPLAY_DOMAIN,
                ResourceKind::Server => DISPLAY_SERVER,
                ResourceKind::Share => DISPLAY_SHARE,
                ResourceKind::Other => 0,
            },
            dwUsage: if r.container { RESOURCEUSAGE_CONTAINER_BIT } else { 1 },
            lpLocalName: PWSTR::null(),
            lpRemoteName: PWSTR(remote.as_mut_ptr()),
            lpComment: PWSTR::null(),
            lpProvider: PWSTR::null(),
        };
        OwnedResource {
            _strings: vec![remote],
            raw,
        }
    }

    fn enumerate(scope: NET_RESOURCE_SCOPE, container: Option<&NetResource>) -> Result<Vec<NetResource>, u32> {
        let owned = container.map(owned);
        let mut handle = HANDLE::default();
        check(unsafe {
            WNetOpenEnumW(
                scope,
                RESOURCETYPE_DISK,
                WNET_OPEN_ENUM_USAGE(0),
                owned.as_ref().map(|o| &o.raw as *const _),
                &mut handle,
            )
        })?;
        let mut out = Vec::new();
        // u64 storage keeps the NETRESOURCEW array correctly aligned.
        let mut buf: Vec<u64> = vec![0; 2048];
        let result = loop {
            let mut count = u32::MAX;
            let mut size = (buf.len() * 8) as u32;
            let e = unsafe { WNetEnumResourceW(handle, &mut count, buf.as_mut_ptr().cast(), &mut size) };
            if e == ERROR_NO_MORE_ITEMS {
                break Ok(());
            }
            if e == ERROR_MORE_DATA {
                buf.resize((size as usize).div_ceil(8) + 1, 0);
                continue;
            }
            if e != NO_ERROR {
                break Err(e.0);
            }
            let items = unsafe { std::slice::from_raw_parts(buf.as_ptr().cast::<NETRESOURCEW>(), count as usize) };
            for r in items {
                if let Some(remote) = read(r.lpRemoteName) {
                    out.push(NetResource {
                        remote,
                        local: read(r.lpLocalName),
                        kind: kind(r.dwDisplayType),
                        container: r.dwUsage & RESOURCEUSAGE_CONTAINER_BIT != 0,
                        comment: read(r.lpComment),
                    });
                }
            }
            if out.len() >= MAX_ITEMS {
                break Ok(());
            }
        };
        unsafe {
            let _ = WNetCloseEnum(handle);
        }
        result.map(|_| out)
    }

    impl NetApi for WindowsNet {
        fn connected(&self) -> Result<Vec<NetResource>, u32> {
            enumerate(RESOURCE_CONNECTED, None)
        }

        fn remembered(&self) -> Result<Vec<NetResource>, u32> {
            enumerate(RESOURCE_REMEMBERED, None)
        }

        fn enumerate(&self, container: Option<&NetResource>) -> Result<Vec<NetResource>, u32> {
            enumerate(RESOURCE_GLOBALNET, container)
        }

        fn add_connection(&self, share_root: &str, prompt_owner: Option<isize>) -> Result<(), u32> {
            let mut remote = wide(share_root);
            let resource = NETRESOURCEW {
                dwType: RESOURCETYPE_DISK,
                lpRemoteName: PWSTR(remote.as_mut_ptr()),
                ..Default::default()
            };
            // No password and no user name: Windows uses the signed-in user (or credentials
            // already saved in the Windows Credential Manager). No drive letter, not remembered.
            let e = match prompt_owner {
                None => unsafe { WNetAddConnection2W(&resource, PCWSTR::null(), PCWSTR::null(), CONNECT_TEMPORARY) },
                Some(hwnd) => unsafe {
                    WNetAddConnection3W(
                        Some(HWND(hwnd as *mut core::ffi::c_void)),
                        &resource,
                        PCWSTR::null(),
                        PCWSTR::null(),
                        CONNECT_TEMPORARY | CONNECT_INTERACTIVE | CONNECT_PROMPT,
                    )
                },
            };
            check(e)
        }

        fn cancel_connection(&self, share_root: &str) -> Result<(), u32> {
            let name = wide(share_root);
            check(unsafe { WNetCancelConnection2W(PCWSTR(name.as_ptr()), NET_CONNECT_FLAGS(0), false) })
        }

        fn probe(&self, path: &str) -> Result<(), u32> {
            std::fs::metadata(Path::new(path))
                .map(|_| ())
                .map_err(|e| e.raw_os_error().map(|c| c as u32).unwrap_or(31))
        }
    }
}
