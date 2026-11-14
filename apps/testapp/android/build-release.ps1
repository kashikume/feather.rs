<#
.SYNOPSIS
Builds the test app's release APK and signs it with your release key.

.DESCRIPTION
Runs `gradlew assembleRelease`, signs the unsigned APK with apksigner (from the SDK's newest
build-tools) and verifies it. With -Install it also installs it on the connected device.
See doc/android.md, "A release APK".

The keystore stays outside the repository: pass it with -Keystore, or set
FEATHER_RELEASE_KEYSTORE (and FEATHER_RELEASE_KEY_ALIAS if the alias isn't feather-testapp).
apksigner asks for the key's password, unless FEATHER_RELEASE_KEY_PASSWORD is set (it is then
read from that variable, not passed on the command line).

.EXAMPLE
.\build-release.ps1 -Keystore D:\keys\android-feather-rs-testapp.ks -Install
#>
param(
    # The keystore holding the release key (made with keytool, see doc/android.md).
    [string]$Keystore = $env:FEATHER_RELEASE_KEYSTORE,
    # The key's alias in the keystore.
    [string]$Alias = $(if ($env:FEATHER_RELEASE_KEY_ALIAS) { $env:FEATHER_RELEASE_KEY_ALIAS } else { 'feather-testapp' }),
    # Also install the signed APK on the connected device (adb install -r).
    [switch]$Install
)

# No $ErrorActionPreference = 'Stop': in Windows PowerShell it turns the progress that gradlew,
# cargo and apksigner write to stderr into errors when the output is redirected. Native commands
# are checked by their exit codes instead, and cmdlets get -ErrorAction Stop.

if (-not $Keystore) {
    throw 'No keystore: pass -Keystore <path> or set FEATHER_RELEASE_KEYSTORE.'
}
if (-not (Test-Path -LiteralPath $Keystore -PathType Leaf)) {
    throw "Keystore not found: $Keystore"
}
if (-not $env:JAVA_HOME) {
    throw 'JAVA_HOME is not set: point it at Android Studio''s jbr (see doc/android.md, Setup).'
}

# The Android SDK: ANDROID_HOME, else local.properties' sdk.dir, else the default location.
function Get-SdkDir {
    if ($env:ANDROID_HOME) { return $env:ANDROID_HOME }
    $props = Join-Path $PSScriptRoot 'local.properties'
    if (Test-Path $props) {
        $line = Get-Content $props -ErrorAction Stop | Where-Object { $_ -match '^sdk\.dir=' } | Select-Object -First 1
        if ($line) {
            # local.properties escapes ':' and '\' with a backslash.
            return ($line -replace '^sdk\.dir=', '') -replace '\\(.)', '$1'
        }
    }
    return Join-Path $env:LOCALAPPDATA 'Android\Sdk'
}

$sdk = Get-SdkDir
$buildTools = Get-ChildItem (Join-Path $sdk 'build-tools') -Directory -ErrorAction Stop |
    Sort-Object { [version]($_.Name -replace '[^0-9.].*$', '') } |
    Select-Object -Last 1
if (-not $buildTools) {
    throw "No build-tools in $sdk"
}
$apksigner = Join-Path $buildTools.FullName 'apksigner.bat'

$outDir = Join-Path $PSScriptRoot 'app\build\outputs\apk\release'
$unsigned = Join-Path $outDir 'app-release-unsigned.apk'
$signed = Join-Path $outDir 'app-release.apk'

Push-Location $PSScriptRoot
try {
    Write-Host '== Building the release APK'
    & .\gradlew.bat assembleRelease
    if ($LASTEXITCODE -ne 0) { throw "gradlew assembleRelease failed ($LASTEXITCODE)." }
    if (-not (Test-Path $unsigned)) { throw "Not found after the build: $unsigned" }

    Write-Host "== Signing with $Keystore ($Alias), build-tools $($buildTools.Name)"
    $signArgs = @('sign', '--ks', $Keystore, '--ks-key-alias', $Alias)
    if ($env:FEATHER_RELEASE_KEY_PASSWORD) {
        $signArgs += @('--ks-pass', 'env:FEATHER_RELEASE_KEY_PASSWORD')
    }
    $signArgs += @('--out', $signed, $unsigned)
    & $apksigner @signArgs
    if ($LASTEXITCODE -ne 0) { throw "apksigner sign failed ($LASTEXITCODE)." }

    & $apksigner verify --print-certs $signed
    if ($LASTEXITCODE -ne 0) { throw "apksigner verify failed ($LASTEXITCODE)." }

    if ($Install) {
        $adb = Join-Path $sdk 'platform-tools\adb.exe'
        if (-not (Test-Path $adb)) { $adb = 'adb' }
        Write-Host '== Installing'
        & $adb install -r $signed
        if ($LASTEXITCODE -ne 0) {
            throw ("adb install failed ($LASTEXITCODE). If the app on the device is signed with " +
                'another key (e.g. the debug build), uninstall it first: ' +
                'adb uninstall com.kashikume.feather.testapp')
        }
    }

    $size = [math]::Round((Get-Item $signed -ErrorAction Stop).Length / 1MB, 1)
    Write-Host "== Done: $signed ($size MB)"
}
finally {
    Pop-Location
}
