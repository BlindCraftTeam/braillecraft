# Writes the document tab list of the running BrailleCraft as UI Automation reads it (each tab list, then each of its
# tabs with its name and whether it is the selected one), for the tab pictures of the build checks
# (.github/workflows/build.yml, Dialog layout check).
#
# Usage: powershell -NoProfile -File tools/ci/tab-uia.ps1 <file.txt>
param([Parameter(Mandatory = $true)][string]$Path)
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
$A = [System.Windows.Automation.AutomationElement]
$Scope = [System.Windows.Automation.TreeScope]
$Type = [System.Windows.Automation.ControlType]
$lines = @()
$proc = Get-Process braillecraft-desktop -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $proc) {
    "the program is not running" | Set-Content -Path $Path -Encoding UTF8
    exit 0
}
$byProcess = New-Object System.Windows.Automation.PropertyCondition($A::ProcessIdProperty, $proc.Id)
$isTabList = New-Object System.Windows.Automation.PropertyCondition($A::ControlTypeProperty, $Type::Tab)
$isTab = New-Object System.Windows.Automation.PropertyCondition($A::ControlTypeProperty, $Type::TabItem)
foreach ($window in $A::RootElement.FindAll($Scope::Children, $byProcess)) {
    $lines += "window: " + $window.Current.Name
    foreach ($list in $window.FindAll($Scope::Descendants, $isTabList)) {
        $lines += "tab list: " + $list.Current.Name + " (" + $list.Current.ClassName + ")"
        foreach ($tab in $list.FindAll($Scope::Children, $isTab)) {
            $selected = "unknown"
            try {
                $pattern = $tab.GetCurrentPattern([System.Windows.Automation.SelectionItemPattern]::Pattern)
                $selected = $pattern.Current.IsSelected
            } catch {}
            $lines += "tab: " + $tab.Current.Name + " selected=" + $selected
        }
    }
}
$lines | Set-Content -Path $Path -Encoding UTF8
