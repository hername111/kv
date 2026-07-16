param(
    [Parameter(Position = 0)]
    [string]$DbPath
)

$ErrorActionPreference = "Stop"
$PageSize = 4096

if ([string]::IsNullOrWhiteSpace($DbPath)) {
    $dataDir = if ([string]::IsNullOrWhiteSpace($env:KV_DATA_DIR)) {
        "kv_data"
    } else {
        $env:KV_DATA_DIR
    }
    $DbPath = Join-Path $dataDir "kv.db"
}

if (-not (Test-Path -LiteralPath $DbPath -PathType Leaf)) {
    Write-Error "Database file not found: $DbPath"
    Write-Host "Start kv-server first, or pass the path explicitly:"
    Write-Host "  powershell -NoProfile -ExecutionPolicy Bypass -File scripts/show-kv-db.ps1 -DbPath path/to/kv.db"
    exit 1
}

$resolvedPath = (Resolve-Path -LiteralPath $DbPath).Path
$stream = [System.IO.File]::Open(
    $resolvedPath,
    [System.IO.FileMode]::Open,
    [System.IO.FileAccess]::Read,
    [System.IO.FileShare]::ReadWrite
)

try {
    if ($stream.Length -lt 36) {
        throw "Database file is too short to contain a superblock: $($stream.Length) bytes"
    }

    $header = New-Object byte[] 36
    $bytesRead = $stream.Read($header, 0, $header.Length)
    if ($bytesRead -ne $header.Length) {
        throw "Could not read the complete superblock"
    }

    $fileBytes = $stream.Length
    $pageCount = [math]::Floor($fileBytes / $PageSize)
    $remainder = $fileBytes % $PageSize
    $nextPage = [BitConverter]::ToUInt64($header, 0)
    $freeHead = [BitConverter]::ToUInt64($header, 8)
    $catalogRoot = [BitConverter]::ToUInt64($header, 16)
    $magic = [Text.Encoding]::ASCII.GetString($header, 24, 8).Trim([char]0)
    $formatVersion = [BitConverter]::ToUInt32($header, 32)
} finally {
    $stream.Dispose()
}

Write-Host "KV Database file inspection (read-only)"
Write-Host "--------------------------------------"
Write-Host ("path:             {0}" -f $resolvedPath)
Write-Host ("file size:        {0} bytes" -f $fileBytes)
Write-Host ("page size:        {0} bytes" -f $PageSize)
Write-Host ("page count:       {0}" -f $pageCount)
if ([string]::IsNullOrEmpty($magic) -and $formatVersion -eq 0) {
    $magicDisplay = "<legacy empty>"
} else {
    $magicDisplay = $magic
}
Write-Host ("superblock magic: {0}" -f $magicDisplay)
Write-Host ("format version:   {0}" -f $formatVersion)
Write-Host ("next page id:     {0}" -f $nextPage)
Write-Host ("free-list head:   {0}" -f $freeHead)
Write-Host ("catalog root:     {0}" -f $catalogRoot)

if ($remainder -ne 0) {
    Write-Warning "File size is not page-aligned (remainder $remainder bytes)"
}
if ([string]::IsNullOrEmpty($magic) -and $formatVersion -eq 0) {
    Write-Warning "Legacy superblock detected; kv-server will upgrade it on the next superblock write"
} elseif ($magic -ne "KVDBPAGE") {
    Write-Warning "Expected magic KVDBPAGE"
}
if ($nextPage -gt $pageCount) {
    Write-Warning "Next page id exceeds physical page count"
}
