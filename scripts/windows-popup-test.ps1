# Windows popup test (CI): checks the reminder popup never takes keyboard
# focus, that Drank and Snooze work, and grabs screenshots (incl. the tray
# badge, issue #11). Needs a debug build, which honours HYDRATION_INTERVAL_SECS.
param(
    [string]$Exe = "src-tauri/target/debug/hydration-reminder.exe",
    [string]$Out = "artifacts",
    [int]$IntervalSecs = 8
)
$ErrorActionPreference = "Stop"
New-Item -ItemType Directory -Force $Out | Out-Null
Add-Type -AssemblyName System.Drawing, System.Windows.Forms
Add-Type @"
using System; using System.Text; using System.Runtime.InteropServices;
public static class W {
  public struct RECT { public int Left, Top, Right, Bottom; }
  delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc f, IntPtr l);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  public struct POINT { public int X, Y; }
  [DllImport("user32.dll")] static extern bool GetClientRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] static extern bool ClientToScreen(IntPtr h, ref POINT p);
  // The visible content in screen coordinates (excludes the invisible resize border).
  public static RECT ContentRect(IntPtr h) {
    RECT c; GetClientRect(h, out c); var p = new POINT();
    ClientToScreen(h, ref p);
    return new RECT { Left = p.X, Top = p.Y, Right = p.X + c.Right, Bottom = p.Y + c.Bottom };
  }
  [DllImport("user32.dll")] public static extern int GetWindowLong(IntPtr h, int i);
  [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindowEx(IntPtr p, IntPtr a, string c, string t);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern IntPtr SendMessage(IntPtr h, uint m, IntPtr w, StringBuilder l);
  [DllImport("user32.dll")] static extern IntPtr SendMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, int x, int y, uint d, UIntPtr e);
  [DllImport("user32.dll")] public static extern void keybd_event(byte k, byte s, uint f, UIntPtr e);

  public static string Title(IntPtr h) { var s = new StringBuilder(256); GetWindowText(h, s, 256); return s.ToString(); }
  public static uint Pid(IntPtr h) { uint p; GetWindowThreadProcessId(h, out p); return p; }
  public static IntPtr FindTop(uint pid, string title) {
    IntPtr found = IntPtr.Zero;
    EnumWindows((h, l) => { if (Pid(h) == pid && Title(h) == title) { found = h; return false; } return true; }, IntPtr.Zero);
    return found;
  }
  public static string EditText(IntPtr edit) {
    int n = (int)SendMessage(edit, 0x000E, IntPtr.Zero, IntPtr.Zero);
    var s = new StringBuilder(n + 1); SendMessage(edit, 0x000D, (IntPtr)(n + 1), s); return s.ToString();
  }
  public static void Click(int x, int y) {
    SetCursorPos(x, y); System.Threading.Thread.Sleep(150);
    mouse_event(0x2, 0, 0, 0, UIntPtr.Zero); mouse_event(0x4, 0, 0, 0, UIntPtr.Zero);
  }
  public static void Focus(IntPtr h) {
    keybd_event(0x12, 0, 0, UIntPtr.Zero); keybd_event(0x12, 0, 2, UIntPtr.Zero); // Alt unlocks SetForegroundWindow
    SetForegroundWindow(h);
  }
}
"@
[W]::SetProcessDPIAware() | Out-Null

$results = [System.Collections.Generic.List[object]]::new()
function Check([string]$name, [bool]$pass, [string]$detail = "") {
    $results.Add([pscustomobject]@{ Check = $name; Pass = $pass; Detail = $detail })
    Write-Host ("[{0}] {1} {2}" -f ($(if ($pass) { "PASS" } else { "FAIL" })), $name, $detail)
}
function Shot([string]$name, $crop = $null, [int]$zoom = 1) {
    $b = [System.Windows.Forms.SystemInformation]::VirtualScreen
    $bmp = New-Object System.Drawing.Bitmap $b.Width, $b.Height
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($b.Left, $b.Top, 0, 0, $bmp.Size)
    if ($crop) {
        $part = $bmp.Clone($crop, $bmp.PixelFormat)
        $big = New-Object System.Drawing.Bitmap ($crop.Width * $zoom), ($crop.Height * $zoom)
        $gb = [System.Drawing.Graphics]::FromImage($big)
        $gb.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::NearestNeighbor
        $gb.DrawImage($part, 0, 0, $big.Width, $big.Height)
        $big.Save((Join-Path $Out "$name.png")); $gb.Dispose(); $big.Dispose(); $part.Dispose()
    } else {
        $bmp.Save((Join-Path $Out "$name.png"))
    }
    $g.Dispose(); $bmp.Dispose()
}
function Wait-Until([scriptblock]$cond, [int]$secs, [scriptblock]$each = {}) {
    $end = (Get-Date).AddSeconds($secs)
    while ((Get-Date) -lt $end) { & $each; if (& $cond) { return $true }; Start-Sleep -Milliseconds 300 }
    return $false
}
function Send-Text([string]$text) { [System.Windows.Forms.SendKeys]::SendWait($text) }
function FgName { $h = [W]::GetForegroundWindow(); "'{0}' (pid {1})" -f [W]::Title($h), [W]::Pid($h) }

# 1. Notepad is the app being typed in.
$np = Start-Process notepad -PassThru
$np.WaitForInputIdle() | Out-Null
$null = Wait-Until { $np.Refresh(); $np.MainWindowHandle -ne 0 } 10
$npWin = $np.MainWindowHandle
$edit = [W]::FindWindowEx($npWin, [IntPtr]::Zero, "Edit", $null)

# 2. The app, with a short interval. Fresh state, so the first reminder is one interval away.
$env:HYDRATION_INTERVAL_SECS = "$IntervalSecs"
$app = Start-Process $Exe -PassThru -RedirectStandardError (Join-Path $Out "app-stderr.log")
Start-Sleep -Seconds 3
[W]::Focus($npWin)
Start-Sleep -Milliseconds 500
Check "Notepad has focus before the reminder" ([W]::GetForegroundWindow() -eq $npWin) (FgName)
Send-Text "Typing before the reminder. "

$popup = [IntPtr]::Zero
$visible = Wait-Until {
    if ($popup -eq [IntPtr]::Zero) { $script:popup = [W]::FindTop([uint32]$app.Id, "Hydration Reminder") }
    $popup -ne [IntPtr]::Zero -and [W]::IsWindowVisible($popup)
} ($IntervalSecs + 30) { Send-Text "." }
Check "Popup appears" $visible
if (-not $visible) { Shot "fail-no-popup"; $app.Kill(); $np.Kill(); throw "popup never appeared" }

Start-Sleep -Milliseconds 800
Check "Focus stays in Notepad when the popup appears" ([W]::GetForegroundWindow() -eq $npWin) (FgName)
Send-Text " Still typing with the popup up. "
$ex = [W]::GetWindowLong($popup, -20)
Check "Popup is no-activate and topmost" ((($ex -band 0x08000000) -ne 0) -and (($ex -band 0x8) -ne 0)) ("exstyle 0x{0:X8}" -f $ex)
$r = [W]::ContentRect($popup)
$wa = [System.Windows.Forms.Screen]::PrimaryScreen.WorkingArea
$gapR = $wa.Right - $r.Right; $gapB = $wa.Bottom - $r.Bottom
$m = 12 * [W]::GetDpiForWindow($popup) / 96.0
Check "Popup sits bottom-right, 12 px above the taskbar and from the edge" ([math]::Abs($gapR - $m) -le 2 -and [math]::Abs($gapB - $m) -le 2) ("content {0},{1}-{2},{3}; gaps right {4}, bottom {5}; work area {6}" -f $r.Left, $r.Top, $r.Right, $r.Bottom, $gapR, $gapB, $wa)
Shot "1-popup-shown"
# Bottom-right corner of the screen: the taskbar tray, where the count badge is.
$bounds = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
$trayCrop = New-Object System.Drawing.Rectangle ($bounds.Right - 320), ($bounds.Bottom - 60), 320, 60
Shot "tray-badge-due-4x" $trayCrop 4

# Button centres from popup.css (border 1, padding 20/16; Drank ~74x36, Snooze ~124, gap 8), in DIPs,
# measured from the content rect.
$s = [W]::GetDpiForWindow($popup) / 96.0
$y = [int]($r.Bottom - 35 * $s)
$drankX = [int]($r.Right - 58 * $s)
$snoozeX = [int]($r.Right - 165 * $s)

# 3. Drank.
[W]::Click($drankX, $y)
$hid = Wait-Until { -not [W]::IsWindowVisible($popup) } 5
Check "Drank hides the popup" $hid
Start-Sleep -Milliseconds 500
Check "Focus is back in Notepad after Drank (#13)" ([W]::GetForegroundWindow() -eq $npWin) (FgName)
Send-Text "AFTER-DRANK-OK "
Start-Sleep -Milliseconds 300
if ($edit -ne [IntPtr]::Zero) {
    Check "Typing after Drank lands in Notepad" ([W]::EditText($edit).Contains("AFTER-DRANK-OK")) ""
}
Shot "2-after-drank"
Shot "tray-badge-count-1-4x" $trayCrop 4

# 4. Next reminder, then Snooze.
$again = Wait-Until { [W]::IsWindowVisible($popup) } ($IntervalSecs + 15) { Send-Text "." }
Check "Popup appears again one interval after Drank" $again
if ($again) {
    Start-Sleep -Milliseconds 800
    Check "Focus stays in Notepad on the second popup" ([W]::GetForegroundWindow() -eq $npWin) (FgName)
    [W]::Click($snoozeX, $y)
    $hid = Wait-Until { -not [W]::IsWindowVisible($popup) } 5
    Check "Snooze hides the popup" $hid
    Start-Sleep -Milliseconds 500
    Check "Focus is back in Notepad after Snooze" ([W]::GetForegroundWindow() -eq $npWin) (FgName)
    Send-Text "AFTER-SNOOZE-OK"
    Start-Sleep -Milliseconds 300
    if ($edit -ne [IntPtr]::Zero) {
        Check "Typing after Snooze lands in Notepad" ([W]::EditText($edit).Contains("AFTER-SNOOZE-OK")) ""
    }
    Shot "3-after-snooze"
}

Check "App still running" (-not $app.HasExited) ""
if ($edit -ne [IntPtr]::Zero) { [W]::EditText($edit) | Set-Content (Join-Path $Out "notepad-text.txt") }
if (-not $app.HasExited) { $app.Kill() }
if (-not $np.HasExited) { $np.Kill() }

# Summary
$md = @("## Windows popup test", "", "| Check | Result | Detail |", "|---|---|---|")
foreach ($x in $results) { $md += "| $($x.Check) | $(if ($x.Pass) { '✅' } else { '❌' }) | $($x.Detail) |" }
$md += "", "Screenshots (incl. a 4x crop of the tray badge for #11) are in the ``windows-popup-test`` artifact."
if ($env:GITHUB_STEP_SUMMARY) { $md -join "`n" | Add-Content $env:GITHUB_STEP_SUMMARY }
$md -join "`n" | Set-Content (Join-Path $Out "summary.md")
if ($results | Where-Object { -not $_.Pass }) { exit 1 }
