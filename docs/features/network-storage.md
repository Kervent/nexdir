# Network shares and NAS

> **New in 0.6.0-alpha.1** (alpha). Tested on Windows 11 with a Samba server; not yet with NAS
> hardware or on Windows 10 (see *Known limitations*).

Nexdir can show and open Windows file shares (SMB): NAS devices (Synology, QNAP, TrueNAS and
others), Samba servers, shared folders on other Windows PCs, routers with a storage function and
network drives with a drive letter. Nexdir uses the SMB client that is built into Windows; it does
not implement any network protocol itself and does not change Windows' security settings.

Only SMB file shares are supported. FTP, SFTP, WebDAV, NFS and cloud storage are not part of this
feature.

## Requirements

- Windows 10 or 11 with the normal Windows file sharing client (enabled by default).
- The share must be reachable from this PC, and your account (or the account you sign in with)
  must have permission for it.
- Current Windows versions block SMB1 and unauthenticated guest access. Nexdir does not turn
  these protections off. Older NAS devices may need a firmware update or a user account.

## The Network view

Open **Network** in the sidebar (or type `\\NAS` in the address bar). The view shows:

| Section | Content | Contacts the network? |
|---|---|---|
| Network drives and connections | Drive letters mapped to shares (e.g. *Media (Z:)*) and shares Windows is connected to. Remembered drives that are not connected are marked *Not connected*. | No: Windows reports these locally. |
| Saved | Shares you saved in Nexdir. They stay visible when the NAS is off. Status: *Not checked* until you check it. | No |
| Shares on \\\\server | The shares of one server, after you open `\\server`. Hidden shares ending in `$` are left out, like in Explorer. | Yes, that one server. |
| Found on the network | Result of **Search the network** (see below). | Yes, after you click. |

The same share under a drive letter and as `\\server\share` is shown once.

Each card has a status (*Connected*, *Not connected*, *Not reachable*, *Sign-in required*,
*Access denied*, *Not available*, *Connection lost*, *Blocked by Windows*, *Other account in use*,
*No answer*, *Not checked*) and a menu (right-click, the ⋯ button or Shift+F10):

- **Open**, **Open in new tab**, **Open in other pane** (switches split view on)
- **Check connection**: asks the server now
- **Sign in with Windows…**: Windows' own sign-in dialog (see below)
- **Save in Network** / **Remove from saved** / **Rename…** (saved shares: changes only the name)
- **Copy network path**, **Properties**
- **Disconnect**: only for connections Nexdir created itself

## Searching the network

**Search the network** runs in the background with a progress bar and a **Stop** button. It
lists:

1. servers that Windows' network list reports (as far as Windows reports them), and
2. servers you already use (connections, network drives, saved shares),

and asks each of them for its visible shares. Shares are shown only when the server really
reported them. A server whose shares cannot be listed is shown with the reason.

Windows does not guarantee that every NAS appears. On current Windows versions the network list is
often empty, because it depends on network discovery settings and on how the server announces
itself. **Adding a share by its path always works.** Nexdir never scans IP address ranges or ports
and never searches in the background.

## Connecting a share

**Add network share** opens a dialog:

- **Server or network path**, for example `\\192.168.178.10\Media`, `\\NAS-SERVER\Backups`,
  `\\homeserver\Documents`, `//nas/share` or `smb://nas.local/share`. The dialog says
  whether the input is a server, a share or a folder in a share. A server alone opens its list of
  shares.
- **Name** (optional): how the share appears in Nexdir.
- **Save in Network**: saves the path and the name, never a password.

Nexdir connects with your Windows account first. If the share needs a different user name and
password, the dialog shows *Sign-in required* and a **Sign in…** button. It opens **Windows' own
sign-in dialog**. Windows can remember the credentials in its Credential Manager if you tick that
option there. Nexdir never sees, stores or logs user names or passwords.

Windows allows only one account per server at a time. If you are already connected to the server
with another account, Nexdir shows *Other account in use* and does not change the existing
connection.

## Working with files on a share

Shares open like normal folders, in tabs and in split view (for example: local folder left, NAS
right). Available on shares:

| Action | On a share | Notes |
|---|---|---|
| Browse, filter, sort, preview, open files, Properties | ✓ | Programs and scripts still need confirmation before they run. |
| New folder, rename | ✓ | |
| Copy PC → share, share → PC, share → other share | ✓ | Staged copy: the file is written to a temporary name, verified (size and BLAKE3 hash), flushed and only then renamed into place. An existing file is never overwritten without your choice (Skip / Keep both / Replace). |
| Move inside the same share | ✓ | A rename on the server; nothing is deleted. |
| Move between a share and another location | — | Not available yet. Copy instead and delete the original yourself. |
| Delete (Recycle Bin) | — | Shares have no Recycle Bin and Nexdir never deletes permanently. |
| Storage analysis, duplicate search, search | ✓ (read-only) | Duplicate *removal* on shares is refused, because it would delete. |

If a share does not answer, Nexdir shows that it is not reachable after a short time and stays
usable; other tabs and the other pane keep working.

**Copies to a share on slow networks.** Progress and speed show the real transfer, and **Cancel**
stops a copy within a fraction of a second. On links with high latency Nexdir copies more slowly
than Explorer, because each part is confirmed by the server before the next one is sent.

**If the server disappears during a copy**, the copy is reported as failed, the original stays
untouched, nothing appears under the final name and an existing file on the share keeps its old
content. A temporary file named `~nexdir-….partial` may remain on the share; the message names it,
and you can delete it once the share is back. Folders on a share that refuse access offer **Sign in…** and **Go to Network**.

Up (Alt+↑) from the root of a share opens the share list of its server.

## Connections and closing Nexdir

Nexdir creates connections without a drive letter and without making them permanent. When
Nexdir closes, it releases the connections it created (gently: a connection that another program
still uses stays). Network drives and connections made in Explorer or with `net use` are never
disconnected or changed by Nexdir.

## Privacy

Opening the Network view reads only what Windows already knows. Servers are contacted when you
open a share, check a connection, list a server's shares or search the network. Nexdir sends no
data anywhere else. Saved shares are stored in `network.json` in Nexdir's settings folder (path
and name only).

## Known limitations

- Moving across a share boundary and deleting on shares are not available yet.
- Drive letters cannot be assigned from Nexdir (use Explorer or `net use`); existing drive letters
  are shown and used.
- Windows' network list is often empty on current Windows versions, so discovery may only show
  servers you already know.
- Tested with a real Samba server on a virtual network (WSL), over Windows' SMB loopback and with
  test doubles. Not yet tested on NAS hardware (see
  [network-storage-tests.md](../testing/network-storage-tests.md) and
  [network-storage-acceptance.md](../reports/network-storage-acceptance.md)).
