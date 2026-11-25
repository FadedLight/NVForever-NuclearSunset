@echo off
rem Start Nuclear Sunset on NVForever (this fork's build). Edit the two lines below, then run.
rem   MO2     = your Nuclear Sunset Mod Organizer 2 instance folder
rem   PROFILE = the MO2 profile to play
setlocal
set "MO2=C:\Modlists\NuclearSunset"
set "PROFILE=Hikikomori NVR High"

set "HERE=%~dp0"
set "NVF_REPORTS=0"
set "NVF_PLAY_SCRIPT_FUNCTIONS=%HERE%schemas\script\functions.json"
if not exist "%MO2%\profiles\%PROFILE%" (echo MO2 profile not found: "%MO2%\profiles\%PROFILE%" & pause & exit /b 1)
start "" /D "%HERE%" "%HERE%nvforever.exe" --esm-schema "%HERE%schemas\fnv" --nif-schema "%HERE%schemas\nif" --data "%MO2%\[NoDelete] Stock New Vegas\Data" --mo2 "%MO2%" --profile "%PROFILE%" %*
endlocal
