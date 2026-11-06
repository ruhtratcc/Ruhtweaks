param([Parameter(ValueFromRemainingArguments=$true)][string[]]$CargoArgs = @('test','--locked'))
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
Push-Location $repo
try {
    $localCargo = Join-Path $repo 'work/cargo'
    $localRust = Join-Path $repo 'work/rustup'
    if (Test-Path -LiteralPath (Join-Path $localCargo 'bin/cargo.exe')) {
        $env:CARGO_HOME = $localCargo
        $env:RUSTUP_HOME = $localRust
        $env:PATH = "$localCargo\bin;$env:PATH"
        $llvmBin = Join-Path $repo 'work/toolchain/llvm-mingw-20261020-msvcrt-x86_64/bin'
        if (Test-Path -LiteralPath $llvmBin) {
            $env:PATH = "$llvmBin;$env:PATH"
            $linkerAlias = Join-Path $llvmBin 'ld.exe'
            if (-not (Test-Path -LiteralPath $linkerAlias)) {
                Copy-Item -LiteralPath (Join-Path $llvmBin 'ld.lld.exe') -Destination $linkerAlias
            }
            $env:CARGO_ENCODED_RUSTFLAGS = @('-C','dlltool=llvm-dlltool.exe','-C',"link-arg=-B$($llvmBin.Replace('\','/'))/") -join [char]0x1f
            $env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER = Join-Path $localRust 'toolchains/1.99.1-x86_64-pc-windows-gnu/lib/rustlib/x86_64-pc-windows-gnu/bin/self-contained/x86_64-w64-mingw32-gcc.exe'
        }
    }
    & cargo @CargoArgs
    if ($LASTEXITCODE -ne 0) { throw "Cargo falhou: $LASTEXITCODE" }
} finally { Pop-Location }
