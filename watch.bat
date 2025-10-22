@echo off
setlocal

pushd "%~dp0"

:: Watch and recompile SCSS in the background
start "SASS Watch" sass --watch static/style.scss:static/style.css

:: Watch and rebuild the Yew project
start "Cargo Watch" cargo watch -w src -s "wasm-pack build --target web"

:: Serve the files and enable live reload
start "Browser Sync" browser-sync start --server --files "static/*.css, pkg/*" --startPath index.html

echo.
echo Preparing docs build...
set "DOCS_DIR=%CD%\docs"

if not exist "%DOCS_DIR%" (
    mkdir "%DOCS_DIR%"
)

call wasm-pack build --target web --out-dir "%DOCS_DIR%\pkg"
if errorlevel 1 (
    echo Failed to build WebAssembly package for docs output.
    goto :END
)

copy /Y index.html "%DOCS_DIR%\index.html" >nul
copy /Y manifest.json "%DOCS_DIR%\manifest.json" >nul

if exist service-worker.js (
    copy /Y service-worker.js "%DOCS_DIR%\service-worker.js" >nul
)

call :SyncDir static "%DOCS_DIR%\static"
call :SyncDir icons "%DOCS_DIR%\icons"

echo Docs output ready at "%DOCS_DIR%".
goto :END

:SyncDir
if not exist "%~1" (
    echo Skipping missing source "%~1".
    exit /b 0
)
robocopy "%~1" "%~2" /MIR >nul
if errorlevel 8 (
    echo Failed to copy "%~1" to "%~2".
)
exit /b 0

:END
popd
endlocal
