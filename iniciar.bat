@echo off
title Twitch Miner Go
echo ============================================================
echo      INICIANDO TWITCH CHANNEL POINTS MINER GO
echo ============================================================

if not exist config.json (
    if exist config.example.json (
        echo [INFO] Criando config.json a partir do config.example.json...
        copy config.example.json config.json >nul
        echo [AVISO] Edite o config.json com seu token antes de rodar!
        pause
    )
)

twitch-miner.exe
pause
