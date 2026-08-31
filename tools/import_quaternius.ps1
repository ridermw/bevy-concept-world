<#
.SYNOPSIS
    Imports the official Quaternius Universal Animation Library humanoid into this repository.

.DESCRIPTION
    Expands the official free "Universal Animation Library[Standard].zip" archive into this
    repository's ignored '.asset-import\quaternius' staging directory, copies the qualified GLB
    and the archive's own license file into 'assets\characters\quaternius', and regenerates
    'asset.lock.ron' with the lowercase SHA-256 and byte size of the imported GLB.

    The archive is downloaded from the official page:
        https://quaternius.itch.io/universal-animation-library

    Pack v3.0 (uploaded 16 June 2026) names the glTF-binary humanoid 'UAL1_Standard.glb'.
    Earlier packs used 'AnimationLibrary_Godot_Standard.glb'. Both names are searched, in that
    order of preference, so the script keeps working across pack revisions. Use -ModelName to
    pin one explicitly.

    The '_RM' variant bakes root motion into every clip and is deliberately NOT imported: this
    prototype requires in-place locomotion.

.PARAMETER ArchivePath
    Path to the downloaded 'Universal Animation Library[Standard].zip'.

.PARAMETER ModelName
    Candidate GLB file names to look for inside the archive, most preferred first.

.EXAMPLE
    .\tools\import_quaternius.ps1 -ArchivePath "$env:USERPROFILE\Downloads\Universal Animation Library[Standard].zip"
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidateNotNullOrEmpty()]
    [string]$ArchivePath,

    [Parameter()]
    [ValidateNotNullOrEmpty()]
    [string[]]$ModelName = @('AnimationLibrary_Godot_Standard.glb', 'UAL1_Standard.glb')
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repo = Split-Path -Parent $PSScriptRoot
$destination = Join-Path $repo 'assets\characters\quaternius'
$temp = Join-Path $repo '.asset-import\quaternius'

if (-not (Test-Path -LiteralPath $ArchivePath -PathType Leaf)) {
    throw "Archive not found: $ArchivePath"
}

$archive = Get-Item -LiteralPath $ArchivePath
if ($archive.Extension -ne '.zip') {
    throw "Archive must be a .zip file, got '$($archive.Extension)': $($archive.FullName)"
}

# Only ever remove the one exact staging directory this script owns. Never recurse from the
# repository root, the destination, or a caller-supplied path.
$expectedTemp = [System.IO.Path]::GetFullPath((Join-Path $repo '.asset-import\quaternius'))
$resolvedTemp = [System.IO.Path]::GetFullPath($temp)
if ($resolvedTemp -ne $expectedTemp) {
    throw "Refusing to clean unexpected staging path '$resolvedTemp'; expected '$expectedTemp'."
}
if (Test-Path -LiteralPath $resolvedTemp) {
    if (-not (Test-Path -LiteralPath $resolvedTemp -PathType Container)) {
        throw "Staging path exists but is not a directory: $resolvedTemp"
    }
    Remove-Item -LiteralPath $resolvedTemp -Recurse -Force
}

New-Item -ItemType Directory -Force -Path $resolvedTemp, $destination | Out-Null

# ExtractToDirectory treats both paths literally, so square brackets in the official archive
# name are not mistaken for wildcards, and it rejects entries that escape the destination.
Add-Type -AssemblyName System.IO.Compression.FileSystem
[System.IO.Compression.ZipFile]::ExtractToDirectory($archive.FullName, $resolvedTemp)

$extracted = @(Get-ChildItem -LiteralPath $resolvedTemp -Recurse -File)
if ($extracted.Count -eq 0) {
    throw "Archive expanded to no files: $($archive.FullName)"
}

$model = $null
foreach ($candidate in $ModelName) {
    $model = $extracted | Where-Object { $_.Name -eq $candidate } | Select-Object -First 1
    if ($model) { break }
}

if (-not $model) {
    $found = ($extracted | ForEach-Object { $_.Name }) -join ', '
    throw ("None of the expected model files ({0}) were found in the archive. Archive contains: {1}" -f ($ModelName -join ', '), $found)
}

$license = $extracted |
    Where-Object { $_.Name -match '^licen[sc]e(\.(txt|md))?$' } |
    Select-Object -First 1

if (-not $license) {
    throw "A license file was not found in the archive: $($archive.FullName)"
}

$modelTarget = Join-Path $destination $model.Name
$licenseTarget = Join-Path $destination 'LICENSE.txt'
Copy-Item -LiteralPath $model.FullName -Destination $modelTarget -Force
Copy-Item -LiteralPath $license.FullName -Destination $licenseTarget -Force

$hash = (Get-FileHash -LiteralPath $modelTarget -Algorithm SHA256).Hash.ToLowerInvariant()
$size = (Get-Item -LiteralPath $modelTarget).Length

if ($size -le 0) {
    throw "Imported model is empty: $modelTarget"
}

$lockPath = Join-Path $destination 'asset.lock.ron'
@"
(
    sha256: "$hash",
    byte_size: $size,
)
"@ | Set-Content -LiteralPath $lockPath -Encoding utf8NoBOM

Write-Output "Archive:  $($archive.FullName)"
Write-Output "Imported: $modelTarget"
Write-Output "License:  $licenseTarget (from $($license.Name))"
Write-Output "Lock:     $lockPath"
Write-Output "SHA-256:  $hash"
Write-Output "Bytes:    $size"
