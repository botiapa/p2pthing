$in = $args[0];
if($in -like "g*") {
    Start-Process -NoNewWindow -Wait -FilePath "cmd"  -Args "/C cargo run --features server,tui,gui,audio g"
}
elseif($in -like "t*") {
    Start-Process -NoNewWindow -Wait -FilePath "cmd"  -Args "/C cargo run --features server,tui,gui,audio t"
}
elseif($in -like "s*") {
    Start-Process -NoNewWindow -Wait -FilePath "cmd"  -Args "/C cargo run --features server,tui,gui,audio s"
}
else {
    Write-Host "Invalid arg received. Valid args: gui, tui, or server"
}