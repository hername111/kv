param(
    [Parameter(Position = 0)]
    [string]$DbPath,

    [string]$StateUrl = "http://127.0.0.1:8080/api/state",

    [switch]$NoWebState,

    [switch]$ResetVideoDemo,

    [string]$DemoDir = "target/video-demo"
)

$ErrorActionPreference = "Stop"
$PageSize = 4096

function Get-WorkspacePath {
    return (Resolve-Path -LiteralPath ".").Path
}

function Clear-VideoDemoData {
    param([string]$Directory)

    $workspace = Get-WorkspacePath
    $targetPath = Join-Path $workspace $Directory
    $parent = Split-Path -Parent $targetPath

    if (-not (Test-Path -LiteralPath $parent)) {
        New-Item -ItemType Directory -Path $parent -Force | Out-Null
    }

    $fullTarget = [System.IO.Path]::GetFullPath($targetPath)
    $allowedRoot = [System.IO.Path]::GetFullPath((Join-Path $workspace "target"))
    $allowedPrefix = $allowedRoot.TrimEnd([System.IO.Path]::DirectorySeparatorChar) + [System.IO.Path]::DirectorySeparatorChar

    if (-not $fullTarget.StartsWith($allowedPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing to delete target/ itself or a path outside target/: $fullTarget"
    }

    if (Test-Path -LiteralPath $fullTarget) {
        Remove-Item -LiteralPath $fullTarget -Recurse -Force
    }

    New-Item -ItemType Directory -Path $fullTarget -Force | Out-Null

    Write-Host "Clean video demo data directory:"
    Write-Host ("  {0}" -f $fullTarget)
    Write-Host "Start kv-server with:"
    Write-Host ('  $env:KV_DATA_DIR="{0}"' -f $Directory)
    Write-Host "  cargo run -p kv-server"
}

function Resolve-DatabasePath {
    param([string]$Path)

    if (-not [string]::IsNullOrWhiteSpace($Path)) {
        return $Path
    }

    $dataDir = if ([string]::IsNullOrWhiteSpace($env:KV_DATA_DIR)) {
        "kv_data"
    } else {
        $env:KV_DATA_DIR
    }
    return Join-Path $dataDir "kv.db"
}

function Show-DatabaseFile {
    param([string]$Path)

    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        Write-Error "Database file not found: $Path"
        Write-Host "Start kv-server first, or pass the path explicitly:"
        Write-Host "  powershell -NoProfile -ExecutionPolicy Bypass -File scripts/show-kv-db.ps1 -DbPath path/to/kv.db"
        exit 1
    }

    $resolvedPath = (Resolve-Path -LiteralPath $Path).Path
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

    Write-Host "KV database file inspection (read-only)"
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
}

function Format-Cell {
    param($Value)

    if ($null -eq $Value) {
        return "NULL"
    }
    return [string]$Value
}

function Show-WebState {
    param([string]$Url)

    Write-Host ""
    Write-Host "Web state from kv-server"
    Write-Host "------------------------"
    Write-Host ("url:              {0}" -f $Url)

    try {
        $state = Invoke-RestMethod -Method Get -Uri $Url -TimeoutSec 3
    } catch {
        Write-Warning "Could not read Web state. Start kv-server first if you want table data: $($_.Exception.Message)"
        return
    }

    $tables = @($state.tables)
    $totalRows = 0
    foreach ($table in $tables) {
        $totalRows += @($table.rows).Count
    }

    Write-Host ("table count:      {0}" -f $tables.Count)
    Write-Host ("row count:        {0}" -f $totalRows)

    if ($tables.Count -eq 0) {
        Write-Host "tables:           <empty>"
        return
    }

    foreach ($table in $tables) {
        $meta = $table.meta
        $columns = @($meta.columns)
        $rows = @($table.rows)
        $columnNames = $columns | ForEach-Object { $_.name }

        Write-Host ""
        Write-Host ("table:            {0}" -f $meta.tableName)
        Write-Host ("columns:          {0}" -f ($columnNames -join ", "))
        Write-Host ("primary key:      {0}" -f $columns[$meta.primaryKeyIndex].name)
        Write-Host ("indexes:          {0}" -f $meta.indexes)
        Write-Host ("rows:             {0}" -f $rows.Count)

        if ($rows.Count -eq 0) {
            continue
        }

        Write-Host "data:"
        Write-Host ("  {0}" -f ($columnNames -join " | "))
        foreach ($row in $rows) {
            $values = @($row) | ForEach-Object { Format-Cell $_ }
            Write-Host ("  {0}" -f ($values -join " | "))
        }
    }
}

if ($ResetVideoDemo) {
    Clear-VideoDemoData -Directory $DemoDir
    exit 0
}

$DbPath = Resolve-DatabasePath -Path $DbPath
Show-DatabaseFile -Path $DbPath

if (-not $NoWebState) {
    Show-WebState -Url $StateUrl
}
