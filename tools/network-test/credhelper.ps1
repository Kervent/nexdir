# Test helper for the Samba acceptance tests (see README.md).
# Passwords are read from creds.json and passed to Win32 APIs in memory: never printed, never on a
# command line. Only targets of the temporary test server are touched.
param(
  [Parameter(Mandatory)][ValidateSet("set-cred", "remove-cred", "connect-as", "disconnect", "list")] [string]$Action,
  [string]$User,
  [string]$Remote,
  [string]$Server = "172.29.37.90",
  [string]$Secrets = (Join-Path $PSScriptRoot "secrets\creds.json")
)
$ErrorActionPreference = "Stop"
if ($Remote -and -not $Remote.StartsWith("\\$Server\")) { throw "Only shares of the test server $Server are allowed." }

Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public static class NxCred {
  [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
  public struct CREDENTIAL { public int Flags; public int Type; public string TargetName; public string Comment;
    public System.Runtime.InteropServices.ComTypes.FILETIME LastWritten; public int CredentialBlobSize; public IntPtr CredentialBlob;
    public int Persist; public int AttributeCount; public IntPtr Attributes; public string TargetAlias; public string UserName; }
  [DllImport("advapi32.dll", CharSet = CharSet.Unicode, SetLastError = true)] public static extern bool CredWrite(ref CREDENTIAL c, int flags);
  [DllImport("advapi32.dll", CharSet = CharSet.Unicode, SetLastError = true)] public static extern bool CredDelete(string target, int type, int flags);
  [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
  public struct NETRESOURCE { public int dwScope; public int dwType; public int dwDisplayType; public int dwUsage;
    public string lpLocalName; public string lpRemoteName; public string lpComment; public string lpProvider; }
  [DllImport("mpr.dll", CharSet = CharSet.Unicode)] public static extern int WNetAddConnection2(ref NETRESOURCE r, string pw, string user, int flags);
  [DllImport("mpr.dll", CharSet = CharSet.Unicode)] public static extern int WNetCancelConnection2(string name, int flags, bool force);
  public static int Write(string target, string user, string pw) {
    var blob = Marshal.StringToCoTaskMemUni(pw);
    try {
      var c = new CREDENTIAL { Type = 2 /*DOMAIN_PASSWORD*/, TargetName = target, UserName = user, CredentialBlob = blob,
        CredentialBlobSize = pw.Length * 2, Persist = 1 /*SESSION*/, Comment = "Nexdir Samba acceptance test (temporary)" };
      return CredWrite(ref c, 0) ? 0 : Marshal.GetLastWin32Error();
    } finally { Marshal.ZeroFreeCoTaskMemUnicode(blob); }
  }
}
"@

function Get-TestPassword([string]$u) { (Get-Content $Secrets -Raw | ConvertFrom-Json).$u }

switch ($Action) {
  "set-cred" {
    # Session-only credential (removed at logoff at the latest) for the test server only.
    $r = [NxCred]::Write($Server, $User, (Get-TestPassword $User)); "set-cred $User -> $r"
  }
  "remove-cred" { $ok = [NxCred]::CredDelete($Server, 2, 0); "remove-cred -> $ok" }
  "connect-as" {
    $res = New-Object NxCred+NETRESOURCE; $res.dwType = 1; $res.lpRemoteName = $Remote
    $r = [NxCred]::WNetAddConnection2([ref]$res, (Get-TestPassword $User), $User, 4); "connect-as $User $Remote -> $r"
  }
  "disconnect" { $r = [NxCred]::WNetCancelConnection2($Remote, 0, $false); "disconnect $Remote -> $r" }
  "list" { (net use) -join "`n" }
}
