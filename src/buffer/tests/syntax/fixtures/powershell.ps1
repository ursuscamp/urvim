# PowerShell syntax fixture
function Invoke-Greet {
    param($Name)
    $count = 42
    $text = @"
hello
"@
    if ($true) {
        Write-Output $Name
    }
}
