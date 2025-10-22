@echo off

:: Watch and recompile SCSS in the background
start sass --watch static/style.scss:static/style.css

:: Watch and rebuild the Yew project
start cargo watch -w src -s "wasm-pack build --target web"

:: Serve the files and enable live reload
start browser-sync start --server --files "static/*.css, pkg/*" --startPath index.html  --https --key "certs/key.pem" --cert "certs/cert.pem"
