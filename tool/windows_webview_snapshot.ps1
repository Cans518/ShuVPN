param(
    [Parameter(Mandatory)][ValidateRange(1, 2147483647)][int]$ApplicationPid,
    [Parameter(Mandatory)][ValidateRange(1, 65535)][int]$DebugPort
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$principal = [Security.Principal.WindowsPrincipal]::new([Security.Principal.WindowsIdentity]::GetCurrent())
Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class ShuWebViewSnapshot {
    public class Window {
        public string Handle { get; set; }
        public string Class { get; set; }
        public string Title { get; set; }
        public bool Visible { get; set; }
    }
    public delegate bool Callback(IntPtr window, IntPtr data);
    [DllImport("user32.dll")] public static extern bool EnumWindows(Callback callback, IntPtr data);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr window, out uint pid);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr window);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassName(IntPtr window, StringBuilder text, int size);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr window, StringBuilder text, int size);
    [DllImport("user32.dll")] public static extern IntPtr GetProcessWindowStation();
    [DllImport("user32.dll")] public static extern IntPtr GetThreadDesktop(uint thread);
    [DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId();
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern bool GetUserObjectInformation(IntPtr handle, int kind, StringBuilder text, int size, out int required);
    public static string ObjectName(IntPtr handle) {
        var text = new StringBuilder(512);
        int required;
        return GetUserObjectInformation(handle, 2, text, text.Capacity * 2, out required) ? text.ToString() : null;
    }
    public static List<Window> Windows(uint process) {
        var result = new List<Window>();
        EnumWindows((window, data) => {
            uint pid; GetWindowThreadProcessId(window, out pid);
            if (pid == process) {
                var name = new StringBuilder(256);
                var title = new StringBuilder(1024);
                GetClassName(window, name, name.Capacity);
                GetWindowText(window, title, title.Capacity);
                result.Add(new Window { Handle=window.ToInt64().ToString("x"), Class=name.ToString(), Title=title.ToString(), Visible=IsWindowVisible(window) });
            }
            return true;
        }, IntPtr.Zero);
        return result;
    }
}
'@
$processes = @(Get-CimInstance Win32_Process)
$owned = [Collections.Generic.HashSet[uint32]]::new()
[void]$owned.Add([uint32]$ApplicationPid)
do {
    $changed = $false
    foreach ($process in $processes) {
        if ($owned.Contains([uint32]$process.ParentProcessId) -and $owned.Add([uint32]$process.ProcessId)) {
            $changed = $true
        }
    }
} while ($changed)
$tree = @($processes | Where-Object { $owned.Contains([uint32]$_.ProcessId) } |
    Select-Object ProcessId,ParentProcessId,Name,ExecutablePath,CommandLine,SessionId)
$policies = @()
foreach ($location in @('HKLM:\SOFTWARE\Policies\Microsoft\Edge', 'HKCU:\SOFTWARE\Policies\Microsoft\Edge')) {
    foreach ($name in @('RemoteDebuggingAllowed', 'DeveloperToolsAvailability')) {
        $entry = Get-ItemProperty -LiteralPath $location -Name $name -ErrorAction SilentlyContinue
        if ($entry) { $policies += [PSCustomObject]@{Path=$location;Name=$name;Value=$entry.$name} }
    }
}
[PSCustomObject]@{
    ApplicationPid = $ApplicationPid
    DebugPort = $DebugPort
    HostWindowStation = [ShuWebViewSnapshot]::ObjectName([ShuWebViewSnapshot]::GetProcessWindowStation())
    HostDesktop = [ShuWebViewSnapshot]::ObjectName([ShuWebViewSnapshot]::GetThreadDesktop([ShuWebViewSnapshot]::GetCurrentThreadId()))
    Interactive = [Environment]::UserInteractive
    Administrator = $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
    SessionId = [Diagnostics.Process]::GetCurrentProcess().SessionId
    Processes = $tree
    Windows = @([ShuWebViewSnapshot]::Windows([uint32]$ApplicationPid))
    Listeners = @(Get-NetTCPConnection -LocalPort $DebugPort -ErrorAction SilentlyContinue |
        Select-Object LocalAddress,LocalPort,State,OwningProcess)
    EdgePolicies = $policies
} | ConvertTo-Json -Depth 8
