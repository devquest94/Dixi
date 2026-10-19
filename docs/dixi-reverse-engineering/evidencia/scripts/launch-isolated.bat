@echo off
set APP=C:\Users\DanielSan\AppData\Local\WisprFlow\app-1.6.1174\Wispr Flow.exe
set PROFILE=D:\Codigo\Dixi\.tmp-audit\isolated-profile
set LOG=D:\Codigo\Dixi\.tmp-audit\isolated.log
if not exist "%PROFILE%" mkdir "%PROFILE%"
start "" /B "%APP%" --user-data-dir="%PROFILE%" --remote-debugging-port=9333 > "%LOG%" 2>&1
echo launched
