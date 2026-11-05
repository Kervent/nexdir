//! Translation of Windows network errors into a connection status and a message for the user.
//!
//! The numeric codes are the documented Win32 error codes; a test checks them against the
//! constants of the Windows SDK bindings. Messages never contain user names or passwords
//! (Windows does not return them in these errors, and Nexdir never has the password).

use serde::Serialize;

pub const ERROR_ACCESS_DENIED: u32 = 5;
pub const ERROR_NOT_SUPPORTED: u32 = 50;
pub const ERROR_BAD_NETPATH: u32 = 53;
pub const ERROR_UNEXP_NET_ERR: u32 = 59;
pub const ERROR_NETNAME_DELETED: u32 = 64;
pub const ERROR_BAD_DEV_TYPE: u32 = 66;
pub const ERROR_BAD_NET_NAME: u32 = 67;
pub const ERROR_ALREADY_ASSIGNED: u32 = 85;
pub const ERROR_INVALID_PASSWORD: u32 = 86;
pub const ERROR_SEM_TIMEOUT: u32 = 121;
pub const ERROR_BAD_PROVIDER: u32 = 1204;
pub const ERROR_SESSION_CREDENTIAL_CONFLICT: u32 = 1219;
pub const ERROR_NO_NETWORK: u32 = 1222;
pub const ERROR_CANCELLED: u32 = 1223;
pub const ERROR_CONNECTION_REFUSED: u32 = 1225;
pub const ERROR_NETWORK_UNREACHABLE: u32 = 1231;
pub const ERROR_HOST_UNREACHABLE: u32 = 1232;
pub const ERROR_LOGON_FAILURE: u32 = 1326;
pub const ERROR_ACCOUNT_RESTRICTION: u32 = 1327;
pub const ERROR_PASSWORD_EXPIRED: u32 = 1330;
pub const ERROR_ACCOUNT_DISABLED: u32 = 1331;
pub const ERROR_DOWNGRADE_DETECTED: u32 = 1265;
pub const ERROR_SMB_GUEST_LOGON_BLOCKED: u32 = 1272;
pub const ERROR_NO_NET_OR_BAD_PATH: u32 = 1203;
pub const ERROR_NOT_CONNECTED: u32 = 2250;
pub const ERROR_OPEN_FILES: u32 = 2401;
pub const ERROR_DEVICE_IN_USE: u32 = 2404;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum NetworkStatus {
    /// The share answered.
    Connected,
    /// Known to Windows (remembered drive or favourite) but not checked or not connected.
    NotConnected,
    /// The server does not answer or the network cannot be reached.
    Unreachable,
    /// The share requires a user name and password.
    SignInRequired,
    /// Signed in, but the account has no permission for the share.
    AccessDenied,
    /// The server answered but the share or folder does not exist.
    NotFound,
    /// The connection was lost during an action.
    ConnectionLost,
    /// Windows refused the connection because of a security policy (guest access, signing).
    BlockedByPolicy,
    /// Another connection to this server already uses different credentials.
    CredentialConflict,
    /// The user cancelled the Windows sign-in dialog.
    Cancelled,
    /// Nexdir stopped waiting for Windows (the request may still finish in the background).
    TimedOut,
    Error,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusReport {
    pub status: NetworkStatus,
    pub message: String,
    /// Win32 error code for the diagnostics view; `None` when the operation succeeded.
    pub code: Option<u32>,
}

impl StatusReport {
    pub fn connected() -> Self {
        Self {
            status: NetworkStatus::Connected,
            message: "The share is reachable.".into(),
            code: None,
        }
    }

    pub fn timed_out() -> Self {
        Self {
            status: NetworkStatus::TimedOut,
            message: "The server did not answer in time. It may be switched off or very slow. Try again later.".into(),
            code: None,
        }
    }
}

/// Maps a Win32 error code from a network or file call to a status and an explanation.
pub fn from_code(code: u32) -> StatusReport {
    use NetworkStatus as S;
    let (status, message) = match code {
        ERROR_LOGON_FAILURE | ERROR_INVALID_PASSWORD => (S::SignInRequired, "Sign-in required. The share needs a user name and password that Windows does not have."),
        ERROR_ACCOUNT_RESTRICTION | ERROR_PASSWORD_EXPIRED | ERROR_ACCOUNT_DISABLED => {
            (S::SignInRequired, "The server rejected the account (restricted, disabled or the password has expired).")
        }
        ERROR_ACCESS_DENIED => (S::AccessDenied, "Access denied. Your account does not have permission for this share or folder."),
        ERROR_SESSION_CREDENTIAL_CONFLICT => (
            S::CredentialConflict,
            "Windows is already connected to this server with a different user name. Windows allows only one account per server; disconnect the other connection first (Nexdir does not change it).",
        ),
        ERROR_SMB_GUEST_LOGON_BLOCKED => (
            S::BlockedByPolicy,
            "Windows blocks guest access to this share for security reasons. Set up a user account on the NAS and sign in with it; Nexdir does not weaken this policy.",
        ),
        ERROR_DOWNGRADE_DETECTED => (
            S::BlockedByPolicy,
            "Windows refused the connection because the server's security could not be verified (for example SMB signing). Nexdir does not weaken this setting.",
        ),
        ERROR_BAD_NETPATH | ERROR_NETWORK_UNREACHABLE | ERROR_HOST_UNREACHABLE | ERROR_NO_NETWORK | ERROR_CONNECTION_REFUSED | ERROR_SEM_TIMEOUT | ERROR_NO_NET_OR_BAD_PATH => {
            (S::Unreachable, "Not reachable. The server does not answer: it may be switched off, offline or the name may be wrong.")
        }
        ERROR_BAD_NET_NAME => (S::NotFound, "Not available. The server answered, but this share does not exist."),
        2 | 3 => (S::NotFound, "Not available. The folder does not exist on the share."),
        ERROR_NETNAME_DELETED | ERROR_UNEXP_NET_ERR => (S::ConnectionLost, "The connection was interrupted. Check the network and try again."),
        ERROR_NOT_CONNECTED => (S::NotConnected, "Not connected."),
        ERROR_CANCELLED => (S::Cancelled, "Sign-in was cancelled."),
        ERROR_ALREADY_ASSIGNED => (S::Error, "This drive letter is already in use."),
        ERROR_BAD_DEV_TYPE | ERROR_BAD_PROVIDER | ERROR_NOT_SUPPORTED => (S::Error, "This location is not a Windows file share (SMB)."),
        ERROR_OPEN_FILES | ERROR_DEVICE_IN_USE => (S::Error, "The connection is in use (open files or folders). Close them first."),
        _ => (S::Error, "Windows could not connect to the share."),
    };
    StatusReport {
        status,
        message: message.into(),
        code: Some(code),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::Foundation as F;

    /// The codes above must be the real Win32 values (checked against the SDK bindings).
    #[test]
    fn codes_match_the_windows_sdk() {
        let pairs = [
            (ERROR_ACCESS_DENIED, F::ERROR_ACCESS_DENIED),
            (ERROR_NOT_SUPPORTED, F::ERROR_NOT_SUPPORTED),
            (ERROR_BAD_NETPATH, F::ERROR_BAD_NETPATH),
            (ERROR_UNEXP_NET_ERR, F::ERROR_UNEXP_NET_ERR),
            (ERROR_NETNAME_DELETED, F::ERROR_NETNAME_DELETED),
            (ERROR_BAD_DEV_TYPE, F::ERROR_BAD_DEV_TYPE),
            (ERROR_BAD_NET_NAME, F::ERROR_BAD_NET_NAME),
            (ERROR_ALREADY_ASSIGNED, F::ERROR_ALREADY_ASSIGNED),
            (ERROR_INVALID_PASSWORD, F::ERROR_INVALID_PASSWORD),
            (ERROR_SEM_TIMEOUT, F::ERROR_SEM_TIMEOUT),
            (ERROR_BAD_PROVIDER, F::ERROR_BAD_PROVIDER),
            (ERROR_SESSION_CREDENTIAL_CONFLICT, F::ERROR_SESSION_CREDENTIAL_CONFLICT),
            (ERROR_NO_NETWORK, F::ERROR_NO_NETWORK),
            (ERROR_CANCELLED, F::ERROR_CANCELLED),
            (ERROR_CONNECTION_REFUSED, F::ERROR_CONNECTION_REFUSED),
            (ERROR_NETWORK_UNREACHABLE, F::ERROR_NETWORK_UNREACHABLE),
            (ERROR_HOST_UNREACHABLE, F::ERROR_HOST_UNREACHABLE),
            (ERROR_LOGON_FAILURE, F::ERROR_LOGON_FAILURE),
            (ERROR_ACCOUNT_RESTRICTION, F::ERROR_ACCOUNT_RESTRICTION),
            (ERROR_PASSWORD_EXPIRED, F::ERROR_PASSWORD_EXPIRED),
            (ERROR_ACCOUNT_DISABLED, F::ERROR_ACCOUNT_DISABLED),
            (ERROR_DOWNGRADE_DETECTED, F::ERROR_DOWNGRADE_DETECTED),
            (ERROR_SMB_GUEST_LOGON_BLOCKED, F::ERROR_SMB_GUEST_LOGON_BLOCKED),
            (ERROR_NO_NET_OR_BAD_PATH, F::ERROR_NO_NET_OR_BAD_PATH),
            (ERROR_NOT_CONNECTED, F::ERROR_NOT_CONNECTED),
            (ERROR_OPEN_FILES, F::ERROR_OPEN_FILES),
            (ERROR_DEVICE_IN_USE, F::ERROR_DEVICE_IN_USE),
        ];
        for (ours, sdk) in pairs {
            assert_eq!(ours, sdk.0, "code {ours}");
        }
    }

    #[test]
    fn maps_errors_to_statuses() {
        assert_eq!(from_code(ERROR_LOGON_FAILURE).status, NetworkStatus::SignInRequired);
        assert_eq!(from_code(ERROR_ACCESS_DENIED).status, NetworkStatus::AccessDenied);
        assert_eq!(from_code(ERROR_BAD_NETPATH).status, NetworkStatus::Unreachable);
        assert_eq!(from_code(ERROR_NETWORK_UNREACHABLE).status, NetworkStatus::Unreachable);
        assert_eq!(from_code(ERROR_BAD_NET_NAME).status, NetworkStatus::NotFound);
        assert_eq!(
            from_code(ERROR_SESSION_CREDENTIAL_CONFLICT).status,
            NetworkStatus::CredentialConflict
        );
        assert_eq!(from_code(ERROR_NETNAME_DELETED).status, NetworkStatus::ConnectionLost);
        assert_eq!(from_code(ERROR_NOT_CONNECTED).status, NetworkStatus::NotConnected);
        assert_eq!(from_code(ERROR_SMB_GUEST_LOGON_BLOCKED).status, NetworkStatus::BlockedByPolicy);
        assert_eq!(from_code(ERROR_CANCELLED).status, NetworkStatus::Cancelled);
        assert_eq!(from_code(ERROR_ALREADY_ASSIGNED).code, Some(85));
        assert_eq!(from_code(999_999).status, NetworkStatus::Error);
    }

    /// The system message for each code is the one the mapping assumes (checked through
    /// FormatMessage on this machine; the wording is localised, so only presence is checked).
    #[test]
    fn windows_knows_every_mapped_code() {
        for code in [
            ERROR_BAD_NETPATH,
            ERROR_BAD_NET_NAME,
            ERROR_LOGON_FAILURE,
            ERROR_SESSION_CREDENTIAL_CONFLICT,
            ERROR_SMB_GUEST_LOGON_BLOCKED,
            ERROR_NOT_CONNECTED,
        ] {
            let text = std::io::Error::from_raw_os_error(code as i32).to_string();
            assert!(!text.contains("Unknown error"), "{code}: {text}");
        }
    }
}
