# Turns Windows high contrast on or off for the tab pictures of the build checks (.github/workflows/build.yml,
# Dialog layout check), through SystemParametersInfo SPI_SETHIGHCONTRAST.
#
# Usage: powershell -NoProfile -File tools/ci/high-contrast.ps1 on|off
param([Parameter(Mandatory = $true)][ValidateSet("on", "off")][string]$State)
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class BrailleCraftContrast {
    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    public struct HIGHCONTRAST {
        public int cbSize;
        public int dwFlags;
        public string lpszDefaultScheme;
    }
    [DllImport("user32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    public static extern bool SystemParametersInfo(int action, int param, ref HIGHCONTRAST value, int winIni);
    public static bool Set(bool on) {
        HIGHCONTRAST hc = new HIGHCONTRAST();
        hc.cbSize = Marshal.SizeOf(typeof(HIGHCONTRAST));
        hc.dwFlags = on ? 1 : 0;
        hc.lpszDefaultScheme = on ? "High Contrast Black" : null;
        // SPI_SETHIGHCONTRAST, with SPIF_SENDWININICHANGE so running programs repaint.
        return SystemParametersInfo(0x0043, hc.cbSize, ref hc, 2);
    }
}
"@
$ok = [BrailleCraftContrast]::Set($State -eq "on")
Write-Output "high contrast $State`: $ok"
