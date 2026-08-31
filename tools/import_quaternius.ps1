<#
.SYNOPSIS
    Imports (or verifies) the official Quaternius Universal Animation Library humanoid.

.DESCRIPTION
    Import mode expands the official free "Universal Animation Library[Standard].zip" archive
    into this repository's ignored '.asset-import\quaternius' staging directory, stages a
    complete replacement for 'assets\characters\quaternius', validates every staged file, and
    only then swaps the staged directory into place. If any step fails the previous directory
    is restored, so a new GLB or license can never end up paired with a stale 'asset.lock.ron'.

    Verify mode ('-VerifyOnly') re-validates whatever is currently imported -- the locked
    'gltf_path', SHA-256, byte size, GLB container, and license -- and writes nothing.

    The archive is downloaded from the official page:
        https://quaternius.itch.io/universal-animation-library

    Contract (v3.0 Standard pack, uploaded 16 June 2026):
      * archive SHA-256      cc73fc4e495b82958207316596317a3f40b9fa38065bde1027937452da537724
      * archive member       ...\Unreal-Godot\UAL1_Standard.glb   (exactly one match required)
      * archive root license exactly one 'License[.txt|.md]' beside the archive's README
      * destination          assets\characters\quaternius\UAL1_Standard.glb
      * asset path in RON    characters/quaternius/UAL1_Standard.glb

    Every selection is exact and deterministic: zero matches and two-or-more matches are both
    hard errors. There is no fallback to legacy pack layouts or file names; a pack that does
    not satisfy the contract must be qualified by hand and the contract updated deliberately.

    The '_RM' variant bakes root motion into every clip and is deliberately NOT imported: this
    prototype requires in-place locomotion. It does not match the contract path.

    Compatible with Windows PowerShell 5.1 and PowerShell 7+.

.PARAMETER ArchivePath
    Path to the downloaded 'Universal Animation Library[Standard].zip'.

.PARAMETER ExpectedArchiveSha256
    Lowercase SHA-256 the archive must have. Defaults to the pinned v3.0 Standard archive.
    Override this ONLY for an intentional, reviewed pack upgrade; the new value must be
    recorded in docs\validation\humanoid-import.md and in this script's default.

.PARAMETER VerifyOnly
    Validate the currently imported asset without touching any file.

.EXAMPLE
    .\tools\import_quaternius.ps1 -ArchivePath "$env:USERPROFILE\Downloads\Universal Animation Library[Standard].zip"

.EXAMPLE
    .\tools\import_quaternius.ps1 -VerifyOnly
#>
[CmdletBinding(DefaultParameterSetName = 'Import')]
param(
    [Parameter(Mandatory = $true, ParameterSetName = 'Import')]
    [ValidateNotNullOrEmpty()]
    [string]$ArchivePath,

    [Parameter(ParameterSetName = 'Import')]
    [ValidatePattern('^[0-9a-fA-F]{64}$')]
    [string]$ExpectedArchiveSha256 = 'cc73fc4e495b82958207316596317a3f40b9fa38065bde1027937452da537724',

    [Parameter(Mandatory = $true, ParameterSetName = 'Verify')]
    [switch]$VerifyOnly
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

# ---------------------------------------------------------------------------------------------
# Contract constants. Changing any of these is a deliberate re-qualification of the asset.
# ---------------------------------------------------------------------------------------------

$ContractArchiveMemberSuffix = 'Unreal-Godot\UAL1_Standard.glb'
$ContractModelFileName       = 'UAL1_Standard.glb'
$ContractLicenseFileName     = 'LICENSE.txt'
$ContractCharacterFileName   = 'character.ron'
$ContractLockFileName        = 'asset.lock.ron'
$ContractDestinationRelative = 'assets\characters\quaternius'
$ContractGltfPath            = 'characters/quaternius/UAL1_Standard.glb'
$ContractLicensePath         = 'characters/quaternius/LICENSE.txt'
$ContractLicenseNamePattern  = '^licen[sc]e(\.(txt|md))?$'

$StagingRelative      = '.asset-import\quaternius'
$StagedDestRelative   = '.asset-import\staged-destination'
$PreviousDestRelative = '.asset-import\previous-destination'

$repo = Split-Path -Parent $PSScriptRoot

# ---------------------------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------------------------

function Get-FullPath {
    param([Parameter(Mandatory = $true)][string]$Path)
    return [System.IO.Path]::GetFullPath($Path)
}

function Write-Utf8NoBomFile {
    # Set-Content -Encoding utf8NoBOM does not exist in Windows PowerShell 5.1, and its 'utf8'
    # writes a BOM there. Writing through .NET keeps both hosts byte-identical.
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][AllowEmptyString()][string]$Text
    )
    $encoding = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText((Get-FullPath $Path), $Text, $encoding)
}

function Get-Sha256Lower {
    param([Parameter(Mandatory = $true)][string]$Path)
    return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

function Remove-OwnedDirectory {
    # Only ever removes one of the exact staging directories this script owns. Never the
    # repository root, the destination, or a caller-supplied path.
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$ExpectedPath
    )
    $resolved = Get-FullPath $Path
    if ($resolved -ne (Get-FullPath $ExpectedPath)) {
        throw "Refusing to clean unexpected path '$resolved'; expected '$ExpectedPath'."
    }
    if (Test-Path -LiteralPath $resolved) {
        if (-not (Test-Path -LiteralPath $resolved -PathType Container)) {
            throw "Path exists but is not a directory: $resolved"
        }
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
    return $resolved
}

function Get-RonStringField {
    param(
        [Parameter(Mandatory = $true)][string]$Text,
        [Parameter(Mandatory = $true)][string]$Key,
        [Parameter(Mandatory = $true)][string]$Source
    )
    $pattern = '(?m)^\s*' + [regex]::Escape($Key) + '\s*:\s*"([^"]*)"\s*,?\s*$'
    $match = [regex]::Match($Text, $pattern)
    if (-not $match.Success) {
        throw "Field '$Key' (string) is missing or malformed in $Source"
    }
    return $match.Groups[1].Value
}

function Get-RonIntegerField {
    param(
        [Parameter(Mandatory = $true)][string]$Text,
        [Parameter(Mandatory = $true)][string]$Key,
        [Parameter(Mandatory = $true)][string]$Source
    )
    $pattern = '(?m)^\s*' + [regex]::Escape($Key) + '\s*:\s*([0-9]+)\s*,?\s*$'
    $match = [regex]::Match($Text, $pattern)
    if (-not $match.Success) {
        throw "Field '$Key' (integer) is missing or malformed in $Source"
    }
    return [int64]$match.Groups[1].Value
}

function Read-TextFile {
    param([Parameter(Mandatory = $true)][string]$Path)
    return [System.IO.File]::ReadAllText((Get-FullPath $Path))
}

function Test-GlbContainer {
    # Cheap structural sanity check: 12-byte glTF-binary header, version 2, declared length
    # equal to the real file length. Catches truncated or wrong-format copies.
    param([Parameter(Mandatory = $true)][string]$Path)

    $file = Get-Item -LiteralPath $Path
    if ($file.Length -lt 12) {
        throw "Not a glTF-binary file (shorter than a GLB header): $Path"
    }

    $stream = [System.IO.File]::OpenRead((Get-FullPath $Path))
    try {
        $header = New-Object byte[] 12
        $read = $stream.Read($header, 0, 12)
        if ($read -ne 12) {
            throw "Could not read the GLB header: $Path"
        }
    }
    finally {
        $stream.Dispose()
    }

    $magic = [System.Text.Encoding]::ASCII.GetString($header, 0, 4)
    if ($magic -ne 'glTF') {
        throw "Not a glTF-binary file (magic '$magic', expected 'glTF'): $Path"
    }
    $version = [System.BitConverter]::ToUInt32($header, 4)
    if ($version -ne 2) {
        throw "Unsupported GLB container version $version (expected 2): $Path"
    }
    $declared = [System.BitConverter]::ToUInt32($header, 8)
    if ($declared -ne $file.Length) {
        throw "GLB declares $declared bytes but the file is $($file.Length) bytes: $Path"
    }
}

function Test-ImportedDirectory {
    <#
        Validates one complete, self-contained import directory: the model, the license, the
        hand-written character.ron contract, and the generated asset.lock.ron must all agree
        with each other and with the contract constants. Used both on the staged directory
        (before the swap) and on the live destination (after the swap, and for -VerifyOnly).
        Read-only.
    #>
    param(
        [Parameter(Mandatory = $true)][string]$Directory,
        [Parameter(Mandatory = $true)][string]$Label
    )

    $dir = Get-FullPath $Directory
    if (-not (Test-Path -LiteralPath $dir -PathType Container)) {
        throw "$Label directory not found: $dir"
    }

    $lockPath      = Join-Path $dir $ContractLockFileName
    $characterPath = Join-Path $dir $ContractCharacterFileName
    $modelPath     = Join-Path $dir $ContractModelFileName
    $licensePath   = Join-Path $dir $ContractLicenseFileName

    foreach ($required in @($lockPath, $characterPath, $modelPath, $licensePath)) {
        if (-not (Test-Path -LiteralPath $required -PathType Leaf)) {
            throw "$Label is incomplete, missing file: $required"
        }
    }

    $lockText      = Read-TextFile $lockPath
    $characterText = Read-TextFile $characterPath

    $lockGltfPath  = Get-RonStringField  -Text $lockText      -Key 'gltf_path'    -Source $lockPath
    $lockSha256    = Get-RonStringField  -Text $lockText      -Key 'sha256'       -Source $lockPath
    $lockSize      = Get-RonIntegerField -Text $lockText      -Key 'byte_size'    -Source $lockPath
    $charGltfPath  = Get-RonStringField  -Text $characterText -Key 'gltf_path'    -Source $characterPath
    $charLicense   = Get-RonStringField  -Text $characterText -Key 'license_path' -Source $characterPath

    if ($lockGltfPath -cne $ContractGltfPath) {
        throw "$Label lock binds gltf_path '$lockGltfPath' but the contract is '$ContractGltfPath': $lockPath"
    }
    if ($charGltfPath -cne $ContractGltfPath) {
        throw "$Label character.ron declares gltf_path '$charGltfPath' but the contract is '$ContractGltfPath': $characterPath"
    }
    if ($charLicense -cne $ContractLicensePath) {
        throw "$Label character.ron declares license_path '$charLicense' but the contract is '$ContractLicensePath': $characterPath"
    }
    if ($lockSha256 -notmatch '^[0-9a-f]{64}$') {
        throw "$Label lock sha256 '$lockSha256' is not a lowercase 64-character SHA-256: $lockPath"
    }

    Test-GlbContainer -Path $modelPath

    $actualSize = (Get-Item -LiteralPath $modelPath).Length
    if ($actualSize -le 0) {
        throw "$Label model is empty: $modelPath"
    }
    if ($actualSize -ne $lockSize) {
        throw "$Label model is $actualSize bytes but the lock says $lockSize bytes: $modelPath"
    }

    $actualHash = Get-Sha256Lower -Path $modelPath
    if ($actualHash -cne $lockSha256) {
        throw "$Label model SHA-256 is $actualHash but the lock says $lockSha256 : $modelPath"
    }

    if ((Get-Item -LiteralPath $licensePath).Length -le 0) {
        throw "$Label license file is empty: $licensePath"
    }

    return [pscustomobject]@{
        Directory   = $dir
        ModelPath   = $modelPath
        LicensePath = $licensePath
        LockPath    = $lockPath
        GltfPath    = $lockGltfPath
        Sha256      = $actualHash
        ByteSize    = $actualSize
    }
}

# ---------------------------------------------------------------------------------------------
# Verify-only mode
# ---------------------------------------------------------------------------------------------

$destination = Get-FullPath (Join-Path $repo $ContractDestinationRelative)

if ($PSCmdlet.ParameterSetName -eq 'Verify') {
    $verified = Test-ImportedDirectory -Directory $destination -Label 'Imported asset'
    Write-Output "Verified: $($verified.Directory)"
    Write-Output "Model:    $($verified.ModelPath)"
    Write-Output "License:  $($verified.LicensePath)"
    Write-Output "Lock:     $($verified.LockPath)"
    Write-Output "AssetPath:$($verified.GltfPath)"
    Write-Output "SHA-256:  $($verified.Sha256)"
    Write-Output "Bytes:    $($verified.ByteSize)"
    Write-Output 'Result:   OK'
    return
}

# ---------------------------------------------------------------------------------------------
# Import: qualify the archive
# ---------------------------------------------------------------------------------------------

if (-not (Test-Path -LiteralPath $ArchivePath -PathType Leaf)) {
    throw "Archive not found: $ArchivePath"
}

$archive = Get-Item -LiteralPath $ArchivePath
if ($archive.Extension -ne '.zip') {
    throw "Archive must be a .zip file, got '$($archive.Extension)': $($archive.FullName)"
}

$expectedArchiveHash = $ExpectedArchiveSha256.ToLowerInvariant()
$actualArchiveHash = Get-Sha256Lower -Path $archive.FullName
if ($actualArchiveHash -cne $expectedArchiveHash) {
    throw ("Archive SHA-256 mismatch, refusing to extract.`n" +
        "  archive:  $($archive.FullName)`n" +
        "  expected: $expectedArchiveHash`n" +
        "  actual:   $actualArchiveHash`n" +
        'If this is an intentional pack upgrade, re-qualify the pack by hand and re-run with ' +
        '-ExpectedArchiveSha256 <hash>, then record the new hash in the script default and in ' +
        'docs\validation\humanoid-import.md.')
}

# The live character.ron is the hand-written contract and is never regenerated; it is carried
# into the staged directory so the swap replaces a complete, self-consistent asset directory.
$liveCharacter = Join-Path $destination $ContractCharacterFileName
if (-not (Test-Path -LiteralPath $liveCharacter -PathType Leaf)) {
    throw ("The hand-written contract '$ContractCharacterFileName' was not found at " +
        "'$liveCharacter'. This script never generates it; restore it from version control first.")
}

# ---------------------------------------------------------------------------------------------
# Import: extract
# ---------------------------------------------------------------------------------------------

$staging = Remove-OwnedDirectory -Path (Join-Path $repo $StagingRelative) -ExpectedPath (Join-Path $repo $StagingRelative)
New-Item -ItemType Directory -Force -Path $staging | Out-Null

# ExtractToDirectory treats both paths literally, so square brackets in the official archive
# name are not mistaken for wildcards, and it rejects entries that escape the destination.
if (-not ('System.IO.Compression.ZipFile' -as [type])) {
    Add-Type -AssemblyName System.IO.Compression.FileSystem
}
[System.IO.Compression.ZipFile]::ExtractToDirectory($archive.FullName, $staging)

$extracted = @(Get-ChildItem -LiteralPath $staging -Recurse -File)
if ($extracted.Count -eq 0) {
    throw "Archive expanded to no files: $($archive.FullName)"
}

$stagingPrefix = $staging.TrimEnd('\') + '\'
$entries = @($extracted | ForEach-Object {
        $full = $_.FullName
        [pscustomobject]@{
            FullName     = $full
            RelativePath = $full.Substring($stagingPrefix.Length)
            File         = $_
        }
    } | Sort-Object -Property RelativePath)

# --- model: exactly one archive member whose path ends with the contract suffix -----------------

$modelCandidates = @($entries | Where-Object {
        $_.RelativePath -eq $ContractArchiveMemberSuffix -or
        $_.RelativePath.EndsWith('\' + $ContractArchiveMemberSuffix, [System.StringComparison]::OrdinalIgnoreCase)
    })

if ($modelCandidates.Count -eq 0) {
    $listing = ($entries | ForEach-Object { $_.RelativePath }) -join ', '
    throw ("The contract model '$ContractArchiveMemberSuffix' was not found in the archive. " +
        "Archive contains: $listing")
}
if ($modelCandidates.Count -gt 1) {
    $listing = ($modelCandidates | ForEach-Object { $_.RelativePath }) -join ', '
    throw ("The contract model '$ContractArchiveMemberSuffix' is ambiguous: $($modelCandidates.Count) " +
        "matches found ($listing). Refusing to guess which one is official.")
}
$model = $modelCandidates[0]

# --- license: exactly one license file at the archive root --------------------------------------

# The archive root is the single top-level directory the pack unpacks into, or the staging
# directory itself if the pack has no wrapper directory.
$topLevel = @(Get-ChildItem -LiteralPath $staging -Force | Sort-Object -Property Name)
if ($topLevel.Count -eq 1 -and $topLevel[0].PSIsContainer) {
    $archiveRoot = Get-FullPath $topLevel[0].FullName
}
else {
    $archiveRoot = $staging
}

$licenseCandidates = @($entries |
    Where-Object {
        $_.File.Name -match $ContractLicenseNamePattern -and
        (Get-FullPath $_.File.DirectoryName) -eq $archiveRoot
    })

if ($licenseCandidates.Count -eq 0) {
    throw ("A license file was not found at the archive root '$archiveRoot': $($archive.FullName)")
}
if ($licenseCandidates.Count -gt 1) {
    $listing = ($licenseCandidates | ForEach-Object { $_.RelativePath }) -join ', '
    throw ("The archive root license is ambiguous: $($licenseCandidates.Count) candidates found " +
        "($listing). Refusing to guess which one is official.")
}
$license = $licenseCandidates[0]

# ---------------------------------------------------------------------------------------------
# Import: stage a complete replacement directory, validate it, then swap
# ---------------------------------------------------------------------------------------------

$stagedDest = Remove-OwnedDirectory -Path (Join-Path $repo $StagedDestRelative) -ExpectedPath (Join-Path $repo $StagedDestRelative)
New-Item -ItemType Directory -Force -Path $stagedDest | Out-Null

$stagedModel     = Join-Path $stagedDest $ContractModelFileName
$stagedLicense   = Join-Path $stagedDest $ContractLicenseFileName
$stagedCharacter = Join-Path $stagedDest $ContractCharacterFileName
$stagedLock      = Join-Path $stagedDest $ContractLockFileName

Copy-Item -LiteralPath $model.FullName -Destination $stagedModel -Force
Copy-Item -LiteralPath $license.FullName -Destination $stagedLicense -Force
Copy-Item -LiteralPath $liveCharacter -Destination $stagedCharacter -Force

$hash = Get-Sha256Lower -Path $stagedModel
$size = (Get-Item -LiteralPath $stagedModel).Length

$lockLines = @(
    '(',
    "    gltf_path: `"$ContractGltfPath`",",
    "    sha256: `"$hash`",",
    "    byte_size: $size,",
    ')'
)
Write-Utf8NoBomFile -Path $stagedLock -Text (($lockLines -join [System.Environment]::NewLine) + [System.Environment]::NewLine)

$staged = Test-ImportedDirectory -Directory $stagedDest -Label 'Staged import'

$previous = Remove-OwnedDirectory -Path (Join-Path $repo $PreviousDestRelative) -ExpectedPath (Join-Path $repo $PreviousDestRelative)

# Swap. Between the two moves the destination does not exist; every failure path restores the
# previous directory, so a new model or license can never be left paired with a stale lock.
Move-Item -LiteralPath $destination -Destination $previous
try {
    Move-Item -LiteralPath $stagedDest -Destination $destination
    $imported = Test-ImportedDirectory -Directory $destination -Label 'Imported asset'
}
catch {
    if (Test-Path -LiteralPath $destination) {
        Remove-OwnedDirectory -Path $stagedDest -ExpectedPath (Join-Path $repo $StagedDestRelative) | Out-Null
        Move-Item -LiteralPath $destination -Destination $stagedDest
    }
    Move-Item -LiteralPath $previous -Destination $destination
    throw ("Import failed and was rolled back; '$destination' is unchanged. Rejected files are " +
        "in '$stagedDest'. Cause: $($_.Exception.Message)")
}

Remove-OwnedDirectory -Path $previous -ExpectedPath (Join-Path $repo $PreviousDestRelative) | Out-Null

Write-Output "Archive:  $($archive.FullName)"
Write-Output "ArcHash:  $actualArchiveHash"
Write-Output "Model:    $($imported.ModelPath) (from $($model.RelativePath))"
Write-Output "License:  $($imported.LicensePath) (from $($license.RelativePath))"
Write-Output "Lock:     $($imported.LockPath)"
Write-Output "AssetPath:$($imported.GltfPath)"
Write-Output "SHA-256:  $($imported.Sha256)"
Write-Output "Bytes:    $($imported.ByteSize)"
Write-Output 'Result:   OK'
