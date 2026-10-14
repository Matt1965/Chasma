# Option B pilot export (Bufomorph + Muscomorph). Run from chasma-agent-c.
$ErrorActionPreference = "Stop"
$Root = Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
Set-Location $Root
& "$PSScriptRoot\ensure_fbx2gltf.ps1" | Out-Null

$Pack = "C:\Users\matt1\My project\Assets\Monsters Full Pack Vol 1"

py -3 tools\monster_pack\export_monster_fbx2gltf.py `
  --source-dir "$Pack\Monsters Pack Vol 5\Bufomorph" `
  --sk-fbx "$Pack\Monsters Pack Vol 5\Bufomorph\FBX Files\SK_Buformorph.FBX" `
  --anim-prefix Buformorph `
  --prefab "$Pack\Monsters Pack Vol 5\Bufomorph\Prefabs\Buformorph_Red.prefab" `
  --output "$Root\assets\units\bufomorph.glb"

py -3 tools\monster_pack\export_monster_fbx2gltf.py `
  --source-dir "$Pack\Monsters Pack Vol 2\Muscomorph" `
  --sk-fbx "$Pack\Monsters Pack Vol 2\Muscomorph\FBX Files\SK_Muscomorph.FBX" `
  --anim-prefix Muscomorph `
  --output "$Root\assets\units\muscomorph.glb"

Write-Host "Pilot exports complete. Run game from this worktree:"
Write-Host "  cd $Root"
Write-Host "  cargo run --features dev"
