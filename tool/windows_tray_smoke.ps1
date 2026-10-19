param(
    [Parameter(Mandatory)][int]$ApplicationPid,
    [ValidateSet('inspect', 'show', 'hide', 'account', 'settings', 'http', 'exit')][string]$Action = 'inspect',
    [string]$IconScreenshot
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class ShuTrayProbe {
    [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] public struct IconId { public uint Size; public IntPtr Window; public uint Id; public Guid Guid; }
    [StructLayout(LayoutKind.Sequential)] public struct Point { public int X, Y; }
    public delegate bool EnumWindow(IntPtr window, IntPtr data);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumWindow callback, IntPtr data);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassName(IntPtr window, StringBuilder name, int maximum);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr window, out uint pid);
    [DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr window, uint message, IntPtr wparam, IntPtr lparam);
    [DllImport("user32.dll")] public static extern int GetMenuItemCount(IntPtr menu);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetMenuString(IntPtr menu, uint index, StringBuilder text, int maximum, uint flags);
    [DllImport("user32.dll")] public static extern uint GetMenuState(IntPtr menu, uint index, uint flags);
    [DllImport("user32.dll")] public static extern IntPtr GetSubMenu(IntPtr menu, int index);
    [DllImport("user32.dll")] public static extern bool GetMenuItemRect(IntPtr window, IntPtr menu, uint index, out Rect rect);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint flags, uint dx, uint dy, uint data, UIntPtr extra);
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
    [DllImport("user32.dll")] public static extern int GetSystemMetrics(int index);
    [DllImport("shell32.dll")] public static extern int Shell_NotifyIconGetRect(ref IconId icon, out Rect rect);
    [DllImport("user32.dll")] public static extern bool GetCursorPos(out Point point);
    [DllImport("user32.dll")] public static extern IntPtr WindowFromPoint(Point point);
    public static string PointerTarget() {
        Point point; GetCursorPos(out point);
        var window = WindowFromPoint(point);
        uint pid; GetWindowThreadProcessId(window, out pid);
        var name = new StringBuilder(256); GetClassName(window, name, name.Capacity);
        return String.Format("{0},{1} -> {2}:{3}:{4}", point.X, point.Y, window, pid, name);
    }
    public static int TrayRect(uint process, out Rect rect) {
        IntPtr found = IntPtr.Zero;
        EnumWindows((window, data) => {
            uint pid; GetWindowThreadProcessId(window, out pid);
            var name = new StringBuilder(256); GetClassName(window, name, name.Capacity);
            if (pid == process && name.ToString() == "tray_icon_app") { found = window; return false; }
            return true;
        }, IntPtr.Zero);
        rect = new Rect();
        if (found == IntPtr.Zero) return -1;
        int last = -1;
        for (uint id = 1; id <= 64; id++) {
            var icon = new IconId { Size=(uint)Marshal.SizeOf(typeof(IconId)), Window=found, Id=id };
            last = Shell_NotifyIconGetRect(ref icon, out rect);
            if (last == 0) return 0;
        }
        return last;
    }
    public static string PopupDebug() {
        var result = new StringBuilder();
        EnumWindows((window, data) => {
            uint pid; GetWindowThreadProcessId(window, out pid);
            var name = new StringBuilder(256); GetClassName(window, name, name.Capacity);
            if (name.ToString().Contains("32768")) result.AppendFormat("{0}:{1}:{2};", window, pid, name);
            return true;
        }, IntPtr.Zero);
        return result.ToString();
    }
    public static IntPtr Popup(uint process) {
        IntPtr found = IntPtr.Zero;
        EnumWindows((window, data) => {
            uint pid; GetWindowThreadProcessId(window, out pid);
            var name = new StringBuilder(256); GetClassName(window, name, name.Capacity);
            if (pid == process && name.ToString() == "#32768") { found = window; return false; }
            return true;
        }, IntPtr.Zero);
        return found;
    }
}
'@
[ShuTrayProbe]::SetProcessDPIAware() | Out-Null
$scope = [System.Windows.Automation.TreeScope]::Descendants
$root = [System.Windows.Automation.AutomationElement]::RootElement
$buttonCondition = [System.Windows.Automation.PropertyCondition]::new(
    [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
    [System.Windows.Automation.ControlType]::Button
)
function Find-TrayIcon {
    foreach ($button in $root.FindAll($scope, $buttonCondition)) {
        if ($button.Current.Name -like 'ShuVPN*尚未连接*' -and -not $button.Current.IsOffscreen) { return $button }
    }
    return $null
}
$icon = Find-TrayIcon
if (-not $icon) {
    $overflow = $root.FindAll($scope, $buttonCondition) | Where-Object {
        $_.Current.AutomationId -eq 'SystemTrayIcon' -or
        $_.Current.Name -match '显示隐藏的图标|Show hidden icons|隐藏图标菜单'
    } | Select-Object -First 1
    if (-not $overflow) { throw 'The taskbar notification overflow button was not found' }
    $invoke = $overflow.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)
    $invoke.Invoke()
    $deadline = [DateTime]::UtcNow.AddSeconds(5)
    do {
        Start-Sleep -Milliseconds 100
        $icon = Find-TrayIcon
    } until ($icon -or [DateTime]::UtcNow -ge $deadline)
}
if (-not $icon) { throw 'The real ShuVPN tray icon was not found' }
$liveIcon = $false
for ($attempt = 0; $attempt -lt 5; $attempt++) {
    $icon = Find-TrayIcon
    if (-not $icon) { break }
    try {
        $candidate = $icon.Current.BoundingRectangle
        [ShuTrayProbe]::SetCursorPos([int]($candidate.X + $candidate.Width / 2), [int]($candidate.Y + $candidate.Height / 2)) | Out-Null
        # Explorer removes stale icons from previously terminated test processes on hover.
        Start-Sleep -Milliseconds 300
        if (-not $icon.Current.IsOffscreen) { $liveIcon = $true; break }
    } catch [System.Windows.Automation.ElementNotAvailableException] {
        continue
    }
}
if (-not $liveIcon) { throw 'No live ShuVPN tray icon survived Explorer refresh' }
$bounds = $icon.Current.BoundingRectangle
$iconName = $icon.Current.Name
if ($bounds.IsEmpty) { throw 'The tray icon has no screen bounds' }
$nativeBounds = [ShuTrayProbe+Rect]::new()
$rectangleResult = [ShuTrayProbe]::TrayRect($ApplicationPid, [ref]$nativeBounds)
if ($rectangleResult -ne 0) { throw "Shell_NotifyIconGetRect failed for application tray: $rectangleResult" }
$bounds = [Windows.Rect]::new($nativeBounds.Left, $nativeBounds.Top, $nativeBounds.Right - $nativeBounds.Left, $nativeBounds.Bottom - $nativeBounds.Top)
if ($IconScreenshot) {
    $bitmap = [Drawing.Bitmap]::new([int]$bounds.Width, [int]$bounds.Height)
    $graphics = [Drawing.Graphics]::FromImage($bitmap)
    try {
        $graphics.CopyFromScreen([int]$bounds.X, [int]$bounds.Y, 0, 0, $bitmap.Size)
        $bitmap.Save($IconScreenshot, [Drawing.Imaging.ImageFormat]::Png)
    } finally { $graphics.Dispose(); $bitmap.Dispose() }
}
if (-not [ShuTrayProbe]::SetCursorPos([int]($bounds.X + $bounds.Width / 2), [int]($bounds.Y + $bounds.Height / 2))) {
    throw 'Could not position the pointer over the tray icon'
}
Start-Sleep -Milliseconds 200
[ShuTrayProbe]::mouse_event(0x0008, 0, 0, 0, [UIntPtr]::Zero)
Start-Sleep -Milliseconds 75
[ShuTrayProbe]::mouse_event(0x0010, 0, 0, 0, [UIntPtr]::Zero)
$deadline = [DateTime]::UtcNow.AddSeconds(5)
do {
    Start-Sleep -Milliseconds 100
    $popup = [ShuTrayProbe]::Popup($ApplicationPid)
} until ($popup -ne [IntPtr]::Zero -or [DateTime]::UtcNow -ge $deadline)
if ($popup -eq [IntPtr]::Zero) {
    if ($IconScreenshot) {
        $bitmap = [Drawing.Bitmap]::new(600, 500)
        $graphics = [Drawing.Graphics]::FromImage($bitmap)
        try {
            $left = [Math]::Max(0, [Math]::Min([int]$bounds.X - 450, [ShuTrayProbe]::GetSystemMetrics(0) - 600))
            $top = [Math]::Max(0, [Math]::Min([int]$bounds.Y - 400, [ShuTrayProbe]::GetSystemMetrics(1) - 500))
            $graphics.CopyFromScreen($left, $top, 0, 0, $bitmap.Size)
            $bitmap.Save($IconScreenshot + '.failure.png', [Drawing.Imaging.ImageFormat]::Png)
        } finally { $graphics.Dispose(); $bitmap.Dispose() }
    }
    throw "The real native tray menu did not open for PID $ApplicationPid. Pointer: $([ShuTrayProbe]::PointerTarget()). Popups: $([ShuTrayProbe]::PopupDebug())"
}
$menu = [ShuTrayProbe]::SendMessage($popup, 0x01E1, [IntPtr]::Zero, [IntPtr]::Zero)
function Read-Menu([IntPtr]$Handle) {
    $items = @()
    for ($index = 0; $index -lt [ShuTrayProbe]::GetMenuItemCount($Handle); $index++) {
        $label = [Text.StringBuilder]::new(256)
        [ShuTrayProbe]::GetMenuString($Handle, $index, $label, 256, 0x400) | Out-Null
        $state = [ShuTrayProbe]::GetMenuState($Handle, $index, 0x400)
        $submenu = [ShuTrayProbe]::GetSubMenu($Handle, $index)
        $items += [ordered]@{
            index = $index
            text = $label.ToString()
            enabled = ($state -band 3) -eq 0
            checked = ($state -band 8) -ne 0
            children = @(if ($submenu -ne [IntPtr]::Zero) { Read-Menu $submenu })
        }
    }
    return $items
}
$items = @(Read-Menu $menu)
$labels = @{
    show = '打开 ShuVPN'; hide = '隐藏窗口'; account = '校园账户'
    settings = '设置与外观'; exit = '退出 ShuVPN'; http = 'HTTP 代理'
}
try {
    if ($Action -ne 'inspect') {
        if ($Action -eq 'http') {
            $parent = $items | Where-Object text -eq '下次连接配置'
            if (-not $parent) { throw 'The proxy submenu is missing' }
            $parentRect = [ShuTrayProbe+Rect]::new()
            if (-not [ShuTrayProbe]::GetMenuItemRect([IntPtr]::Zero, $menu, $parent.index, [ref]$parentRect)) { throw 'Could not locate proxy submenu' }
            [ShuTrayProbe]::SetCursorPos($parentRect.Left + 20, [int](($parentRect.Top + $parentRect.Bottom) / 2)) | Out-Null
            [ShuTrayProbe]::mouse_event(0x0002, 0, 0, 0, [UIntPtr]::Zero)
            [ShuTrayProbe]::mouse_event(0x0004, 0, 0, 0, [UIntPtr]::Zero)
            Start-Sleep -Milliseconds 250
            $menu = [ShuTrayProbe]::GetSubMenu($menu, $parent.index)
            $selected = $parent.children | Where-Object text -eq $labels[$Action]
        } else {
            $selected = $items | Where-Object text -eq $labels[$Action]
        }
        if (-not $selected -or -not $selected.enabled) { throw "Tray action is missing or disabled: $Action" }
        $rect = [ShuTrayProbe+Rect]::new()
        if (-not [ShuTrayProbe]::GetMenuItemRect([IntPtr]::Zero, $menu, $selected.index, [ref]$rect)) { throw "Could not locate menu item: $Action" }
        [ShuTrayProbe]::SetCursorPos($rect.Left + 20, [int](($rect.Top + $rect.Bottom) / 2)) | Out-Null
        [ShuTrayProbe]::mouse_event(0x0002, 0, 0, 0, [UIntPtr]::Zero)
        [ShuTrayProbe]::mouse_event(0x0004, 0, 0, 0, [UIntPtr]::Zero)
    }
} finally {
    # Dismiss any remaining menu without synthesizing an application command.
    [ShuTrayProbe]::SendMessage($popup, 0x0100, [IntPtr]27, [IntPtr]::Zero) | Out-Null
}
[ordered]@{ icon = $iconName; action = $Action; items = $items } | ConvertTo-Json -Depth 8
