$ErrorActionPreference = "Stop"
cargo test
cargo build --release
New-Item -ItemType Directory -Force -Path dist | Out-Null
Copy-Item target\release\wonderdraft-asset-studio.exe dist\wonderdraft-asset-studio-windows-x86_64.exe
Write-Host "Built: $PWD\dist\wonderdraft-asset-studio-windows-x86_64.exe"
