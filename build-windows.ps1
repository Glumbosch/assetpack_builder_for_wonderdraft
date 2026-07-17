$ErrorActionPreference = "Stop"
cargo test
cargo build --release
New-Item -ItemType Directory -Force -Path dist | Out-Null
Copy-Item target\release\assetpack-builder-for-wonderdraft.exe dist\assetpack-builder-for-wonderdraft-windows-x86_64.exe
Write-Host "Built: $PWD\dist\assetpack-builder-for-wonderdraft-windows-x86_64.exe"
