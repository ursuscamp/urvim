# PowerShell syntax fixture
# Multi-line comment
<#
  Multi-line block comment
  in PowerShell
#>

function Invoke-Greet {
    param(
        [Parameter(Mandatory=$true)]
        [string]$Name,
        [int]$Count = 42
    )
    $count = 42
    $flag = $true
    $falseFlag = $false
    $nullVal = $null
    $floating = 3.14
    $hex_val = 0xFF
    $binary_val = 0b1010_0011

    $text = @"
hello
world
"@

    $here = @'
literal
here-string
'@

    if ($true) {
        Write-Output $Name
        Write-Host "count: $count"
    } elseif ($false) {
        Write-Warning "unreachable"
    } else {
        Write-Error "error"
    }

    foreach ($item in @(1, 2, 3)) {
        Write-Output $item
    }

    for ($i = 0; $i -lt 10; $i++) {
        Write-Output $i
    }

    while ($count -gt 0) {
        $count--
    }

    switch ($count) {
        42 { "answer" }
        default { "other" }
    }

    $items = @("one", "two", "three")
    $mapping = @{"name" = "Ada"; "age" = 42}
    $calc = (1 + 2) * 3

    try {
        Get-Item "C:\missing.txt" -ErrorAction Stop
    } catch {
        Write-Error $_.Exception.Message
    } finally {
        Cleanup-Resources
    }
}
