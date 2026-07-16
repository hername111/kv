$ErrorActionPreference = "Continue"
$env:NO_COLOR = "1"
$rootDir = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$logDir = Join-Path ([IO.Path]::GetTempPath()) ("kv-check-" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $logDir | Out-Null

$passed = 0
$failed = 0

function Invoke-CheckStep {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][string]$Command,
        [string[]]$Arguments = @(),
        [string]$WorkingDirectory = $rootDir
    )

    $safeName = $Name -replace '[^A-Za-z0-9_.-]', '_'
    $logFile = Join-Path $logDir ("{0}_{1}_{2}.log" -f $script:passed, $script:failed, $safeName)

    Write-Host ""
    Write-Host "[$Name]"
    Write-Host ("  {0} {1}" -f $Command, ($Arguments -join " "))

    $stdoutLog = "$logFile.stdout"
    $stderrLog = "$logFile.stderr"
    $status = 1
    try {
        $resolvedCommand = Get-Command $Command -ErrorAction Stop
        if ($resolvedCommand.CommandType -eq [System.Management.Automation.CommandTypes]::ExternalScript) {
            $processFile = (Get-Process -Id $PID).Path
            $processArguments = @(
                "-NoProfile",
                "-ExecutionPolicy", "Bypass",
                "-File", ('"{0}"' -f $resolvedCommand.Source)
            ) + $Arguments
        } else {
            $processFile = $resolvedCommand.Source
            $processArguments = $Arguments
        }

        $process = Start-Process `
            -FilePath $processFile `
            -ArgumentList $processArguments `
            -WorkingDirectory $WorkingDirectory `
            -NoNewWindow `
            -Wait `
            -PassThru `
            -RedirectStandardOutput $stdoutLog `
            -RedirectStandardError $stderrLog
        $status = $process.ExitCode

        $output = @()
        if (Test-Path -LiteralPath $stdoutLog) {
            $output += Get-Content -LiteralPath $stdoutLog -Encoding utf8
        }
        if (Test-Path -LiteralPath $stderrLog) {
            $output += Get-Content -LiteralPath $stderrLog -Encoding utf8
        }
        $output | Set-Content -Encoding utf8 $logFile
    } catch {
        $_ | Out-String | Set-Content -Encoding utf8 $logFile
        $status = 1
    } finally {
        Remove-Item -LiteralPath $stdoutLog, $stderrLog -Force -ErrorAction SilentlyContinue
    }

    if ($status -eq 0) {
        $script:passed++
        Write-Host "  PASS"
    } else {
        $script:failed++
        Write-Host "  FAIL (exit $status)"
    }

    if ((Test-Path -LiteralPath $logFile) -and (Get-Item -LiteralPath $logFile).Length -gt 0) {
        Get-Content -LiteralPath $logFile -Encoding utf8 -Tail 8 | ForEach-Object { Write-Host "  $_" }
    } else {
        Write-Host "  (no output)"
    }
}

try {
    Write-Host "KV Database - verification summary"
    Write-Host "=================================="
    Write-Host "root: $rootDir"
    Write-Host "logs: $logDir (removed on exit)"
    Write-Host "Run this script before starting a demo server on port 3307."

    Invoke-CheckStep -Name "Rust format" -Command "cargo" -Arguments @("fmt", "--check", "--all")
    Invoke-CheckStep -Name "Rust clippy" -Command "cargo" -Arguments @("clippy", "--workspace", "--all-targets", "--", "-D", "warnings")
    Invoke-CheckStep -Name "Rust tests" -Command "cargo" -Arguments @("test", "--workspace", "--all-targets")
    Invoke-CheckStep -Name "Rust docs" -Command "cargo" -Arguments @("doc", "--workspace", "--no-deps")
    Invoke-CheckStep -Name "Frontend build" -Command "npm" -Arguments @("run", "build") -WorkingDirectory (Join-Path $rootDir "demo-client")
    Invoke-CheckStep -Name "Protocol and persistence tests" -Command "python" -Arguments @("test_protocol.py")
    Invoke-CheckStep -Name "Diff whitespace" -Command "git" -Arguments @("diff", "--check")

    Write-Host ""
    Write-Host "=================================="
    Write-Host "passed: $passed"
    Write-Host "failed: $failed"

    if ($failed -eq 0) {
        Write-Host "RESULT: ALL CHECKS PASSED"
        exit 0
    }

    Write-Host "RESULT: CHECKS FAILED (see the failed step output above)"
    exit 1
} finally {
    if (Test-Path -LiteralPath $logDir) {
        Remove-Item -LiteralPath $logDir -Recurse -Force
    }
}
