param(
  [int]$MaxDepth = 12,
  [string]$OutFile = "D:\Codigo\Dixi\.tmp-audit\uia-tree.txt"
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes

$root = [System.Windows.Automation.AutomationElement]::RootElement
$cond = New-Object System.Windows.Automation.PropertyCondition(
  [System.Windows.Automation.AutomationElement]::ClassNameProperty, 'Chrome_WidgetWin_1')
$wins = $root.FindAll([System.Windows.Automation.TreeScope]::Children, $cond)

$sb = New-Object System.Text.StringBuilder
function Esc([string]$s) {
  if ($null -eq $s) { return '' }
  return ($s -replace "`r?`n", ' | ' -replace "`t", ' ')
}

function Walk($el, $depth) {
  if ($depth -gt $MaxDepth) { return }
  try {
    $ct   = $el.Current.ControlType.ProgrammaticName -replace 'ControlType\.',''
    $name = Esc $el.Current.Name
    $aid  = $el.Current.AutomationId
    $cls  = $el.Current.ClassName
    $r    = $el.Current.BoundingRectangle
    $off  = $false
    try { $off = [System.Windows.Automation.AutomationElement]::IsOffscreen($el) } catch {}
    $line = ('{0}[{1}] name="{2}" aid="{3}" cls="{4}" rect=({5},{6},{7},{8}) off={9}' -f
      ('  ' * $depth), $ct, $name, $aid, $cls,
      [int]$r.X, [int]$r.Y, [int]$r.Width, [int]$r.Height, $off)
    [void]$sb.AppendLine($line)
  } catch {
    [void]$sb.AppendLine(('  ' * $depth) + "[ERR] " + $_.Exception.Message)
    return
  }
  $child = $null
  try { $child = [System.Windows.Automation.TreeWalker]::ControlViewWalker.GetFirstChild($el) } catch {}
  while ($null -ne $child) {
    Walk $child ($depth + 1)
    try { $child = [System.Windows.Automation.TreeWalker]::ControlViewWalker.GetNextSibling($child) } catch { break }
  }
}

[void]$sb.AppendLine("=== Chrome_WidgetWin_1 top-level windows: $($wins.Count) ===")
foreach ($w in $wins) {
  try {
    $procId = $w.Current.ProcessId
    $pname = ''
    try { $pname = (Get-Process -Id $procId -ErrorAction Stop).ProcessName } catch {}
    [void]$sb.AppendLine("---- WINDOW pid=$procId proc=$pname name='$(Esc $w.Current.Name)' class=$($w.Current.ClassName) rect=$($w.Current.BoundingRectangle) ----")
    if ($pname -like '*Wispr*' -or $pname -like '*wispr*') {
      Walk $w 1
    }
  } catch {
    [void]$sb.AppendLine("[WIN ERR] " + $_.Exception.Message)
  }
}

$text = $sb.ToString()
[System.IO.File]::WriteAllText($OutFile, $text, (New-Object System.Text.UTF8Encoding($false)))
Write-Output "WROTE $OutFile chars=$($text.Length)"
