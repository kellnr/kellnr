param(
    [Parameter(Mandatory=$false)]
    [ValidateSet("arm64", "amd64")]
    [string]$arch = ""
)

# Auto-detect architecture if not specified
# Note: PROCESSOR_ARCHITECTURE can be misleading under emulation
# Query the actual CPU architecture from the registry
if ($arch -eq "") {
    $cpuArch = (Get-ItemProperty "HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager\Environment").PROCESSOR_ARCHITECTURE
    if ($cpuArch -eq "AMD64") {
        $arch = "amd64"
    } elseif ($cpuArch -eq "ARM64") {
        $arch = "arm64"
    } else {
        Write-Host "Unsupported architecture: $cpuArch" -ForegroundColor Red
        exit 1
    }
}

if ($arch -eq "amd64") {
    $cargoTarget = "x86_64-pc-windows-msvc"
} elseif ($arch -eq "arm64") {
    $cargoTarget = "aarch64-pc-windows-msvc"
} else {
    Write-Host "Unsupported architecture: $arch" -ForegroundColor Red
    exit 1
}

Write-Host "Building for architecture: $arch" -ForegroundColor Cyan

# Setup Visual Studio Build Tools environment for Rust/Cargo compilation
# This script sets environment variables for the current PowerShell session only

# Determine architecture-specific paths based on detected arch
if ($arch -eq "amd64") {
    $hostArch = "Hostx64"
    $targetArch = "x64"
    $llvmArch = "x64"
    $sdkArch = "x64"
} elseif ($arch -eq "arm64") {
    $hostArch = "Hostarm64"
    $targetArch = "arm64"
    $llvmArch = "ARM64"
    $sdkArch = "arm64"
} else {
    Write-Host "Unsupported architecture: $arch" -ForegroundColor Red
    exit 1
}

# Base Visual Studio Build Tools path
$vsBasePath = "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools"

# Dynamically find the MSVC version
$msvcToolsPath = "$vsBasePath\VC\Tools\MSVC"
$msvcVersions = Get-ChildItem $msvcToolsPath -Directory | Sort-Object Name -Descending

if ($msvcVersions.Count -eq 0) {
    Write-Host "No MSVC version found in $msvcToolsPath" -ForegroundColor Red
    exit 1
} else {
    $msvcVersion = $msvcVersions[0].Name
    Write-Host "Using MSVC version: $msvcVersion" -ForegroundColor Green
}

# Windows SDK version (common versions - script will find the latest)
$windowsKitsPath = "C:\Program Files (x86)\Windows Kits\10"

# Find the latest Windows SDK version
$sdkVersions = Get-ChildItem "$windowsKitsPath\Include" -Directory | Sort-Object Name -Descending
if ($sdkVersions.Count -eq 0) {
    Write-Host "No Windows SDK found in $windowsKitsPath\Include" -ForegroundColor Yellow
    $sdkVersion = ""
} else {
    $sdkVersion = $sdkVersions[0].Name
    Write-Host "Using Windows SDK version: $sdkVersion" -ForegroundColor Green
}

Write-Host "Target architecture: $targetArch" -ForegroundColor Cyan

# Define all the paths we need to add
$pathsToAdd = @()

# MSVC Compiler and tools
$msvcBinPath = "$vsBasePath\VC\Tools\MSVC\$msvcVersion\bin\$hostArch\$targetArch"
if (Test-Path $msvcBinPath) {
    $pathsToAdd += $msvcBinPath
    Write-Host "Found MSVC tools: $msvcBinPath" -ForegroundColor Green
} else {
    Write-Host "MSVC tools not found: $msvcBinPath" -ForegroundColor Red
    # Try fallback path for cross-compilation tools
    $msvcBinPathFallback = "$vsBasePath\VC\Tools\MSVC\$msvcVersion\bin\Hostx64\$targetArch"
    if (Test-Path $msvcBinPathFallback) {
        $pathsToAdd += $msvcBinPathFallback
        Write-Host "Found MSVC tools (fallback): $msvcBinPathFallback" -ForegroundColor Yellow
    }
}

# LLVM/Clang tools
$libclangPath = "$vsBasePath\VC\Tools\Llvm\$llvmArch\bin"
if (Test-Path $libclangPath) {
    $pathsToAdd += $libclangPath
    Write-Host "Found LLVM/Clang tools: $libclangPath" -ForegroundColor Green
} else {
    Write-Host "LLVM/Clang tools not found: $libclangPath" -ForegroundColor Yellow
}

# Windows SDK tools (using correct architecture)
if ($sdkVersion) {
    $sdkBinPath = "$windowsKitsPath\bin\$sdkVersion\$sdkArch"
    if (Test-Path $sdkBinPath) {
        $pathsToAdd += $sdkBinPath
        Write-Host "Found Windows SDK tools: $sdkBinPath" -ForegroundColor Green
    } else {
        Write-Host "Windows SDK tools not found: $sdkBinPath" -ForegroundColor Yellow
    }
}

# MSBuild (if available)
$msbuildPath = "$vsBasePath\MSBuild\Current\Bin"
if (Test-Path $msbuildPath) {
    $pathsToAdd += $msbuildPath
    Write-Host "Found MSBuild: $msbuildPath" -ForegroundColor Green
} else {
    Write-Host "MSBuild not found: $msbuildPath" -ForegroundColor Yellow
}

# Set environment variables for this session
Write-Host ""
Write-Host "Setting up environment variables..." -ForegroundColor Cyan

# LIBCLANG_PATH for bindgen and other tools
$env:LIBCLANG_PATH = $libclangPath
Write-Host "LIBCLANG_PATH = $libclangPath"

# MSVC library and include paths
if ($sdkVersion) {
    # Windows SDK include paths
    $includeStr = @(
        "$vsBasePath\VC\Tools\MSVC\$msvcVersion\include",
        "$windowsKitsPath\Include\$sdkVersion\um",
        "$windowsKitsPath\Include\$sdkVersion\shared",
        "$windowsKitsPath\Include\$sdkVersion\winrt",
        "$windowsKitsPath\Include\$sdkVersion\ucrt"
    ) -join ";"

    $env:INCLUDE = $includeStr
    Write-Host "INCLUDE paths set"

    # Library paths (using correct architecture)
    $libStr = @(
        "$vsBasePath\VC\Tools\MSVC\$msvcVersion\lib\$targetArch",
        "$windowsKitsPath\Lib\$sdkVersion\um\$sdkArch",
        "$windowsKitsPath\Lib\$sdkVersion\ucrt\$sdkArch"
    ) -join ";"

    $env:LIB = $libStr
    Write-Host "LIB paths set"
}

# Visual Studio environment variables
$env:VCToolsInstallDir = "$vsBasePath\VC\Tools\MSVC\$msvcVersion\"
$env:VCToolsVersion = $msvcVersion
$env:VisualStudioVersion = "17.0"
$env:VSCMD_ARG_HOST_ARCH = $hostArch.Replace("Host", "").ToLower()
$env:VSCMD_ARG_TGT_ARCH = $targetArch

# Set the platform target for proper compilation
if ($arch -eq "arm64") {
    $env:Platform = "ARM64"
    Write-Host "Platform set to ARM64"
}

# Add all paths to PATH
$currentPath = $env:Path
$addedPaths = @()

foreach ($pathToAdd in $pathsToAdd) {
    if ($currentPath -notlike "*$pathToAdd*") {
        $env:Path = "$($env:Path);$pathToAdd"
        $addedPaths += $pathToAdd
        Write-Host "Added to PATH: $pathToAdd" -ForegroundColor Green
    } else {
        Write-Host "Already in PATH: $pathToAdd" -ForegroundColor Yellow
    }
}

Write-Host ""
Write-Host "Environment setup complete!" -ForegroundColor Green
